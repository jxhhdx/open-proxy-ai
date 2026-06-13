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
## Bug 提交流程

1. 发现 bug → 在此文件新增条目
2. 分析根因 → 填写 `根因` 和 `连带发现`
3. 修复 → 填写 `修复` 章节
4. 写单元测试 → 在 `如何防止复发` 中引用测试名
5. 将状态改为 `已修复`
