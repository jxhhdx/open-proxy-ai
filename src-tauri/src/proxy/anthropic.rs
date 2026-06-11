use serde_json::{json, Value};

/// Convert an Anthropic-format request body to OpenAI-format messages and tools.
pub fn anthropic_to_openai(body: &Value) -> (Vec<Value>, Option<Vec<Value>>) {
    let mut messages = Vec::new();

    // System prompt
    if let Some(system) = body.get("system") {
        let sys_text = match system {
            Value::String(s) => s.clone(),
            Value::Array(arr) => arr
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if !sys_text.is_empty() {
            messages.push(json!({"role": "system", "content": sys_text}));
        }
    }

    // Messages
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("user");
            let content = msg.get("content");

            match content {
                Some(Value::String(text)) => {
                    messages.push(json!({"role": role, "content": text}));
                }
                Some(Value::Array(blocks)) => {
                    let text: String = blocks
                        .iter()
                        .filter(|b| b.get("type") == Some(&json!("text")))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let tool_uses: Vec<&Value> = blocks
                        .iter()
                        .filter(|b| b.get("type") == Some(&json!("tool_use")))
                        .collect();

                    let tool_results: Vec<&Value> = blocks
                        .iter()
                        .filter(|b| b.get("type") == Some(&json!("tool_result")))
                        .collect();

                    if !tool_uses.is_empty() && role == "assistant" {
                        let tool_calls: Vec<Value> = tool_uses
                            .iter()
                            .map(|t| {
                                json!({
                                    "id": t.get("id"),
                                    "type": "function",
                                    "function": {
                                        "name": t.get("name"),
                                        "arguments": serde_json::to_string(&t.get("input").unwrap_or(&json!({}))).unwrap_or_default()
                                    }
                                })
                            })
                            .collect();

                        messages.push(json!({
                            "role": "assistant",
                            "content": if text.is_empty() { Value::Null } else { json!(text) },
                            "tool_calls": tool_calls
                        }));
                    } else if !tool_results.is_empty() {
                        for tr in tool_results {
                            let result_content = tr
                                .get("content")
                                .and_then(|c| match c {
                                    Value::String(s) => Some(s.clone()),
                                    Value::Array(arr) => Some(
                                        arr.iter()
                                            .filter_map(|b| {
                                                b.get("text").and_then(|t| t.as_str())
                                            })
                                            .collect::<Vec<_>>()
                                            .join("\n"),
                                    ),
                                    _ => None,
                                })
                                .unwrap_or_default();

                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tr.get("tool_use_id"),
                                "content": result_content
                            }));
                        }
                    } else {
                        messages.push(json!({"role": role, "content": text}));
                    }
                }
                _ => {}
            }
        }
    }

    // Tools
    let tools: Option<Vec<Value>> = body
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|tools| {
            tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.get("name"),
                            "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                            "parameters": t.get("input_schema").unwrap_or(&json!({}))
                        }
                    })
                })
                .collect()
        });

    (messages, tools.filter(|t| !t.is_empty()))
}

/// Map OpenAI finish_reason to Anthropic stop_reason.
fn map_stop_reason(finish_reason: Option<&str>) -> &str {
    match finish_reason {
        Some("stop") => "end_turn",
        Some("tool_calls") => "tool_use",
        Some("length") => "max_tokens",
        _ => "end_turn",
    }
}

/// Convert an OpenAI non-streaming response to Anthropic format.
pub fn openai_to_anthropic(
    oai: &Value,
    model: &str,
    input_tokens: u64,
) -> Value {
    let choice = oai.get("choices").and_then(|c| c.get(0));
    let msg = choice.and_then(|c| c.get("message"));

    let mut content: Vec<Value> = Vec::new();

    // Text content
    if let Some(text) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
        if !text.is_empty() {
            content.push(json!({"type": "text", "text": text}));
        }
    }

    // Tool calls
    if let Some(tcs) = msg.and_then(|m| m.get("tool_calls")).and_then(|t| t.as_array()) {
        for tc in tcs {
            let mut input = json!({});
            if let Some(args) = tc.pointer("/function/arguments").and_then(|a| a.as_str()) {
                serde_json::from_str::<Value>(args).ok().map(|v| input = v);
            }
            content.push(json!({
                "type": "tool_use",
                "id": tc.get("id"),
                "name": tc.pointer("/function/name"),
                "input": input
            }));
        }
    }

    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }

    let oai_usage = oai.get("usage");
    let stop_reason = choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str());

    json!({
        "id": format!("msg_{:x}", rand::random::<u64>()),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": model,
        "stop_reason": map_stop_reason(stop_reason),
        "usage": {
            "input_tokens": oai_usage.and_then(|u| u.get("prompt_tokens")).and_then(|t| t.as_u64()).unwrap_or(input_tokens),
            "output_tokens": oai_usage.and_then(|u| u.get("completion_tokens")).and_then(|t| t.as_u64()).unwrap_or(0),
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0
        }
    })
}

/// Streaming state for converting OpenAI SSE → Anthropic SSE on-the-fly.
#[allow(dead_code)]
pub struct AnthropicStreamConverter {
    pub msg_id: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub text_block_started: bool,
    pub tool_count: i32,
    pub tool_open_idx: Option<i32>,
}

impl AnthropicStreamConverter {
    pub fn new(msg_id: String, model: String, input_tokens: u64) -> Self {
        Self {
            msg_id,
            model,
            input_tokens,
            output_tokens: 0,
            text_block_started: false,
            tool_count: 0,
            tool_open_idx: None,
        }
    }

    /// Process one OpenAI delta chunk and return Anthropic SSE event strings.
    /// Returns Vec of (event_name, data_json_string).
    pub fn process_delta(
        &mut self,
        delta: &Value,
        finish_reason: Option<&str>,
    ) -> Vec<(String, String)> {
        let mut events: Vec<(String, String)> = Vec::new();

        // Text delta
        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                if !self.text_block_started {
                    self.text_block_started = true;
                    events.push((
                        "content_block_start".into(),
                        serde_json::to_string(&json!({
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {"type": "text", "text": ""}
                        }))
                        .unwrap_or_default(),
                    ));
                }
                events.push((
                    "content_block_delta".into(),
                    serde_json::to_string(&json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {"type": "text_delta", "text": text}
                    }))
                    .unwrap_or_default(),
                ));
                self.output_tokens += (text.len() / 4).max(1) as u64;
            }
        }

        // Tool call deltas
        if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tcs {
                let idx = tc.get("index").and_then(|i| i.as_i64()).unwrap_or(0) as i32;

                if idx != self.tool_open_idx.unwrap_or(-1) {
                    // Close previous tool block if open
                    if let Some(prev) = self.tool_open_idx {
                        events.push((
                            "content_block_stop".into(),
                            serde_json::to_string(&json!({
                                "type": "content_block_stop",
                                "index": if self.text_block_started { prev + 1 } else { prev }
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                    self.tool_open_idx = Some(idx);
                    self.tool_count += 1;

                    let block_idx = if self.text_block_started {
                        idx + 1
                    } else {
                        idx
                    };

                    let tool_name = tc
                        .pointer("/function/name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let tool_id = tc
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("");

                    events.push((
                        "content_block_start".into(),
                        serde_json::to_string(&json!({
                            "type": "content_block_start",
                            "index": block_idx,
                            "content_block": {
                                "type": "tool_use",
                                "id": tool_id,
                                "name": tool_name
                            }
                        }))
                        .unwrap_or_default(),
                    ));
                }

                if let Some(args) = tc
                    .pointer("/function/arguments")
                    .and_then(|a| a.as_str())
                {
                    if !args.is_empty() {
                        let block_idx = if self.text_block_started {
                            idx + 1
                        } else {
                            idx
                        };
                        events.push((
                            "content_block_delta".into(),
                            serde_json::to_string(&json!({
                                "type": "content_block_delta",
                                "index": block_idx,
                                "delta": {
                                    "type": "input_json_delta",
                                    "partial_json": args
                                }
                            }))
                            .unwrap_or_default(),
                        ));
                        self.output_tokens += (args.len() / 4).max(1) as u64;
                    }
                }
            }
        }

        // Finish
        if let Some(fr) = finish_reason {
            let total_blocks = (if self.text_block_started { 1 } else { 0 })
                + self.tool_count;
            for i in 0..total_blocks {
                events.push((
                    "content_block_stop".into(),
                    serde_json::to_string(&json!({
                        "type": "content_block_stop",
                        "index": i
                    }))
                    .unwrap_or_default(),
                ));
            }
            events.push((
                "message_delta".into(),
                serde_json::to_string(&json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": map_stop_reason(Some(fr))},
                    "usage": {"output_tokens": self.output_tokens}
                }))
                .unwrap_or_default(),
            ));
            events.push((
                "message_stop".into(),
                serde_json::to_string(&json!({"type": "message_stop"}))
                    .unwrap_or_default(),
            ));
        }

        events
    }
}
