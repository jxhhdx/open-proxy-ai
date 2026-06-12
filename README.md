# Open Proxy AI

> **A desktop app** that exposes free AI models as standard OpenAI and Anthropic APIs, with a built-in model pool, speed testing, and automatic failover.

[中文文档](README.zh.md)

---

## ✨ Features

- **🖥️ Desktop App** — Built with Tauri + React, no terminal needed. Double-click to run.
- **🌐 API Proxy** — OpenAI (`/v1/chat/completions`) and Anthropic (`/v1/messages`) formats.
- **🔀 Model Pool** — Auto-failover: if one model fails, try the next by priority.
- **⚡ Speed Test** — One-click batch latency and throughput testing.
- **🔌 Import to Tools** — One-click export to Claude Code, Codex, or CCSwitch.
- **🌙 Theme** — Dark, Light, or System-following. English/Chinese UI.

## 🚀 Quick Start

### Download

Download the latest `.dmg` from [Releases](https://github.com/jxhhdx/open-proxy-ai/releases).

### Build from source

```bash
git clone https://github.com/jxhhdx/open-proxy-ai.git
cd open-proxy-ai

# Install dependencies
npm install

# Run in development mode
cargo tauri dev

# Build production .app
cargo tauri build
```

## 🎯 Usage

Open the app → server starts automatically on `http://localhost:6446`.

### Dashboard

| Section | Description |
|---------|-------------|
| **API Keys** | Auto-generated keys, click to copy |
| **Model Pool** | Enable/disable models, drag to reorder priority |
| **Speed Test** | Test all models at once, view latency & tokens/sec |
| **Import Pool** | Export config to Claude Code / Codex / CCSwitch |
| **Settings** | Language toggle (中文/English), theme (Dark/Light/System) |

### Available Models

| Model | Type | Reliability |
|-------|------|-------------|
| `deepseek-v4-flash-free` | Free | ✅ Solid |
| `big-pickle` | Free (alias) | ✅ Solid |
| `minimax-m2.5-free` | Free | ⚠️ Intermittent |
| `nemotron-3-super-free` | Free | ⚠️ Hit or miss |
| `qwen3.6-plus-free` | Free | ❌ Ended |

You can also add **custom providers** with your own API URL and key.

## 🔧 API Endpoints

Once the app is running:

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/chat/completions` | OpenAI format |
| `POST` | `/v1/messages` | Anthropic format |
| `GET` | `/v1/models` | List models |
| `GET` | `/health` | Health check |

### curl example

```bash
curl http://localhost:6446/v1/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek-v4-flash-free","messages":[{"role":"user","content":"Hello"}]}'
```

## 🏗️ Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 |
| Frontend | React 18 + TypeScript |
| Backend | Rust (axum, reqwest, tokio) |
| Drag & Drop | @dnd-kit/sortable |
| Building | Vite |

## 📄 License

MIT
