# Open Proxy AI

> **一款桌面应用**，将免费 AI 模型暴露为标准 OpenAI 和 Anthropic API，内置模型号池、测速和自动切换功能。

[English Documentation](README.md)

---

## ✨ 功能特性

- **🖥️ 桌面应用** — 基于 Tauri + React 构建，双击即用，无需命令行。
- **🌐 API 代理** — 支持 OpenAI（`/v1/chat/completions`）和 Anthropic（`/v1/messages`）格式。
- **🔀 模型号池** — 自动故障切换：当前模型失败后按优先级尝试下一个。
- **⚡ 一键测速** — 批量测试所有模型的延迟和生成速度。
- **🔌 导入外部工具** — 一键导出配置到 Claude Code、Codex 或 CCSwitch。
- **🌙 主题切换** — 深色/浅色/跟随系统。中英文界面。

## 🚀 快速开始

### 下载

从 [Releases](https://github.com/jxhhdx/open-proxy-ai/releases) 页面下载最新的 `.dmg` 安装包。

### 从源码构建

```bash
git clone https://github.com/jxhhdx/open-proxy-ai.git
cd open-proxy-ai

# 安装依赖
npm install

# 开发模式运行
cargo tauri dev

# 构建正式版 .app
cargo tauri build
```

## 🎯 使用说明

打开应用 → 服务器自动在 `http://localhost:6446` 启动。

### 仪表盘

| 模块 | 说明 |
|------|------|
| **API 密钥** | 自动生成的 Key，点击复制 |
| **模型号池** | 启用/禁用模型，拖拽调整优先级 |
| **一键测速** | 批量测试所有模型，查看延迟和 tok/s |
| **导入号池** | 导出配置到 Claude Code / Codex / CCSwitch |
| **设置** | 语言切换（中文/English）、主题（深色/浅色/跟随系统） |

### 可用模型

| 模型 | 类型 | 稳定性 |
|-------|------|--------|
| `deepseek-v4-flash-free` | 免费 | ✅ 稳定 |
| `big-pickle` | 免费（别名） | ✅ 稳定 |
| `minimax-m2.5-free` | 免费 | ⚠️ 偶尔不可用 |
| `nemotron-3-super-free` | 免费 | ⚠️ 不太稳定 |
| `qwen3.6-plus-free` | 免费 | ❌ 已失效 |

你也可以**添加自定义提供商**，填入自己的 API 地址和 Key。

## 🔧 API 接口

应用启动后可用：

| 方法 | 路径 | 说明 |
|--------|------|------|
| `POST` | `/v1/chat/completions` | OpenAI 格式 |
| `POST` | `/v1/messages` | Anthropic 格式 |
| `GET` | `/v1/models` | 模型列表 |
| `GET` | `/health` | 健康检查 |

### curl 示例

```bash
curl http://localhost:6446/v1/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek-v4-flash-free","messages":[{"role":"user","content":"你好"}]}'
```

## 🏗️ 技术栈

| 层 | 技术 |
|-------|-----------|
| 桌面框架 | Tauri 2 |
| 前端 | React 18 + TypeScript |
| 后端 | Rust（axum、reqwest、tokio） |
| 拖拽排序 | @dnd-kit/sortable |
| 构建 | Vite |

## 📄 许可证

MIT
