use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tracing::info;

mod proxy;

use proxy::auth::AuthManager;
use proxy::model_pool::{ModelPool, ModelPoolEntry};
use proxy::server::{run_speed_test, ProxyState, SpeedTestResult};
use proxy::zen::{SessionManager, ZenClient};

#[derive(Serialize)]
pub struct AppStatus {
    pub running: bool,
    pub port: u16,
    pub model_count: usize,
    pub keys: Vec<proxy::auth::ApiKeyEntry>,
    pub custom_models: Vec<String>,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub builtin: bool,
}

#[derive(Deserialize)]
pub struct AddModelRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SpeedTestRequest {
    pub model: String,
}

#[derive(Deserialize)]
pub struct ImportRequest {
    pub model: String,
    #[serde(default)]
    pub model_name: String,
    pub api_key: String,
    pub tool: String, // "claude" | "codex" | "ccswitch"
}

// ── Model Pool ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PoolEntryRequest {
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub priority: u32,
    pub enabled: bool,
    pub builtin: bool,
    pub provider_type: String,
}

#[derive(Deserialize)]
pub struct TogglePoolRequest {
    pub pool_mode: bool,
}

#[derive(Serialize)]
pub struct PoolStatus {
    pub pool_mode: bool,
    pub entries: Vec<ModelPoolEntry>,
}

#[tauri::command]
async fn get_model_pool(
    state: tauri::State<'_, AppState>,
) -> Result<PoolStatus, String> {
    let pool = state.proxy.model_pool.read().await;
    Ok(PoolStatus {
        pool_mode: pool.pool_mode,
        entries: pool.entries.clone(),
    })
}

#[tauri::command]
async fn set_pool_mode(
    state: tauri::State<'_, AppState>,
    req: TogglePoolRequest,
) -> Result<bool, String> {
    let mut pool = state.proxy.model_pool.write().await;
    pool.pool_mode = req.pool_mode;
    if let Some(ref config_dir) = state.config_dir {
        pool.save(&config_dir.join("model_pool.json"));
    }
    Ok(pool.pool_mode)
}

#[tauri::command]
async fn upsert_pool_entry(
    state: tauri::State<'_, AppState>,
    req: PoolEntryRequest,
) -> Result<PoolStatus, String> {
    let mut pool = state.proxy.model_pool.write().await;
    let id = req.id.unwrap_or_else(|| format!("provider-{}", uuid::Uuid::new_v4()));
    let entry = ModelPoolEntry {
        id,
        name: req.name,
        base_url: req.base_url,
        api_key: req.api_key,
        model_name: req.model_name,
        priority: req.priority,
        enabled: req.enabled,
        builtin: req.builtin,
        provider_type: req.provider_type,
    };
    pool.upsert(entry);
    if let Some(ref config_dir) = state.config_dir {
        pool.save(&config_dir.join("model_pool.json"));
    }
    Ok(PoolStatus {
        pool_mode: pool.pool_mode,
        entries: pool.entries.clone(),
    })
}

#[tauri::command]
async fn remove_pool_entry(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<PoolStatus, String> {
    let mut pool = state.proxy.model_pool.write().await;
    pool.remove(&id);
    if let Some(ref config_dir) = state.config_dir {
        pool.save(&config_dir.join("model_pool.json"));
    }
    Ok(PoolStatus {
        pool_mode: pool.pool_mode,
        entries: pool.entries.clone(),
    })
}

#[tauri::command]
async fn toggle_pool_entry(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<PoolStatus, String> {
    let mut pool = state.proxy.model_pool.write().await;
    pool.toggle_enabled(&id);
    if let Some(ref config_dir) = state.config_dir {
        pool.save(&config_dir.join("model_pool.json"));
    }
    Ok(PoolStatus {
        pool_mode: pool.pool_mode,
        entries: pool.entries.clone(),
    })
}

#[tauri::command]
async fn init_pool_builtins(
    state: tauri::State<'_, AppState>,
) -> Result<PoolStatus, String> {
    let mut pool = state.proxy.model_pool.write().await;
    let models: Vec<&str> = proxy::server::MODELS.to_vec();
    pool.init_builtins(&models);
    if let Some(ref config_dir) = state.config_dir {
        pool.save(&config_dir.join("model_pool.json"));
    }
    Ok(PoolStatus {
        pool_mode: pool.pool_mode,
        entries: pool.entries.clone(),
    })
}

#[tauri::command]
async fn import_to_tool(
    _app_handle: tauri::AppHandle,
    req: ImportRequest,
) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let base_url = "http://localhost:6446";
    let base_url_v1 = "http://localhost:6446/v1";
    let display_name = format!("OpenCode Free ({})", req.model);

    match req.tool.as_str() {
        "claude" => {
            // ~/.claude/settings.json
            let path = home.join(".claude/settings.json");
            let _ = std::fs::create_dir_all(path.parent().unwrap());
            let config = serde_json::json!({
                "env": {
                    "ANTHROPIC_BASE_URL": base_url,
                    "ANTHROPIC_API_KEY": req.api_key,
                    "ANTHROPIC_MODEL": api_model(&req),
                    "ANTHROPIC_DEFAULT_SONNET_MODEL": api_model(&req),
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL": api_model(&req),
                    "API_TIMEOUT_MS": "3000000"
                }
            });
            let content = serde_json::to_string_pretty(&config)
                .map_err(|e| format!("Serialize error: {}", e))?;
            std::fs::write(&path, content)
                .map_err(|e| format!("Write error: {}", e))?;
            Ok(format!("✅ 已导入到 Claude Code ({})", path.display()))
        }
        "codex" => {
            // ~/.codex/config.toml
            let path = home.join(".codex/config.toml");
            let _ = std::fs::create_dir_all(path.parent().unwrap());
            let content = format!(
                "base_url = \"{}\"\nmodel = \"{}\"\napi_key = \"{}\"\n",
                base_url_v1, api_model(&req), req.api_key
            );
            std::fs::write(&path, content)
                .map_err(|e| format!("Write error: {}", e))?;
            Ok(format!("✅ 已导入到 Codex ({})", path.display()))
        }
        "ccswitch" => {
            // Use ccswitch:// deep link protocol (official way)
            let encoded_name = urlencoding(&display_name);
            let encoded_endpoint = urlencoding(base_url);
            let encoded_key = urlencoding(&req.api_key);
            let encoded_model = urlencoding(&api_model(&req));
            let encoded_homepage = urlencoding("https://github.com/jxhhdx/opencode-free-proxy");

            let deep_link = format!(
                "ccswitch://v1/import?resource=provider&app=claude&name={}&endpoint={}&apiKey={}&model={}&homepage={}",
                encoded_name, encoded_endpoint, encoded_key, encoded_model, encoded_homepage
            );

            // Open deep link (macOS)
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open")
                    .arg(&deep_link)
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&deep_link)
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(&["/c", "start", "", &deep_link])
                    .spawn();
            }

            // Also write cc-cast config as fallback
            let cast_path = home.join(".cc-cast/config.json");
            let _ = std::fs::create_dir_all(cast_path.parent().unwrap());
            let cast_config = serde_json::json!({
                "profiles": {
                    &display_name: {
                        "env": {
                            "ANTHROPIC_BASE_URL": base_url,
                            "ANTHROPIC_AUTH_TOKEN": req.api_key,
                            "ANTHROPIC_MODEL": api_model(&req)
                        }
                    }
                }
            });
            if let Ok(content) = serde_json::to_string_pretty(&cast_config) {
                let _ = std::fs::write(&cast_path, content);
            }
            Ok("✅ 已唤起 CCSwitch 导入窗口，请在弹窗中确认".into())
        }
        other => Err(format!("Unknown tool: {}. Supported: claude, codex, ccswitch", other)),
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Use model_name if provided, fall back to display name.
fn api_model(req: &ImportRequest) -> String {
    if req.model_name.is_empty() { req.model.clone() } else { req.model_name.clone() }
}


// ── Tauri Commands ────────────────────────────────────────────────────

#[tauri::command]
async fn get_status(
    state: tauri::State<'_, AppState>,
) -> Result<AppStatus, String> {
    let auth = state.proxy.auth.read().await;
    let custom = state.proxy.custom_models.read().await;
    Ok(AppStatus {
        running: state.server_running.load(std::sync::atomic::Ordering::Relaxed),
        port: 6446,
        model_count: proxy::server::MODELS.len() + custom.len(),
        keys: auth.get_keys(),
        custom_models: custom.clone(),
    })
}

#[tauri::command]
async fn get_models(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModelInfo>, String> {
    let custom = state.proxy.custom_models.read().await;
    let mut models: Vec<ModelInfo> = proxy::server::MODELS
        .iter()
        .map(|m| ModelInfo {
            id: m.to_string(),
            builtin: true,
        })
        .collect();
    for cm in custom.iter() {
        if !proxy::server::MODELS.contains(&cm.as_str()) {
            models.push(ModelInfo {
                id: cm.clone(),
                builtin: false,
            });
        }
    }
    Ok(models)
}

#[tauri::command]
async fn add_custom_model(
    state: tauri::State<'_, AppState>,
    req: AddModelRequest,
) -> Result<Vec<String>, String> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err("Model name cannot be empty".into());
    }
    let mut custom = state.proxy.custom_models.write().await;
    if !custom.contains(&name) {
        custom.push(name.clone());
        custom.sort();
        // Persist
        if let Some(config_dir) = state.config_dir.clone() {
            let path = config_dir.join("custom_models.json");
            let _ = std::fs::create_dir_all(&config_dir);
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&*custom).unwrap_or_default());
        }
    }
    Ok(custom.clone())
}

#[tauri::command]
async fn remove_custom_model(
    state: tauri::State<'_, AppState>,
    req: AddModelRequest,
) -> Result<Vec<String>, String> {
    let mut custom = state.proxy.custom_models.write().await;
    custom.retain(|m| m != &req.name);
    if let Some(config_dir) = state.config_dir.clone() {
        let path = config_dir.join("custom_models.json");
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&*custom).unwrap_or_default());
    }
    Ok(custom.clone())
}

#[tauri::command]
async fn run_speed_test_cmd(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    req: SpeedTestRequest,
) -> Result<SpeedTestResult, String> {
    info!("Speed testing model: {}", req.model);

    let result = run_speed_test(&state.proxy, &req.model).await;

    // Emit event to frontend
    let _ = app_handle.emit("speed-test-complete", &result);

    Ok(result)
}

// ── App State ─────────────────────────────────────────────────────────

pub struct AppState {
    pub proxy: ProxyState,
    pub server_running: std::sync::atomic::AtomicBool,
    pub config_dir: Option<PathBuf>,
}

// ── Server Start ──────────────────────────────────────────────────────

async fn start_proxy_server(state: ProxyState, port: u16) {
    let app = proxy::server::create_router(Arc::new(state.clone()));
    let addr = format!("0.0.0.0:{}", port);

    info!("Starting proxy server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to {}: {}", addr, e);
            return;
        }
    };

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        tracing::error!("Server error: {}", e);
    });
}

// ── Tauri App ─────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Determine config directory
            let home = dirs_or_fallback();
            let config_dir = home.join(".config/opencode-free-proxy");
            let _ = std::fs::create_dir_all(&config_dir);

            let keys_path = config_dir.join("api-keys.json");
            let auth = AuthManager::new(keys_path);

            // Load custom models
            let custom_models_path = config_dir.join("custom_models.json");
            let custom_models: Vec<String> =
                std::fs::read_to_string(&custom_models_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

            let zen = ZenClient::new().expect("Failed to create HTTP client");
            let sessions = SessionManager::new();

            // Load model pool
            let pool_path = config_dir.join("model_pool.json");
            let mut model_pool = ModelPool::load(&pool_path);
            if model_pool.entries.is_empty() {
                model_pool.init_builtins(&proxy::server::MODELS);
                model_pool.save(&pool_path);
            }

            let proxy_state = ProxyState {
                auth: Arc::new(RwLock::new(auth)),
                zen: Arc::new(zen),
                sessions: Arc::new(sessions),
                custom_models: Arc::new(RwLock::new(custom_models)),
                model_pool: Arc::new(RwLock::new(model_pool)),
            };

            let server_running =
                std::sync::atomic::AtomicBool::new(false);

            let app_state = AppState {
                proxy: proxy_state.clone(),
                server_running,
                config_dir: Some(config_dir),
            };

            app.manage(app_state);

            // Start the proxy server
            let state_for_server = proxy_state.clone();
            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                start_proxy_server(state_for_server, 6446).await;
            });

            // Mark as running (small delay to let server bind)
            let state = handle.state::<AppState>();
            state
                .server_running
                .store(true, std::sync::atomic::Ordering::Relaxed);

            // Setup system tray
            setup_tray(&handle)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of closing
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_models,
            add_custom_model,
            remove_custom_model,
            run_speed_test_cmd,
            import_to_tool,
            get_model_pool,
            set_pool_mode,
            upsert_pool_entry,
            remove_pool_entry,
            toggle_pool_entry,
            init_pool_builtins,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn dirs_or_fallback() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn setup_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
