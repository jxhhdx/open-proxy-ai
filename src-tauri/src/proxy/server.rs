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
use std::time::Instant;
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
    "minimax-m2.5-free",
    "nemotron-3-super-free",
    "qwen3.6-plus-free",
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

fn check_model(model: &str, custom: &[String]) -> bool {
    MODELS.contains(&model) || custom.contains(&model.to_string())
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
                "owned_by": "opencode-free"
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

    let custom = state.custom_models.read().await;
    if !check_model(&model, &custom) {
        let all = get_all_models(&custom);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {"message": format!("Unknown model: {}. Available: {}", model, all.join(", "))}
            })),
        )
            .into_response();
    }
    drop(custom);

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

    let (_, body_str) = ZenClient::build_request_body(&model, &messages, stream, tools);

    if stream {
        match state.zen.send_streaming(body_str, &session_id).await {
            Ok(upstream_resp) => {
                let status = upstream_resp.status();
                if status != 200 {
                    let text = upstream_resp
                        .text()
                        .await
                        .unwrap_or_default();
                    return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                        Json(serde_json::json!({"error": {"message": text, "type": "upstream_error"}})))
                        .into_response();
                }

                let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
                let mut upstream_stream = upstream_resp.bytes_stream();

                tokio::spawn(async move {
                    while let Some(chunk) = upstream_stream.next().await {
                        match chunk {
                            Ok(b) => {
                                if tx.send(b).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                let stream =
                    ReceiverStream::new(rx).map(|b| Ok::<_, std::convert::Infallible>(b));

                Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .header("X-Accel-Buffering", "no")
                    .body(Body::from_stream(stream))
                    .unwrap_or_default()
            }
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {"message": format!("Upstream error: {}", e), "type": "upstream_error"}
                })),
            )
                .into_response(),
        }
    } else {
        match state.zen.send_non_streaming(body_str, &session_id).await {
            Ok((status, resp)) => {
                if status != 200 || ZenClient::is_error(&resp) {
                    let msg = ZenClient::extract_error(&resp);
                    return (
                        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                        Json(serde_json::json!({
                            "error": {"message": format!("{} (free model rate limit)", msg), "type": "rate_limit_error"}
                        })),
                    )
                        .into_response();
                }
                Json(resp).into_response()
            }
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {"message": format!("Upstream error: {}", e), "type": "upstream_error"}
                })),
            )
                .into_response(),
        }
    }
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

    let custom = state.custom_models.read().await;
    if !check_model(&model, &custom) {
        let all = get_all_models(&custom);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "type": "error",
                "error": {"type": "invalid_request_error", "message": format!("Unknown model: {}. Available: {}", model, all.join(", "))}
            })),
        )
            .into_response();
    }
    drop(custom);

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
    let (_, body_str) =
        ZenClient::build_request_body(&model, &msgs_val, stream, tools_val.as_ref());

    if stream {
        match state.zen.send_streaming(body_str, &session_id).await {
            Ok(upstream_resp) => {
                let status = upstream_resp.status();
                if status != 200 {
                    let text = upstream_resp.text().await.unwrap_or_default();
                    return (
                        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                        Json(serde_json::json!({
                            "type": "error",
                            "error": {"type": "upstream_error", "message": text}
                        })),
                    )
                        .into_response();
                }

                let msg_id = format!("msg_{:016x}", rand::random::<u64>());
                let mut converter =
                    AnthropicStreamConverter::new(msg_id.clone(), model.clone(), input_tokens);

                let (tx, rx) =
                    tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(64);
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

                        // Process complete SSE lines
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
                            let delta = parsed
                                .pointer("/choices/0/delta");
                            let finish_reason = parsed
                                .pointer("/choices/0/finish_reason")
                                .and_then(|f| f.as_str());

                            if let Some(d) = delta {
                                let anthropic_events =
                                    converter.process_delta(d, finish_reason);
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

                Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(Body::from_stream(stream))
                    .unwrap_or_default()
            }
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "type": "error",
                    "error": {"type": "upstream_error", "message": format!("{}", e)}
                })),
            )
                .into_response(),
        }
    } else {
        match state.zen.send_non_streaming(body_str, &session_id).await {
            Ok((status, resp)) => {
                if status != 200 || ZenClient::is_error(&resp) {
                    let msg = ZenClient::extract_error(&resp);
                    return (
                        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                        Json(serde_json::json!({
                            "type": "error",
                            "error": {"type": "rate_limit_error", "message": format!("{} (free model rate limit)", msg)}
                        })),
                    )
                        .into_response();
                }
                Json(openai_to_anthropic(&resp, &model, input_tokens)).into_response()
            }
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "type": "error",
                    "error": {"type": "upstream_error", "message": format!("{}", e)}
                })),
            )
                .into_response(),
        }
    }
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
            Some((e.base_url.clone(), e.api_key.clone(), e.model_name.clone()))
        } else {
            None
        }
    });
    drop(pool);

    let start = Instant::now();

    if let Some((ref base_url, ref api_key, ref model_name)) = use_custom {
        // Custom provider: send to user's API endpoint
        let client = reqwest::Client::new();
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": model_name,
            "messages": test_messages,
            "stream": false,
        });
        let resp = client.post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
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
                        let total = data.pointer("/usage/total_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                        let comp = data.pointer("/usage/completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
                        let tps = if elapsed > 0 && comp > 0 { (comp as f64) / (elapsed as f64 / 1000.0) } else { 0.0 };
                        let preview = data.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or("").chars().take(100).collect();
                        SpeedTestResult { model: model.to_string(), success: true, error: None, latency_ms: elapsed, tokens_per_sec: tps, total_tokens: total, response_preview: preview }
                    }
                    Err(e) => SpeedTestResult { model: model.to_string(), success: false, error: Some(format!("Parse error: {}", e)), latency_ms: elapsed, tokens_per_sec: 0.0, total_tokens: 0, response_preview: String::new() }
                }
            }
            Err(e) => SpeedTestResult { model: model.to_string(), success: false, error: Some(format!("Request failed: {}", e)), latency_ms: start.elapsed().as_millis() as u64, tokens_per_sec: 0.0, total_tokens: 0, response_preview: String::new() }
        }
    } else {
        // OpenCode free model: use Zen API
        let session_id = state.sessions.get_session("speedtest");
        let (_, body_str) = ZenClient::build_request_body(model, &test_messages, false, None);

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
