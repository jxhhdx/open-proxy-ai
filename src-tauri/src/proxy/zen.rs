use rand::RngCore;
use reqwest::{Client, Response};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const OC_VERSION: &str = "1.15.0";
const ZEN_URL: &str = "https://opencode.ai/zen/v1/chat/completions";
const TIMEOUT_SECS: u64 = 120;
const SESSION_ROTATION_MS: u64 = 30 * 60 * 1000;

pub struct ZenClient {
    client: Client,
}

impl ZenClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .user_agent(format!(
                "opencode/{OC_VERSION} ai-sdk/provider-utils/4.0.23 runtime/bun/1.3.13"
            ))
            .build()?;
        Ok(ZenClient { client })
    }

    pub fn build_headers(session_id: &str) -> reqwest::header::HeaderMap {
        use reqwest::header::*;
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(AUTHORIZATION, "Bearer public".parse().unwrap());
        headers.insert(
            HeaderName::from_static("x-opencode-client"),
            "cli".parse().unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-opencode-project"),
            "global".parse().unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-opencode-request"),
            format!("msg_{:x}", rand::random::<u64>())
                .parse()
                .unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-opencode-session"),
            session_id.parse().unwrap(),
        );
        headers
    }

    pub fn build_request_body(
        model: &str,
        messages: &serde_json::Value,
        stream: bool,
        tools: Option<&serde_json::Value>,
        original_body: Option<&serde_json::Value>,
    ) -> (serde_json::Value, String) {
        let body = if let Some(orig) = original_body {
            // Start from the original request body (preserves every param),
            // only override the fields we need to change.
            let mut cloned = orig.clone();
            cloned["model"] = serde_json::json!(model);
            cloned["messages"] = messages.clone();
            cloned["stream"] = serde_json::json!(stream);
            cloned
        } else {
            // No original body available — build from scratch.
            // This path is used when the original request is in Anthropic format
            // (messages_handler converting to OpenAI) or for synthetic test requests.
            let mut body = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": stream,
            });

            if let Some(tools) = tools {
                if let Some(arr) = tools.as_array() {
                    if !arr.is_empty() {
                        body["tools"] = tools.clone();
                    }
                }
            }
            body
        };

        let body_str = serde_json::to_string(&body).unwrap_or_default();
        (body, body_str)
    }

    pub async fn send_streaming(
        &self,
        body_str: String,
        session_id: &str,
    ) -> Result<Response, reqwest::Error> {
        let headers = Self::build_headers(session_id);
        self.client
            .post(ZEN_URL)
            .headers(headers)
            .body(body_str)
            .send()
            .await
    }

    pub async fn send_non_streaming(
        &self,
        body_str: String,
        session_id: &str,
    ) -> Result<(reqwest::StatusCode, serde_json::Value), reqwest::Error> {
        let headers = Self::build_headers(session_id);
        let response = self
            .client
            .post(ZEN_URL)
            .headers(headers)
            .body(body_str)
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value =
            response.json().await.unwrap_or(serde_json::json!({}));
        Ok((status, body))
    }

    pub fn is_error(body: &serde_json::Value) -> bool {
        body.get("error").is_some()
            || body
                .get("type")
                .and_then(|t| t.as_str())
                == Some("error")
    }

    pub fn extract_error(body: &serde_json::Value) -> String {
        body.pointer("/error/message")
            .and_then(|m| m.as_str())
            .or_else(|| body.get("message").and_then(|m| m.as_str()))
            .unwrap_or("Unknown error")
            .to_string()
    }
}

pub struct SessionManager {
    sessions: Mutex<HashMap<String, (String, u64)>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_session(&self, user: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut sessions = self.sessions.lock().unwrap();

        if let Some((session_id, ts)) = sessions.get(user) {
            if now - *ts < SESSION_ROTATION_MS {
                return session_id.clone();
            }
        }

        let ts_hex = format!("{:x}", now);
        let mut rnd_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut rnd_bytes);
        let rnd: String = rnd_bytes.iter().map(|b| format!("{:02x}", b)).collect();
        let session_id = format!("ses_{}{}", ts_hex, rnd);

        sessions.insert(user.to_string(), (session_id.clone(), now));
        session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_original_body_preserves_all_params() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let original = serde_json::json!({
            "model": "ignored",
            "messages": [],
            "stream": false,
            "max_tokens": 4096,
            "temperature": 0.7,
            "tool_choice": "auto",
            "n": 2,
            "user": "test-user",
            "stop": ["\n\n"],
        });

        let (body, _) = ZenClient::build_request_body("new-model", &messages, true, None, Some(&original));

        // Overridden fields
        assert_eq!(body["model"], "new-model");
        assert_eq!(body["messages"], messages);
        assert_eq!(body["stream"], true);
        // Preserved fields (no whitelist needed)
        assert_eq!(body["max_tokens"], 4096);
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(body["n"], 2);
        assert_eq!(body["user"], "test-user");
        assert_eq!(body["stop"][0], "\n\n");
    }

    #[test]
    fn test_with_original_body_preserves_unknown_fields() {
        let messages = serde_json::json!([{"role": "user", "content": "hi"}]);
        let original = serde_json::json!({
            "model": "old",
            "messages": [],
            "stream": false,
            "some_future_param": "will_still_be_there",
            "another_unknown": 42,
        });

        let (body, _) = ZenClient::build_request_body("new", &messages, true, None, Some(&original));

        assert_eq!(body["some_future_param"], "will_still_be_there");
        assert_eq!(body["another_unknown"], 42);
    }

    #[test]
    fn test_without_original_body_has_standard_fields() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);

        let (body, _) = ZenClient::build_request_body("m", &messages, true, None, None);

        assert_eq!(body["model"], "m");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"], messages);
        // No extra params leaked in
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn test_without_original_body_but_with_tools() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let tools = serde_json::json!([{"type": "function", "function": {"name": "test", "parameters": {"type": "object", "properties": {}}}}]);

        let (body, _) = ZenClient::build_request_body("m", &messages, false, Some(&tools), None);

        assert!(body.get("tools").is_some());
        assert_eq!(body["tools"][0]["function"]["name"], "test");
    }

    #[test]
    fn test_without_original_body_skips_empty_tools() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let tools = serde_json::json!([]);

        let (body, _) = ZenClient::build_request_body("m", &messages, false, Some(&tools), None);

        // When building from scratch, empty tools array should not add the field
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_with_original_body_overrides_model_and_messages() {
        let messages = serde_json::json!([{"role": "user", "content": "new_msg"}]);
        let original = serde_json::json!({
            "model": "old-model",
            "messages": [{"role": "user", "content": "old_msg"}],
            "stream": false,
        });

        let (body, _) = ZenClient::build_request_body("new-model", &messages, true, None, Some(&original));

        assert_eq!(body["model"], "new-model");
        assert_eq!(body["messages"][0]["content"], "new_msg");
        assert_eq!(body["stream"], true);
    }
}
