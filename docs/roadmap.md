# Open Proxy AI — 核心目标与规划

## 核心价值主张

**万能适配器**：用户不管手里是 OpenAI key、Anthropic key、Google Gemini key 还是任何兼容 API 的 key，都能加进同一个模型池，按优先级排序、故障转移。池子同时暴露 OpenAI（`/v1/chat/completions`）和 Anthropic（`/v1/messages`）双协议出口，让 Claude Code 和 Code X 这类工具即插即用。

## 现状 vs 目标

| 项目 | 现状 | 差距 |
|---|---|---|
| OpenAI key → 池化 | ✅ 完成 | — |
| Anthropic key → 池化（直接转发 + 格式转换） | ✅ 完成 | — |
| **Google/Gemini key → 池化** | ❌ **不支持** | 没有 Google API 格式的转换层 |
| 双协议出口（OpenAI ↔ Anthropic 互转） | ✅ 完成（`anthropic.rs`） | — |
| 模型池热重载（RwLock 实时生效） | ✅ 基本完成 | 需验证进行中请求是否受影响 |
| 故障转移（优先级循环 + 自动重试） | ✅ 完成 | — |
| 内置免费模型（DeepSeek 等 5 个） | ✅ 完成 | — |
| 自定义提供商（任意 base_url） | ✅ 完成 | — |
| MiMo 自动检测 | ✅ 完成 | — |

## 规划功能

### P0 — Google Gemini API 支持（核心目标缺口）

在模型池中添加 `api_format: "google"` 支持，让用户可以将 Google API key 加入池子。

**现有方案**：使用 [`gproxy-protocol`](https://crates.io/crates/gproxy-protocol) crate（v1.0.20，MIT/Apache2），它提供纯 serde 数据类型的双向转换：
- `GenerateContent` ↔ `Chat Completions`（请求 + 响应双向）
- `StreamGenerateContent` ↔ SSE 流式转换
- Embeddings、Model list、Count tokens
- 零 HTTP 依赖，纯类型转换

**实现思路**：
1. 在 `ModelPoolEntry` 的 `api_format` 中增加 `"google"` 枚举值
2. 在 `server.rs` 的路由处理中，当池条目的 `api_format == "google"` 时：
   - 用 `gproxy_protocol::transform::dispatch::transform_request()` 将 OpenAI 格式请求体转为 Gemini `generateContent` 格式
   - 发送到 `https://generativelanguage.googleapis.com/v1/models/{model}:generateContent`（或 streamGenerateContent）
   - 用 `gproxy_protocol::transform::dispatch::transform_response()` 将响应转回 OpenAI 格式
3. Anthropic 请求路径：`Anthropic → OpenAI（已有 anthropic.rs）→ Google（gproxy-protocol）`
4. 需要创建 `google.rs` 模块，类似 `zen.rs` 的结构

**备选方案**：
- [`llm_adapter`](https://crates.io/crates/llm_adapter)（v0.2.7）：统一 core model + 协议 codec，也支持 Gemini，但更早期
- [`aigateway`](https://crates.io/crates/aigateway)（v0.5.0）：协议保真的多提供商网关，但可能更重量级

**推荐用 `gproxy-protocol`**，原因：最成熟（v1.0.20）、纯类型转换无副作用、正好匹配我们的"收到 OpenAI 请求→转换→发到提供商→转回"模式。

### P1 — 模型池无感热重载优化

**现状分析：** 当前实现已经每次都做池快照（`pool.get_enabled()` 返回 `Vec<&ModelPoolEntry>`，路由代码再 clone 到本地 Vec），所以**单个请求的 failover 环路不会受并发修改的影响**。但存在以下潜在问题：

#### 问题 1：池操作与请求之间存在写锁争用

Tauri 命令（`toggle_pool_entry`、`reorder_pool` 等）获取 `model_pool` 的**写锁**期间，所有并发 HTTP 请求的读锁被阻塞。写操作持有锁的时间 = 修改内存 + `serde_json::to_string_pretty` + `std::fs::write`（磁盘 I/O）。

**影响**：快速连续操作 UI（如拖动重排）时，短暂的锁争用可能导致请求延迟毛刺。

**方案**：将 `save()` 移出写锁范围——先用 `.clone()` 获取池的快照，释放写锁，再在快照上做 `save()`。或者用 `RwLock<Arc<ModelPool>>` 的 RCU 模式。

#### 问题 2：没有 draining 机制（优雅关闭）

当用户禁用（`enabled = false`）或删除一个池条目时：

- **新请求**：正确跳过该条目
- **进行中的流式请求**：虽然 HTTP 连接已经建立（连接到上游），不会中断，但如果此时删除条目（`remove_pool_entry`），`active_model_id` 会被设置为已删除的 ID（行 1034、1180），导致 UI 上显示"最后活跃模型"指向不存在的条目
- 更严重的是：如果条目包含 API key 等敏感信息，删除后理论上应该确保没有请求还在使用它

**方案**：给 `ModelPoolEntry` 加两个字段：
```rust
pub active_connections: Arc<AtomicU32>,  // 当前活跃请求数
pub draining: bool,                       // 标记为"排空中，不再接受新请求"
```
- 禁用/删除 → 设 `draining = true`，不再接受新请求
- 存活的流式请求完成后，`active_connections` 归零
- 当 `draining && active_connections == 0` 时才真正移除

#### 问题 3：池快照逻辑三倍重复

`route_chat_completion()`（行 978-1014）、`messages_handler()`（行 1106-1139）、`responses_handler()`（行 837-866）三份完全相同的池迭代 + 克隆逻辑。任何热重载改进都要改三处。

**方案**：抽取公共方法，如 `fn snapshot_pool(pool: &ModelPool, model: &str) -> PoolSnapshot`。

#### 问题 4：原生与自定义提供商标识不清

`responses_handler` 行 884-918 在 failover 循环内判断 `base_url.is_empty()` 来区分"是否自定义提供商"。但实际逻辑上，当池中既有内置 opencode 模型又有自定义提供商时，这是对的。问题是在三处代码里，这个判断方式略有差异，存在不一致风险。

**方案**：整合到 `PoolSnapshot` 结构中，统一处理。

### P1 — 模型实时状态指示器

每个模型条目显示一个状态圆点：
- **绿灯闪烁** — 该模型当前正在被使用（有请求正在通过该模型处理）
- **灰点** — 该模型空闲，未被使用

需要实现：
- 后端：跟踪每个模型当前的活跃请求数，暴露状态接口
- 前端：轮询或 SSE 实时更新圆点状态，绿灯用 CSS animation 闪烁

### P2 — Token 用量统计

- 跟踪**每个模型**消耗的 token 数（输入 + 输出）
- 跟踪**总计** token 消耗
- 在 UI 中展示（可能是每个模型行旁边 + 总览区域）

需要实现：
- 后端：在代理转发请求时解析 usage 字段，累计到模型维度的计数器（内存中或持久化）
- 前端：展示每个模型的 token 用量和总量
