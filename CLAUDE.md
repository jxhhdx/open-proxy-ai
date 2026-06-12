# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Install dependencies
npm install

# Dev mode (Vite dev server + Tauri window)
cargo tauri dev

# Production build (.app/.dmg)
cargo tauri build

# Run frontend-only dev server (no Tauri window)
npx vite

# Check Rust compilation
cargo check
```

There are no tests or lint scripts configured in this project yet.

## Project Overview

**Open Proxy AI** — a Tauri 2 desktop app that exposes free AI models (OpenCode's Zen API + custom providers) as standard OpenAI and Anthropic API endpoints, with a model pool for priority-based failover routing.

### Architecture

```
src/                  # React 19 + TypeScript frontend (Vite 8, Tailwind 4)
├── App.tsx           # Root: fetches status/pool, renders all sections
├── components/       # UI sections: ApiKeys, ModelPool, Settings, Header, AddProviderDialog, Toast
├── hooks/useTauri.ts # Communicates with Rust backend via Tauri invoke()
├── i18n/             # Chinese + English translations with context providers
└── types.ts          # Shared TypeScript types matching Rust structs

src-tauri/src/        # Rust backend (axum + reqwest + tokio)
├── main.rs           # Entry point, calls lib::run()
├── lib.rs            # Tauri app setup: commands, tray, server lifecycle
└── proxy/
    ├── mod.rs
    ├── server.rs     # Axum HTTP router: /v1/chat/completions, /v1/messages, /v1/models, /health
    ├── anthropic.rs  # OpenAI ↔ Anthropic format conversion (request + streaming response)
    ├── auth.rs       # API key authentication (admin + user-default keys)
    ├── model_pool.rs # Model pool data model: entries with priority, enable/disable, failover
    ├── zen.rs        # OpenCode Zen API client + session manager (30-min rotation)
    └── log.rs        # In-memory ring buffer log (200 entries)
```

### Data Flow

1. **Frontend** (`src/`) renders React components that call Tauri commands via `invoke()` — e.g. `get_status`, `get_model_pool`, `run_speed_test_cmd`
2. **Tauri Commands** (`lib.rs`) handle these by reading/writing shared `ProxyState` (wrapped in `Arc<RwLock<>>`) and the on-disk config directory (`~/.config/open-proxy-ai/`)
3. **HTTP Server** (`server.rs`) runs an Axum router on port 6446 that:
   - Authenticates via `Authorization: Bearer` or `x-api-key`
   - Routes `model: "ModelPool"` through all enabled pool entries by priority (failover on error)
   - Routes specific model names through the matching pool entry or directly to Zen API
   - Converts Anthropic-format requests to OpenAI before sending, and converts responses back
4. **Zen API** (`zen.rs`) talks to `https://opencode.ai/zen/v1/chat/completions` with session-based headers

### Key Patterns

- **Failover routing**: when `model: "ModelPool"` is requested, the server iterates enabled pool entries sorted by priority. On any error (HTTP 4xx/5xx, timeout), it logs and tries the next entry.
- **Format conversion** (`anthropic.rs`): translates Anthropic `/v1/messages` requests to OpenAI format, sends through Zen API, and converts streaming SSE responses back on-the-fly (including tool calls).
- **Persistence**: config files live in `~/.config/open-proxy-ai/` — `api-keys.json`, `model_pool.json`, `custom_models.json`. Auto-generated on first launch.
- **Session management**: Zen API sessions rotate every 30 minutes per user, stored in-memory.
- **System tray**: close-to-tray behavior with Show/Quit context menu.

### Built-in Models (defined in `server.rs`)

`deepseek-v4-flash-free`, `big-pickle`, `minimax-m2.5-free`, `nemotron-3-super-free`, `qwen3.6-plus-free`

### Custom Providers

Users can add custom providers (any OpenAI-compatible or Anthropic-compatible API) via the UI. These are stored in the model pool with `base_url`, `api_key`, `model_name`, and `api_format` (openai/anthropic).

### Configuration Files (auto-created in `~/.config/open-proxy-ai/`)

| File | Contents |
|---|---|
| `api-keys.json` | `{name: key}` map, auto-generated admin + user-default keys |
| `model_pool.json` | `{pool_mode, entries[]}` with priority, enabled state, provider config |
| `custom_models.json` | `[string]` simple list of custom model names (legacy, superseded by pool) |

### One-Click Export

Supports exporting config to Claude Code (`~/.claude/settings.json`), Codex (`~/.codex/config.toml`), and CCSwitch (deep link + `~/.cc-cast/config.json` fallback).

### MiMo Auto-Detection

On refresh, checks for MiMo CLI (`which mimo`, `node_modules/.bin/mimo`) and the free client ID file at `~/.local/share/mimocode/mimo-free-client`, auto-adds to pool if found.
