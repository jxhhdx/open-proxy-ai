use serde_json::Value;

/// Responses API-specific fields that must be removed when converting
/// a Responses API request body to a Chat Completions request body.
const RESPONSES_ONLY_FIELDS: &[&str] = &[
    "input",
    "instructions",
    "include",
    "previous_response_id",
    "store",
    "stream_options",
    "client_metadata",
    "prompt_cache_key",
    "reasoning",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "max_output_tokens",
];

/// Extract plain text from a Responses API `content` value.
///
/// The content can be a plain string (`"hello"`) or an array of
/// content blocks (`[{"type":"input_text","text":"hello"}]`).
pub fn extract_text_from_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<&str>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Normalize a single Responses API message object to Chat Completions format.
///
/// * `developer` role → `system`
/// * Array content → plain text
pub fn normalize_message(msg: &Value) -> Value {
    let mut out = msg.clone();
    if let Some(role) = out.get("role").and_then(|r| r.as_str()) {
        if role == "developer" {
            out["role"] = serde_json::json!("system");
        }
    }
    if let Some(content) = out.get("content") {
        if content.is_array() {
            out["content"] = serde_json::json!(extract_text_from_content(content));
        }
    }
    out
}

/// Remove Responses API-only fields from a request body so that the
/// resulting body is a valid Chat Completions request.
pub fn strip_responses_fields(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        for field in RESPONSES_ONLY_FIELDS {
            obj.remove(*field);
        }
    }
}

/// Convert Chat Completions usage format to Responses API format.
///
/// Chat:    `{prompt_tokens, completion_tokens, total_tokens}`
/// Responses: `{input_tokens, output_tokens, total_tokens}`
pub fn convert_usage(chat_usage: &Value) -> Value {
    if chat_usage.is_null() || !chat_usage.is_object() {
        return Value::Null;
    }
    serde_json::json!({
        "input_tokens": chat_usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "output_tokens": chat_usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "total_tokens": chat_usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

/// Build a response object snippet (shared by streaming and non-streaming paths).
fn build_response_obj(id: &str, msg_id: &str, model: &str, text: &str, status: &str, usage: Option<&Value>) -> Value {
    let now = unix_now();
    let mut obj = serde_json::json!({
        "id": id,
        "object": "response",
        "created_at": now,
        "status": status,
        "model": model,
        "output": [
            {
                "type": "message",
                "id": msg_id,
                "role": "assistant",
                "content": [
                    {
                        "type": "output_text",
                        "text": text,
                        "annotations": []
                    }
                ]
            }
        ]
    });
    if let Some(u) = usage {
        obj["usage"] = u.clone();
    }
    obj
}

/// Convert a Chat Completions non-streaming response to Responses API format.
pub fn chat_to_responses(chat_resp: &Value, model: &str) -> Value {
    let resp_id = format!("resp_{:016x}", rand::random::<u64>());
    let msg_id = format!("msg_{:016x}", rand::random::<u64>());

    let choices = chat_resp
        .get("choices")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    let content = choices
        .first()
        .and_then(|c| c.pointer("/message/content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let usage = chat_resp.get("usage").map(convert_usage);

    build_response_obj(&resp_id, &msg_id, model, content, "completed", usage.as_ref())
}

/// SSE converter: transforms OpenAI Chat Completions SSE lines
/// into OpenAI Responses API SSE events on-the-fly.
pub struct ResponsesSseConverter {
    resp_id: String,
    msg_id: String,
    model: String,
    /// Accumulated text content across all deltas.
    buffer: String,
    /// Whether we've seen a finish_reason (stop / length / …).
    finished: bool,
    /// Whether final_events has been called (prevents double send).
    finalized: bool,
    /// Usage data from the last upstream chunk (prompt_tokens etc).
    usage: Option<Value>,
}

impl ResponsesSseConverter {
    pub fn new(model: &str) -> Self {
        Self {
            resp_id: format!("resp_{:016x}", rand::random::<u64>()),
            msg_id: format!("msg_{:016x}", rand::random::<u64>()),
            model: model.to_string(),
            buffer: String::new(),
            finished: false,
            finalized: false,
            usage: None,
        }
    }

    /// Record usage data from an upstream Chat Completions SSE payload.
    pub fn set_usage(&mut self, payload: &Value) {
        if self.usage.is_none() {
            if let Some(u) = payload.get("usage") {
                if !u.is_null() {
                    // Convert Chat Completions usage keys to Responses API.
                    // Chat: {prompt_tokens, completion_tokens, total_tokens}
                    // Responses: {input_tokens, output_tokens, total_tokens}
                    self.usage = Some(serde_json::json!({
                        "input_tokens": u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        "output_tokens": u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                        "total_tokens": u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                    }));
                }
            }
        }
    }

    /// SSE events that must be sent **before** processing upstream chunks.
    pub fn initial_events(&self) -> Vec<(&str, String)> {
        let now = unix_now();
        vec![
            (
                "response.created",
                serde_json::json!({
                    "type": "response.created",
                    "response": {
                        "id": self.resp_id,
                        "object": "response",
                        "created_at": now,
                        "status": "in_progress",
                        "model": self.model,
                    }
                })
                .to_string(),
            ),
            (
                "response.output_item.added",
                serde_json::json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": {
                        "type": "message",
                        "id": self.msg_id,
                        "role": "assistant",
                        "content": []
                    }
                })
                .to_string(),
            ),
            (
                "response.content_part.added",
                serde_json::json!({
                    "type": "response.content_part.added",
                    "index": 0,
                    "part": {
                        "type": "output_text",
                        "text": "",
                        "annotations": []
                    }
                })
                .to_string(),
            ),
        ]
    }

    /// Process one Chat Completions delta, returning zero or more
    /// (event_name, data_json) tuples to emit.
    pub fn process_delta(
        &mut self,
        delta: &Value,
        finish_reason: Option<&str>,
    ) -> Vec<(&str, String)> {
        let mut events = Vec::new();

        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                self.buffer.push_str(text);
                events.push((
                    "output_text.delta",
                    serde_json::json!({
                        "type": "output_text.delta",
                        "index": 0,
                        "delta": text,
                    })
                    .to_string(),
                ));
            }
        }

        if let Some(reason) = finish_reason {
            if !self.finished {
                self.finished = true;
                events.push((
                    "output_text.done",
                    serde_json::json!({
                        "type": "output_text.done",
                        "index": 0,
                        "text": &self.buffer,
                    })
                    .to_string(),
                ));

                if reason != "stop" {
                    events.push((
                        "response.incomplete",
                        serde_json::json!({
                            "type": "response.incomplete",
                            "response": build_response_obj(
                                &self.resp_id, &self.msg_id, &self.model,
                                &self.buffer, "incomplete", None
                            )
                        })
                        .to_string(),
                    ));
                }
            }
        }

        events
    }

    /// SSE events that must be sent **after** all upstream chunks have been
    /// consumed.
    ///
    /// - If finish_reason arrived mid-stream (self.finished == true), we still
    ///   need output_item.done + response.completed.
    /// - If the stream ended without a finish_reason (truncation), we emit all
    ///   remaining events including response.completed with status "completed".
    pub fn final_events(&mut self) -> Vec<(&str, String)> {
        if self.finalized {
            return vec![];
        }
        self.finalized = true;

        if self.finished {
            vec![
                (
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "output_index": 0
                    })
                    .to_string(),
                ),
                (
                    "response.completed",
                    serde_json::json!({
                        "type": "response.completed",
                        "response": build_response_obj(
                            &self.resp_id, &self.msg_id, &self.model,
                            &self.buffer, "completed", self.usage.as_ref()
                        )
                    })
                    .to_string(),
                ),
            ]
        } else {
            let text = std::mem::take(&mut self.buffer);
            vec![
                (
                    "output_text.done",
                    serde_json::json!({
                        "type": "output_text.done",
                        "index": 0,
                        "text": &text,
                    })
                    .to_string(),
                ),
                (
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "output_index": 0
                    })
                    .to_string(),
                ),
                (
                    "response.completed",
                    serde_json::json!({
                        "type": "response.completed",
                        "response": build_response_obj(
                            &self.resp_id, &self.msg_id, &self.model,
                            &text, "completed", self.usage.as_ref()
                        )
                    })
                    .to_string(),
                ),
            ]
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_to_responses_basic() {
        let chat = serde_json::json!({
            "id": "chatcmpl-abc",
            "object": "chat.completion",
            "created": 1000,
            "model": "my-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello world"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20,
                "total_tokens": 30
            }
        });

        let resp = chat_to_responses(&chat, "my-model");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["model"], "my-model");
        assert_eq!(resp["status"], "completed");
        assert!(resp["id"].as_str().unwrap().starts_with("resp_"));
        assert!(resp.get("created_at").is_some(), "must have created_at");
        assert_eq!(resp["output"][0]["type"], "message");
        assert_eq!(resp["output"][0]["role"], "assistant");
        assert_eq!(resp["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(resp["output"][0]["content"][0]["text"], "Hello world");
        assert_eq!(resp["usage"]["total_tokens"], 30);
    }

    #[test]
    fn test_chat_to_responses_empty_content() {
        let chat = serde_json::json!({
            "choices": [{"message": {"content": null}}]
        });
        let resp = chat_to_responses(&chat, "m");
        assert_eq!(resp["output"][0]["content"][0]["text"], "");
        assert_eq!(resp["status"], "completed");
        assert!(resp.get("created_at").is_some());
    }

    #[test]
    fn test_responses_sse_converter_delta() {
        let mut conv = ResponsesSseConverter::new("test-model");

        let delta = serde_json::json!({"content": "Hello"});
        let events = conv.process_delta(&delta, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "output_text.delta");

        let data: Value = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(data["delta"], "Hello");
        assert_eq!(conv.buffer, "Hello");
    }

    #[test]
    fn test_responses_sse_converter_finish() {
        let mut conv = ResponsesSseConverter::new("test-model");

        let delta = serde_json::json!({"content": "Hi"});
        conv.process_delta(&delta, None);

        let events = conv.process_delta(&Value::Null, Some("stop"));
        assert_eq!(events.len(), 1, "stop reason emits only output_text.done");
        assert_eq!(events[0].0, "output_text.done");

        let done: Value = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(done["text"], "Hi");
        assert!(conv.finished);
    }

    #[test]
    fn test_responses_sse_converter_incomplete_finish() {
        let mut conv = ResponsesSseConverter::new("m");
        conv.buffer = "Partial".into();

        let events = conv.process_delta(&Value::Null, Some("length"));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "output_text.done");
        assert_eq!(events[1].0, "response.incomplete");

        let inc: Value = serde_json::from_str(&events[1].1).unwrap();
        assert_eq!(inc["type"], "response.incomplete");
        assert_eq!(inc["response"]["status"], "incomplete");
    }

    #[test]
    fn test_responses_sse_converter_initial_events() {
        let conv = ResponsesSseConverter::new("m");
        let ev = conv.initial_events();
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].0, "response.created");
        assert_eq!(ev[0].1.contains("created_at"), true);
        assert_eq!(ev[0].1.contains("in_progress"), true);
        assert_eq!(ev[1].0, "response.output_item.added");
        assert_eq!(ev[2].0, "response.content_part.added");
    }

    #[test]
    fn test_responses_sse_converter_final_events_no_finish() {
        let mut conv = ResponsesSseConverter::new("m");
        conv.buffer = "Done text".into();
        // No finish_reason seen

        let ev = conv.final_events();
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].0, "output_text.done");
        assert_eq!(ev[1].0, "response.output_item.done");
        assert_eq!(ev[2].0, "response.completed");

        let comp: Value = serde_json::from_str(&ev[2].1).unwrap();
        assert_eq!(comp["type"], "response.completed");
        assert_eq!(comp["response"]["status"], "completed");
        assert_eq!(comp["response"]["output"][0]["content"][0]["text"], "Done text");
        assert!(comp["response"].get("created_at").is_some());
    }

    #[test]
    fn test_responses_sse_converter_final_events_with_finish() {
        let mut conv = ResponsesSseConverter::new("m");
        conv.buffer = "Hello".into();
        conv.finished = true;

        let ev = conv.final_events();
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].0, "response.output_item.done");
        assert_eq!(ev[1].0, "response.completed");

        let comp: Value = serde_json::from_str(&ev[1].1).unwrap();
        assert_eq!(comp["type"], "response.completed");
        assert_eq!(comp["response"]["status"], "completed");
        assert_eq!(comp["response"]["output"][0]["content"][0]["text"], "Hello");
    }

    #[test]
    fn test_responses_sse_converter_double_finalize_noop() {
        let mut conv = ResponsesSseConverter::new("m");
        conv.finished = true;
        let ev1 = conv.final_events();
        assert_eq!(ev1.len(), 2); // output_item.done + response.completed
        let ev2 = conv.final_events();
        assert_eq!(ev2.len(), 0); // already finalized
    }

    // ── Helper function tests ────────────────────────────────────────

    #[test]
    fn test_extract_text_from_string() {
        assert_eq!(extract_text_from_content(&Value::String("hello".into())), "hello");
        assert_eq!(extract_text_from_content(&Value::String("".into())), "");
    }

    #[test]
    fn test_extract_text_from_array() {
        let content = serde_json::json!([
            {"type": "input_text", "text": "Hello"},
            {"type": "input_text", "text": "World"}
        ]);
        assert_eq!(extract_text_from_content(&content), "Hello World");
    }

    #[test]
    fn test_extract_text_from_empty_array() {
        let content = serde_json::json!([]);
        assert_eq!(extract_text_from_content(&content), "");
    }

    #[test]
    fn test_extract_text_from_null() {
        assert_eq!(extract_text_from_content(&Value::Null), "");
    }

    #[test]
    fn test_normalize_message_developer_to_system() {
        let msg = serde_json::json!({
            "role": "developer",
            "content": [{"type": "input_text", "text": "Be helpful."}]
        });
        let norm = normalize_message(&msg);
        assert_eq!(norm["role"], "system");
        assert_eq!(norm["content"], "Be helpful.");
    }

    #[test]
    fn test_normalize_message_user_stays_user() {
        let msg = serde_json::json!({"role": "user", "content": "hello"});
        let norm = normalize_message(&msg);
        assert_eq!(norm["role"], "user");
        assert_eq!(norm["content"], "hello");
    }

    #[test]
    fn test_normalize_message_content_array_to_text() {
        let msg = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "input_text", "text": "What is"}
            ]
        });
        let norm = normalize_message(&msg);
        assert_eq!(norm["content"], "What is");
    }

    #[test]
    fn test_strip_responses_fields_removes_input() {
        let mut body = serde_json::json!({
            "input": "test",
            "instructions": "be nice",
            "model": "gpt-4",
            "messages": [],
            "stream": false,
        });
        strip_responses_fields(&mut body);
        assert!(body.get("input").is_none(), "input should be removed");
        assert!(body.get("instructions").is_none(), "instructions should be removed");
        assert!(body.get("model").is_some(), "model should remain");
        assert!(body.get("messages").is_some(), "messages should remain");
    }

    #[test]
    fn test_strip_responses_fields_removes_all_known_fields() {
        let mut body = serde_json::json!({
            "input": "x",
            "instructions": "x",
            "include": [],
            "previous_response_id": "resp_x",
            "store": false,
            "stream_options": {},
            "client_metadata": {},
            "prompt_cache_key": "x",
            "reasoning": {},
            "tools": [{"type": "web_search_preview"}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "model": "m",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 100,
        });
        strip_responses_fields(&mut body);
        for field in &["input", "instructions", "include", "previous_response_id",
                       "store", "stream_options", "client_metadata", "prompt_cache_key",
                       "reasoning", "tools", "tool_choice", "parallel_tool_calls"] {
            assert!(body.get(field).is_none(), "{} should be removed", field);
        }
        assert!(body.get("model").is_some());
        assert!(body.get("messages").is_some());
        assert!(body.get("max_tokens").is_some());
    }
}
