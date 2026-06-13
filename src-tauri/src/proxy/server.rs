use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use tracing::info;

use super::anthropic::{anthropic_to_openai, openai_to_anthropic, AnthropicStreamConverter};
use super::auth::AuthManager;
use super::log::AppLog;
use super::model_pool::ModelPool;
use super::zen::{SessionManager, ZenClient};

pub const MODELS: &[&str] = &[
    "deepseek-v4-flash-free",
    "big-pickle",
    "nemotron-3-ultra-free",
    "north-mini-code-free",
    "mimo-v2.5-free",
];

#[derive(Clone)]
pub struct ProxyState {
    pub auth: Arc<RwLock<AuthManager>>,
    pub zen: Arc<ZenClient>,
    pub sessions: Arc<SessionManager>,
    pub custom_models: Arc<RwLock<Vec<String>>>,
    pub model_pool: Arc<RwLock<ModelPool>>,
    pub log: Arc<AppLog>,
}

pub fn create_router(state: Arc<ProxyState>) -> Router {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(messages_handler))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn try_send_streaming(zen: &ZenClient, body: &str, session: &str) -> Result<Response, String> {
    match zen.send_streaming(body.to_string(), session).await {
        Ok(upstream_resp) => {
            let status = upstream_resp.status();
            if status != 200 {
                let text = upstream_resp.text().await.unwrap_or_default();
                return Err(format!("{}: {}", status.as_u16(), text));
            }
            let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
            let mut s = upstream_resp.bytes_stream();
            tokio::spawn(async move {
                while let Some(chunk) = s.next().await {
                    if let Ok(b) = chunk { if tx.send(b).await.is_err() { break; } }
                }
            });
            let stream = ReceiverStream::new(rx).map(|b| Ok::<_, std::convert::Infallible>(b));
            Ok(Response::builder()
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .body(Body::from_stream(stream))
                .unwrap_or_default())
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

async fn try_send_non_streaming(zen: &ZenClient, body: &str, session: &str) -> Result<Response, String> {
    match zen.send_non_streaming(body.to_string(), session).await {
        Ok((status, resp)) => {
            if status != 200 || ZenClient::is_error(&resp) {
                let msg = ZenClient::extract_error(&resp);
                return Err(format!("{}: {}", status.as_u16(), msg));
            }
            Ok(Json(resp).into_response())
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

async fn try_send_anthropic_streaming(
    zen: &ZenClient,
    body_str: &str,
    session: &str,
    model_name: &str,
    input_tokens: u64,
) -> Result<Response, String> {
    match zen.send_streaming(body_str.to_string(), session).await {
        Ok(upstream_resp) => {
            let status = upstream_resp.status();
            if status != 200 {
                let text = upstream_resp.text().await.unwrap_or_default();
                return Err(format!("{}: {}", status.as_u16(), text));
            }

            let msg_id = format!("msg_{:016x}", rand::random::<u64>());
            let model = model_name.to_string();
            let mut converter =
                AnthropicStreamConverter::new(msg_id.clone(), model.clone(), input_tokens);

            let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(64);
            let mut buffer = String::new();

            // Send initial message_start
            {
                let start_event = serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": msg_id,
                        "type": "message",
                        "role": "assistant",
                        "content": [],
                        "model": model,
                        "stop_reason": null,
                        "usage": {
                            "input_tokens": input_tokens,
                            "output_tokens": 0,
                            "cache_creation_input_tokens": 0,
                            "cache_read_input_tokens": 0
                        }
                    }
                });
                let sse_line = format!(
                    "event: message_start\ndata: {}\n\n",
                    serde_json::to_string(&start_event).unwrap_or_default()
                );
                let _ = tx.send(Ok(Bytes::from(sse_line))).await;
            }

            let mut upstream_stream = upstream_resp.bytes_stream();

            tokio::spawn(async move {
                while let Some(chunk) = upstream_stream.next().await {
                    let chunk = match chunk {
                        Ok(b) => b,
                        Err(_) => break,
                    };
                    let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                    buffer.push_str(&chunk_str);

                    while let Some(nl) = buffer.find('\n') {
                        let line = buffer[..nl].trim().to_string();
                        buffer = buffer[nl + 1..].to_string();

                        if !line.starts_with("data: ") {
                            continue;
                        }
                        let payload = line[6..].trim().to_string();
                        if payload == "[DONE]" {
                            continue;
                        }

                        let parsed: serde_json::Value = match serde_json::from_str(&payload) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let delta = parsed.pointer("/choices/0/delta");
                        let finish_reason = parsed
                            .pointer("/choices/0/finish_reason")
                            .and_then(|f| f.as_str());

                        if let Some(d) = delta {
                            let anthropic_events = converter.process_delta(d, finish_reason);
                            for (event_name, data_json) in anthropic_events {
                                let sse_line = format!(
                                    "event: {}\ndata: {}\n\n",
                                    event_name, data_json
                                );
                                if tx.send(Ok(Bytes::from(sse_line))).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                drop(tx);
            });

            let stream = ReceiverStream::new(rx);

            Ok(Response::builder()
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .body(Body::from_stream(stream))
                .unwrap_or_default())
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

async fn try_send_anthropic_non_streaming(
    zen: &ZenClient,
    body_str: &str,
    session: &str,
    model_name: &str,
    input_tokens: u64,
) -> Result<Response, String> {
    match zen.send_non_streaming(body_str.to_string(), session).await {
        Ok((status, resp)) => {
            if status != 200 || ZenClient::is_error(&resp) {
                let msg = ZenClient::extract_error(&resp);
                return Err(format!("{}: {}", status.as_u16(), msg));
            }
            Ok(Json(openai_to_anthropic(&resp, model_name, input_tokens)).into_response())
        }
        Err(e) => Err(format!("Request error: {}", e)),
    }
}

/// Strip trailing `/v1` from base_url to avoid double `/v1` when appending API paths.
fn build_api_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let base = base.strip_suffix("/v1").unwrap_or(base);
    format!("{}{}", base, path)
}

/// Create a reqwest Client with a 120-second timeout for custom provider requests.
fn custom_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// ── Custom Provider Helpers ─────────────────────────────────────────────

/// Send a request to a custom OpenAI-compatible provider.
async fn send_custom_openai(
    base_url: &str,
    api_key: &str,
    body_str: &str,
    stream: bool,
) -> Result<Response, String> {
    let client = custom_http_client();
    let url = build_api_url(base_url, "/v1/chat/completions");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body_str.to_string())
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    let status = resp.status();
    if status != 200 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{}: {}", status.as_u16(), text));
    }

    if !stream {
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        return Ok(Json(body).into_response());
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let mut s = resp.bytes_stream();
    tokio::spawn(async move {
        while let Some(chunk) = s.next().await {
            if let Ok(b) = chunk {
                if tx.send(b).await.is_err() {
                    break;
                }
            }
        }
    });
    let stream = ReceiverStream::new(rx).map(|b| Ok::<_, std::convert::Infallible>(b));
    Ok(Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap_or_default())
}

/// Send to custom OpenAI provider and convert response to Anthropic format (streaming).
async fn custom_anthropic_streaming(
    base_url: &str,
    api_key: &str,
    body_str: &str,
    model_name: &str,
    input_tokens: u64,
) -> Result<Response, String> {
    let client = custom_http_client();
    let url = build_api_url(base_url, "/v1/chat/completions");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body_str.to_string())
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    let status = resp.status();
    if status != 200 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{}: {}", status.as_u16(), text));
    }

    let msg_id = format!("msg_{:016x}", rand::random::<u64>());
    let model = model_name.to_string();
    let mut converter =
        AnthropicStreamConverter::new(msg_id.clone(), model.clone(), input_tokens);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(64);
    let mut buffer = String::new();

    // Send initial message_start
    {
        let start_event = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": msg_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": model,
                "stop_reason": null,
                "usage": {
                    "input_tokens": input_tokens,
                    "output_tokens": 0,
                    "cache_creation_input_tokens": 0,
                    "cache_read_input_tokens": 0
                }
            }
        });
        let sse_line = format!(
            "event: message_start\ndata: {}\n\n",
            serde_json::to_string(&start_event).unwrap_or_default()
        );
        let _ = tx.send(Ok(Bytes::from(sse_line))).await;
    }

    let mut upstream_stream = resp.bytes_stream();

    tokio::spawn(async move {
        while let Some(chunk) = upstream_stream.next().await {
            let chunk = match chunk {
                Ok(b) => b,
                Err(_) => break,
            };
            let chunk_str = String::from_utf8_lossy(&chunk).to_string();
            buffer.push_str(&chunk_str);

            while let Some(nl) = buffer.find('\n') {
                let line = buffer[..nl].trim().to_string();
                buffer = buffer[nl + 1..].to_string();

                if !line.starts_with("data: ") {
                    continue;
                }
                let payload = line[6..].trim().to_string();
                if payload == "[DONE]" {
                    continue;
                }

                let parsed: serde_json::Value = match serde_json::from_str(&payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let delta = parsed.pointer("/choices/0/delta");
                let finish_reason = parsed
                    .pointer("/choices/0/finish_reason")
                    .and_then(|f| f.as_str());

                if let Some(d) = delta {
                    let anthropic_events = converter.process_delta(d, finish_reason);
                    for (event_name, data_json) in anthropic_events {
                        let sse_line = format!(
                            "event: {}\ndata: {}\n\n",
                            event_name, data_json
                        );
                        if tx.send(Ok(Bytes::from(sse_line))).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
        drop(tx);
    });

    let stream = ReceiverStream::new(rx);

    Ok(Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap_or_default())
}

/// Send to custom OpenAI provider and convert response to Anthropic format (non-streaming).
async fn custom_anthropic_non_streaming(
    base_url: &str,
    api_key: &str,
    body_str: &str,
    model_name: &str,
    input_tokens: u64,
) -> Result<Response, String> {
    let client = custom_http_client();
    let url = build_api_url(base_url, "/v1/chat/completions");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body_str.to_string())
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    let status = resp.status();
    if status != 200 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{}: {}", status.as_u16(), text));
    }

    let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
    if json.get("error").is_some() {
        let msg = json
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("{}: {}", status.as_u16(), msg));
    }

    Ok(Json(openai_to_anthropic(&json, model_name, input_tokens)).into_response())
}

/// Send directly to a custom Anthropic-compatible provider.
async fn send_custom_anthropic_direct(
    base_url: &str,
    api_key: &str,
    body_str: &str,
    stream: bool,
) -> Result<Response, String> {
    let client = custom_http_client();
    let url = build_api_url(base_url, "/v1/messages");
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .body(body_str.to_string())
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    let status = resp.status();
    if status != 200 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{}: {}", status.as_u16(), text));
    }

    if !stream {
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        return Ok(Json(body).into_response());
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let mut s = resp.bytes_stream();
    tokio::spawn(async move {
        while let Some(chunk) = s.next().await {
            if let Ok(b) = chunk { if tx.send(b).await.is_err() { break; } }
        }
    });
    let stream = ReceiverStream::new(rx).map(|b| Ok::<_, std::convert::Infallible>(b));
    Ok(Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap_or_default())
}

fn get_all_models(custom_models: &[String]) -> Vec<String> {
    let mut all: Vec<String> = MODELS.iter().map(|m| m.to_string()).collect();
    for cm in custom_models {
        if !all.contains(cm) {
            all.push(cm.clone());
        }
    }
    all
}

fn auth_user(
    headers: &HeaderMap,
    auth: &AuthManager,
) -> Result<String, Response> {
    let hdr = headers
        .get("authorization")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok());

    match auth.authenticate(hdr) {
        Some(user) => Ok(user),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"message": "Invalid API key", "type": "auth_error"}
            })),
        )
            .into_response()),
    }
}

// ── GET /v1/models ────────────────────────────────────────────────────

async fn list_models(
    State(state): State<Arc<ProxyState>>,
) -> Json<serde_json::Value> {
    let custom = state.custom_models.read().await;
    let all = get_all_models(&custom);
    let data: Vec<serde_json::Value> = all
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "object": "model",
                "created": 1779000000,
                "owned_by": "free"
            })
        })
        .collect();
    Json(serde_json::json!({"object": "list", "data": data}))
}

// ── POST /v1/chat/completions ─────────────────────────────────────────

async fn chat_completions(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let user = match auth_user(&headers, &*state.auth.read().await) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let model = match body.get("model").and_then(|m| m.as_str()) {
        Some(m) => m.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {"message": "Missing model field"}
                })),
            )
                .into_response();
        }
    };

    let messages = body.get("messages").cloned().unwrap_or(serde_json::json!([]));
    let stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    let tools = body.get("tools");
    let session_id = state.sessions.get_session(&user);

    info!(
        user = %user,
        model = %model,
        stream = %stream,
        "OpenAI chat completion"
    );

    let pool = state.model_pool.read().await;
    let mut models: Vec<String> = Vec::new();
    let mut custom_routes: Vec<(String, String, String)> = Vec::new(); // (base_url, api_key, api_format) per model
    if model == "ModelPool" {
        // Route through entire pool by priority
        for e in pool.get_enabled() {
            models.push(e.model_name.clone());
            if !e.base_url.is_empty() {
                custom_routes.push((e.base_url.clone(), e.api_key.clone(), e.api_format.clone()));
            } else {
                custom_routes.push((String::new(), String::new(), String::new()));
            }
        }
        info!(
            "ModelPool routing order: [{}]",
            models.iter().map(|m| m.as_str()).collect::<Vec<&str>>().join(", ")
        );
    } else if let Some(e) = pool.get_by_name(&model) {
        // Use only this specific model, no pool failover
        if e.enabled {
            models.push(e.model_name.clone());
            if !e.base_url.is_empty() {
                custom_routes.push((e.base_url.clone(), e.api_key.clone(), e.api_format.clone()));
            } else {
                custom_routes.push((String::new(), String::new(), String::new()));
            }
        }
    }
    if models.is_empty() {
        models.push(model.clone());
        custom_routes.push((String::new(), String::new(), String::new()));
    }
    drop(pool);

    let mut last_error = String::from("All models failed");
    for (i, m) in models.iter().enumerate() {
        // Skip if different from requested and requested is still in list (tried first)
        // Build request for this model
        let (_, body_str) = ZenClient::build_request_body(m, &messages, stream, tools, Some(&body));
        let (ref base_url, ref api_key, ref _api_format) = custom_routes[i];
        let result = if !base_url.is_empty() {
            send_custom_openai(base_url, api_key, &body_str, stream).await
        } else if stream {
            try_send_streaming(&state.zen, &body_str, &session_id).await
        } else {
            try_send_non_streaming(&state.zen, &body_str, &session_id).await
        };

        match result {
            Ok(response) => {
                info!("Model pool success on {} (attempt {}/{})", m, i + 1, models.len());
                return response;
            }
            Err(e) => {
                info!("Model pool entry {} (attempt {}/{}) failed: {}", m, i + 1, models.len(), e);
                if m != models.last().unwrap() {
                    info!("Failover: {} -> next", m);
                    last_error = format!("{}: {}", m, e);
                    continue;
                }
                last_error = format!("{}: {}", m, e);
                break;
            }
        }
    }
    return (StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": {"message": last_error, "type": "failover_error"}})))
        .into_response();
}

// ── POST /v1/messages (Anthropic format) ──────────────────────────────

async fn messages_handler(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let user = match auth_user(&headers, &*state.auth.read().await) {
        Ok(u) => u,
        Err(r) => return r,
    };

    let model = match body.get("model").and_then(|m| m.as_str()) {
        Some(m) => m.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "type": "error",
                    "error": {"type": "invalid_request_error", "message": "Missing model"}
                })),
            )
                .into_response();
        }
    };

    let stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    let session_id = state.sessions.get_session(&user);

    let (messages, tools) = anthropic_to_openai(&body);
    let input_tokens = serde_json::to_string(&messages)
        .map(|s| (s.len() / 4) as u64)
        .unwrap_or(0);

    info!(
        user = %user,
        model = %model,
        stream = %stream,
        msg_count = messages.len(),
        "Anthropic messages"
    );

    let msgs_val = serde_json::json!(messages);
    let tools_val = tools.map(|t| serde_json::json!(t));

    // Resolve ModelPool into prioritized list with failover
    let pool = state.model_pool.read().await;
    let mut models: Vec<String> = Vec::new();
    let mut custom_routes: Vec<(String, String, String)> = Vec::new(); // (base_url, api_key, api_format) per model
    if model == "ModelPool" {
        for e in pool.get_enabled() {
            models.push(e.model_name.clone());
            if !e.base_url.is_empty() {
                custom_routes.push((e.base_url.clone(), e.api_key.clone(), e.api_format.clone()));
            } else {
                custom_routes.push((String::new(), String::new(), String::new()));
            }
        }
        info!(
            "ModelPool routing order: [{}]",
            models.iter().map(|m| m.as_str()).collect::<Vec<&str>>().join(", ")
        );
    } else if let Some(e) = pool.get_by_name(&model) {
        if e.enabled {
            models.push(e.model_name.clone());
            if !e.base_url.is_empty() {
                custom_routes.push((e.base_url.clone(), e.api_key.clone(), e.api_format.clone()));
            } else {
                custom_routes.push((String::new(), String::new(), String::new()));
            }
        }
    }
    if models.is_empty() {
        models.push(model.clone());
        custom_routes.push((String::new(), String::new(), String::new()));
    }
    drop(pool);

    let mut last_error = String::from("All models failed");
    for (i, m) in models.iter().enumerate() {
        let (_, body_str) =
            ZenClient::build_request_body(m, &msgs_val, stream, tools_val.as_ref(), Some(&body));
        let (ref base_url, ref api_key, ref api_format) = custom_routes[i];
        let result = if !base_url.is_empty() {
            if api_format == "anthropic" {
                // Send original Anthropic body directly
                let original_body_str = serde_json::to_string(&body).unwrap_or_default();
                send_custom_anthropic_direct(base_url, api_key, &original_body_str, stream).await
            } else if stream {
                custom_anthropic_streaming(base_url, api_key, &body_str, m, input_tokens).await
            } else {
                custom_anthropic_non_streaming(base_url, api_key, &body_str, m, input_tokens).await
            }
        } else if stream {
            try_send_anthropic_streaming(
                &state.zen,
                &body_str,
                &session_id,
                m,
                input_tokens,
            )
            .await
        } else {
            try_send_anthropic_non_streaming(
                &state.zen,
                &body_str,
                &session_id,
                m,
                input_tokens,
            )
            .await
        };

        match result {
            Ok(response) => {
                info!("Model pool success on {} (attempt {}/{})", m, i + 1, models.len());
                return response;
            }
            Err(e) => {
                info!("Model pool entry {} (attempt {}/{}) failed: {}", m, i + 1, models.len(), e);
                if m != models.last().unwrap() {
                    info!("Failover: {} -> next", m);
                    last_error = format!("{}: {}", m, e);
                    continue;
                }
                last_error = format!("{}: {}", m, e);
                break;
            }
        }
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": {"message": last_error, "type": "failover_error"}
        })),
    )
        .into_response()
}

// ── GET /health ───────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": "v9",
        "models": MODELS.len(),
        "endpoints": ["/v1/chat/completions", "/v1/messages", "/v1/models"]
    }))
}

// ── Speed Test ────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct SpeedTestResult {
    pub model: String,
    pub success: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
    pub tokens_per_sec: f64,
    pub total_tokens: u64,
    pub response_preview: String,
}

#[allow(dead_code)]
pub async fn run_speed_test(
    state: &ProxyState,
    model: &str,
) -> SpeedTestResult {
    let test_messages = serde_json::json!([
        {"role": "user", "content": "Reply with exactly 'OK' and nothing else."}
    ]);

    // Check model pool for custom provider URL
    let pool = state.model_pool.read().await;
    let entry = pool.get_by_name(model);
    let use_custom = entry.and_then(|e| {
        if !e.base_url.is_empty() {
            Some((e.base_url.clone(), e.api_key.clone(), e.model_name.clone(), e.api_format.clone()))
        } else {
            None
        }
    });
    drop(pool);

    let start = Instant::now();

    if let Some((ref base_url, ref api_key, ref model_name, ref api_format)) = use_custom {
        // Custom provider: send to user's API endpoint
        let client = custom_http_client();
        let is_anthropic = api_format == "anthropic";
        let url = if is_anthropic {
            build_api_url(base_url, "/v1/messages")
        } else {
            build_api_url(base_url, "/v1/chat/completions")
        };
        let body = if is_anthropic {
            serde_json::json!({
                "model": model_name,
                "messages": [{"role": "user", "content": "Reply with exactly 'OK' and nothing else."}],
                "max_tokens": 50,
            })
        } else {
            serde_json::json!({
                "model": model_name,
                "messages": test_messages,
                "stream": false,
            })
        };
        let json_body = serde_json::to_string(&body).unwrap_or_default();
        let auth_val = if is_anthropic { api_key.clone() } else { format!("Bearer {}", api_key) };
        let auth_header = if is_anthropic { "x-api-key" } else { "Authorization" };
        let resp = client.post(&url)
            .header("Content-Type", "application/json")
            .header(auth_header, &auth_val)
            .body(json_body)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let elapsed = start.elapsed().as_millis() as u64;
                if !r.status().is_success() {
                    let status = r.status().as_u16();
                    let body_text = r.text().await.unwrap_or_default();
                    let detail = if body_text.len() > 150 { format!("{}...", &body_text[..150]) } else { body_text };
                    return SpeedTestResult {
                        model: model.to_string(), success: false,
                        error: Some(format!("HTTP {}: {}", status, detail)),
                        latency_ms: elapsed, tokens_per_sec: 0.0, total_tokens: 0,
                        response_preview: String::new(),
                    };
                }
                match r.json::<serde_json::Value>().await {
                    Ok(data) => {
                        let (total, comp, preview) = if is_anthropic {
                            let it = data.get("usage").and_then(|u| u.get("input_tokens")).and_then(|t| t.as_u64()).unwrap_or(0);
                            let ot = data.get("usage").and_then(|u| u.get("output_tokens")).and_then(|t| t.as_u64()).unwrap_or(0);
                            let text = data.pointer("/content/0/text").and_then(|c| c.as_str()).unwrap_or("").chars().take(100).collect();
                            (it + ot, ot, text)
                        } else {
                            let total = data.pointer("/usage/total_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                            let comp = data.pointer("/usage/completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                            let text = data.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or("").chars().take(100).collect();
                            (total, comp, text)
                        };
                        let tps = if elapsed > 0 && comp > 0 { (comp as f64) / (elapsed as f64 / 1000.0) } else { 0.0 };
                        SpeedTestResult { model: model.to_string(), success: true, error: None, latency_ms: elapsed, tokens_per_sec: tps, total_tokens: total, response_preview: preview }
                    }
                    Err(e) => SpeedTestResult { model: model.to_string(), success: false, error: Some(format!("Parse error: {}", e)), latency_ms: elapsed, tokens_per_sec: 0.0, total_tokens: 0, response_preview: String::new() }
                }
            }
            Err(e) => SpeedTestResult { model: model.to_string(), success: false, error: Some(format!("Request failed: {}", e)), latency_ms: start.elapsed().as_millis() as u64, tokens_per_sec: 0.0, total_tokens: 0, response_preview: String::new() }
        }
    } else {
        // Free model from upstream: use Zen API
        let session_id = state.sessions.get_session("speedtest");
        let (_, body_str) = ZenClient::build_request_body(model, &test_messages, false, None, None);

    match state.zen.send_non_streaming(body_str, &session_id).await {
        Ok((status, resp)) => {
            let elapsed = start.elapsed().as_millis() as u64;

            if status != 200 {
                let msg = ZenClient::extract_error(&resp);
                return SpeedTestResult {
                    model: model.to_string(),
                    success: false,
                    error: Some(msg),
                    latency_ms: elapsed,
                    tokens_per_sec: 0.0,
                    total_tokens: 0,
                    response_preview: String::new(),
                };
            }

            let total_tokens = resp
                .pointer("/usage/total_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(0);

            let completion_tokens = resp
                .pointer("/usage/completion_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(0);

            let tokens_per_sec = if elapsed > 0 && completion_tokens > 0 {
                (completion_tokens as f64) / (elapsed as f64 / 1000.0)
            } else {
                0.0
            };

            let preview = resp
                .pointer("/choices/0/message/content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .chars()
                .take(100)
                .collect();

            SpeedTestResult {
                model: model.to_string(),
                success: true,
                error: None,
                latency_ms: elapsed,
                tokens_per_sec,
                total_tokens,
                response_preview: preview,
            }
        }
        Err(e) => SpeedTestResult {
            model: model.to_string(),
            success: false,
            error: Some(format!("{}", e)),
            latency_ms: start.elapsed().as_millis() as u64,
            tokens_per_sec: 0.0,
            total_tokens: 0,
            response_preview: String::new(),
        },
    }
    }  // closes else
}
