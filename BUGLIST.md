# Bug List

> 记录本项目的 bug 及修复方式。每个 bug 修复后必须写单元测试来防止复发。

---

## BUG-001: Nemotron 模型名过期导致不可用

- **发现日期**: 2026-06-13
- **严重程度**: 高（模型完全不可用）
- **状态**: 已修复

### 现象

用户报告第三个英伟达模型（`nemotron-3-super-free`）无法使用。调用 Zen API 返回 401 错误。

### 根因

OpenCode 的 Zen API 将免费 Nemotron 模型从 `nemotron-3-super-free` 重命名为 `nemotron-3-ultra-free`。内置模型列表未同步更新。

### 连带发现

同步调查发现还有两个内置模型的免费推广已结束，同样不可用：

| 旧模型名 | 原因 |
|---|---|
| `minimax-m2.5-free` | MiniMax M3 Free 推广已结束 |
| `qwen3.6-plus-free` | Qwen3.6 Plus Free 推广已结束 |

替代为 `north-mini-code-free` 和 `mimo-v2.5-free`（已验证可用）。

### 修复

- **MODELS 列表** (`src-tauri/src/proxy/server.rs`): 更新模型名
- **ModelPool 迁移** (`src-tauri/src/proxy/model_pool.rs`): 新增 `migrate_renamed_builtins()` 方法，自动将旧 ID 映射到新 ID，并更新 `deleted_builtins`
- **启动逻辑** (`src-tauri/src/lib.rs`): `init_builtins` 改为每次启动都刷新内置模型

### 如何防止复发

1. 新增 `migrate_renamed_builtins()` + `init_builtins()` 组合逻辑，每次启动刷新内置模型名
2. 单元测试覆盖了迁移场景（`test_migrate_renamed_builtins_updates_entry_id` 等）
3. 后续可通过集成测试（`#[ignore]` + 网络请求）定期验证所有模型名在 Zen API 上的可用性

---

## BUG-002: custom provider 请求参数未透传导致模型返回空响应

- **发现日期**: 2026-06-13
- **严重程度**: 高（Custom provider 发送给 API 的请求缺少 `max_tokens` 等关键参数）
- **状态**: 已修复

### 现象

Custom provider `minimaxai/minimax-m3`（NVIDIA API）调用返回 HTTP 200 但 `choices` 数组为空。模型无法正常响应。

### 根因

`ZenClient::build_request_body()` 构造请求体时只保留了 `model`、`messages`、`stream`、`tools` 四个字段，丢弃了客户端传入的 `max_tokens`、`temperature`、`top_p` 等参数。

NVIDIA API 要求显式传入 `max_tokens`，否则返回空 `choices`。

### 修复

- **参数透传** (`src-tauri/src/proxy/zen.rs`): `build_request_body()` 新增 `extra` 参数，将 `max_tokens`、`temperature`、`top_p`、`stop`、`frequency_penalty`、`presence_penalty`、`seed`、`response_format` 从原始请求体复制到新请求体
- **调用点更新** (`src-tauri/src/proxy/server.rs`): 所有 3 处调用点传入 `extra` 参数

### 如何防止复发

1. `ZenClient::build_request_body()` 的 `extra` 参数显式声明了透传字段白名单
2. 单元测试覆盖了参数透传行为：
   - `test_build_request_body_passes_through_max_tokens`
   - `test_build_request_body_without_extra_preserves_standard`
   - `test_build_request_body_passes_through_tools`
   - `test_build_request_body_ignores_unknown_keys_in_extra`

---

## BUG-003: Anthropic → OpenAI 请求转换丢失 max_tokens 导致自定义提供者返回空响应

- **发现日期**: 2026-06-13
- **严重程度**: 高（Anthropic 格式请求转 OpenAI 格式时，NVIDIA 等自定义提供者返回空 choices）
- **状态**: 已修复

### 现象

通过 `/v1/messages`（Anthropic 格式）请求自定义提供者 `minimaxai/minimax-m3`（NVIDIA API），接口返回 200 OK 但 `choices` 为空数组，导致客户端收不到回复。DeepSeek 等 `anthropic` 格式的提供者不受影响。

### 根因

`messages_handler` 中调用 `build_request_body()` 时传了 `None`（无原始 body），而不是把原始 Anthropic 请求体传进去。结果 body 从零构建，只保留了 `model`、`messages`、`stream`，所有 Anthropic 请求中的参数（`max_tokens`、`temperature` 等）全部丢失。

NVIDIA API 要求显式传入 `max_tokens`，否则返回空 `choices`。

### 历史关联

**BUG-002** 修的是 OpenAI 格式路径（`/v1/chat/completions`）同样的问题——`build_request_body()` 当时用了白名单复制参数会遗漏。那次把 OpenAI 路径修复为克隆原始 body。

但 Anthropic 路径（**这次**的 BUG-003）当时传了 `None`，完全绕过了克隆逻辑，所以 BUG-002 的修复对它无效。

### 修复

- **`server.rs`**: `messages_handler` 中调用 `build_request_body(m, msgs, stream, tools, Some(&body))` 而不是 `None`
- **`zen.rs`**: `build_request_body` 克隆 body 路径增加 tools 处理（替换 Anthropic→OpenAI 转换后的 tools）

### 如何防止复发

1. 单元测试 `test_with_original_body_anthropic_format_preserves_max_tokens` 覆盖了此场景
2. 两个路径（OpenAI 和 Anthropic）现在都用了 `Some(&body)` 克隆方式

---

## BUG-004: 日志时间戳显示 UTC 时间而非本地时间

- **发现日期**: 2026-06-13
- **严重程度**: 中（日志时间与实际时间相差 8 小时，影响调试体验）
- **状态**: 已修复

### 现象

Settings 页面日志显示的 `time` 是 UTC 时间（类似英国时间）。用户在北京时区（UTC+8），日志中的 `09:40:35` 实际应该是北京时间 `17:40:35`。

### 根因

`src-tauri/src/proxy/log.rs` 中 `fmt_time()` 函数直接对 UNIX epoch 秒数取模计算时/分/秒，没有加本地时区偏移：

```rust
let secs = d.as_secs() % 86400;  // 纯 UTC
```

### 修复

- 使用 `chrono::Local::now()` 获取本地时间
- `src-tauri/Cargo.toml` 添加 `chrono = "0.4"` 依赖

### 如何防止复发

1. 单元测试 `test_log_entry_has_local_time` 验证日志时间格式为 `HH:MM:SS`
2. 单元测试 `test_log_respects_max_entries` 验证日志环形缓冲区行为

---

## BUG-005: `/v1/responses` 返回 Chat Completions 格式而非 Responses API 格式

- **发现日期**: 2026-06-13
- **严重程度**: 高（Codex 直接报错 404 / stream disconnected）
- **状态**: 已修复

### 现象

Codex (v0.139.0) 调用 `/v1/responses` 时：
1. 首次请求返回 `404 Not Found`（路由不存在的问题在之前的 commit 已修复）
2. 后续请求返回 `stream disconnected before completion: stream closed before response.completed`

### 根因

`responses_handler` 将 Responses API 请求（`input`、`max_output_tokens`）转换后通过 `route_chat_completion` 发送，但**响应的数据格式仍然是 Chat Completions 格式**（`choices[].message.content`），而 Codex 期望的是 **Responses API 格式**（`output[].content[].text`、SSE 事件 `response.completed` 等）。

非流式场景返回了 Chat Completions JSON → Codex 解析失败。流式场景返回了 Chat Completions SSE（`data: {...}\n\n`）→ Codex 没看到 `response.completed` 事件 → 报 `stream disconnected`。

### 修复

新增 `src-tauri/src/proxy/responses.rs` 模块，包含：

1. **`chat_to_responses()`** — 非流式转换：将 Chat Completions JSON 转为 Responses API JSON（`object: "response"`、`output[]` 结构）
2. **`ResponsesSseConverter`** — 流式 SSE 转换器，将 Chat Completions SSE 转为 Responses API SSE 事件：
   - `response.created` → `response.output_item.added` → `response.content_part.added` → `response.output_text.delta` → `response.output_text.done` → `response.completed`
   - 非 `stop` 的 finish_reason 附加 `response.incomplete` 事件

修改 `responses_handler`，不再委托给 `route_chat_completion`，而是自己处理完整流程（模型池路由 → 发送请求 → 格式转换 → 返回）。

### 如何防止复发

1. 单元测试覆盖了 Responses API 格式转换：
   - `test_chat_to_responses_basic` — 非流式转换验证
   - `test_chat_to_responses_empty_content` — 空响应处理
   - `test_responses_sse_converter_delta` — SSE delta 事件
   - `test_responses_sse_converter_finish` — SSE finish/stop
   - `test_responses_sse_converter_incomplete_finish` — 非 stop 的 incomplete 事件
   - `test_responses_sse_converter_initial_events` — 初始事件
   - `test_responses_sse_converter_final_events` — 结束事件
   - `test_responses_sse_converter_double_finish_noop` — 防重复结束
2. 端到端测试 `test_bugs.sh` 验证了 `/v1/responses` 的响应格式（object=response、output[].type=message）
3. 流式 SSE 格式验证（包含 response.created / output_text.delta / response.completed 事件）

---

## Bug 提交流程

1. 发现 bug → 在此文件新增条目
2. 分析根因 → 填写 `根因` 和 `连带发现`
3. 修复 → 填写 `修复` 章节
4. 写单元测试 → 在 `如何防止复发` 中引用测试名
5. 将状态改为 `已修复`
