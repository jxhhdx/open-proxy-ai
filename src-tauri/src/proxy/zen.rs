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
        extra: Option<&serde_json::Value>,
    ) -> (serde_json::Value, String) {
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

        // Pass through extra chat completion params from the original request
        const PASSTHROUGH_KEYS: &[&str] = &[
            "max_tokens",
            "temperature",
            "top_p",
            "stop",
            "frequency_penalty",
            "presence_penalty",
            "seed",
            "response_format",
        ];
        if let Some(extra) = extra {
            if let Some(obj) = extra.as_object() {
                for key in PASSTHROUGH_KEYS {
                    if let Some(val) = obj.get(*key) {
                        body[key] = val.clone();
                    }
                }
            }
        }

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
    fn test_build_request_body_passes_through_max_tokens() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let extra = serde_json::json!({"max_tokens": 4096, "temperature": 0.7});

        let (body, _) = ZenClient::build_request_body("test-model", &messages, false, None, Some(&extra));

        assert_eq!(body["max_tokens"], 4096);
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(body["model"], "test-model");
    }

    #[test]
    fn test_build_request_body_without_extra_preserves_standard() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);

        let (body, _) = ZenClient::build_request_body("m", &messages, true, None, None);

        assert_eq!(body["model"], "m");
        assert_eq!(body["stream"], true);
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn test_build_request_body_passes_through_tools() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let tools = serde_json::json!([{"type": "function", "function": {"name": "test", "parameters": {"type": "object", "properties": {}}}}]);

        let (body, _) = ZenClient::build_request_body("m", &messages, false, Some(&tools), None);

        assert!(body.get("tools").is_some());
        assert_eq!(body["tools"][0]["function"]["name"], "test");
    }

    #[test]
    fn test_build_request_body_ignores_unknown_keys_in_extra() {
        let messages = serde_json::json!([{"role": "user", "content": "hello"}]);
        let extra = serde_json::json!({"unknown_param": 123, "also_wrong": "yes"});

        let (body, _) = ZenClient::build_request_body("m", &messages, false, None, Some(&extra));

        assert!(body.get("unknown_param").is_none());
        assert_eq!(body["model"], "m");
    }
}
