mod claude;

use std::sync::Arc;
use std::path::PathBuf;
use std::fs;
use tauri::State;
use tokio::sync::Mutex;
use serde_json::{json, Value};

// 全域狀態
struct AppState {
    claude_process: Arc<Mutex<claude::ClaudeProcess>>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// 啟動 Claude CLI（用於互動模式）
#[tauri::command]
async fn start_claude(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    working_dir: Option<String>,
    session_id: Option<String>,
) -> Result<(), String> {
    let process = state.claude_process.clone();
    claude::start_claude(app, process, working_dir, session_id).await
}

/// 發送訊息給 Claude（互動模式）
#[tauri::command]
async fn send_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let process = state.claude_process.clone();
    claude::send_message(process, message).await
}

/// 發送訊息給 Claude（單次模式）
#[tauri::command]
async fn send_to_claude(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    prompt: String,
    working_dir: Option<String>,
    allowed_tools: Option<Vec<String>>,
    permission_mode: Option<String>,
) -> Result<(), String> {
    // 取得現有的 session_id
    let process = state.claude_process.clone();
    let session_id = claude::get_session_id(process.clone()).await;

    // 執行 Claude CLI
    claude::run_claude(app, process, prompt, working_dir, session_id, allowed_tools, permission_mode).await
}

/// 中斷 Claude 程序
#[tauri::command]
async fn interrupt_claude(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let process = state.claude_process.clone();
    claude::interrupt_claude(process).await
}

/// 取得目前的 session ID
#[tauri::command]
async fn get_session_id(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let process = state.claude_process.clone();
    Ok(claude::get_session_id(process).await)
}

/// 清除目前的 session
#[tauri::command]
async fn clear_session(state: State<'_, AppState>) -> Result<(), String> {
    // 先中斷程序
    let process = state.claude_process.clone();
    claude::interrupt_claude(process).await?;
    Ok(())
}

/// 將工具加入專案級白名單（寫入 .claude/settings.local.json）
#[tauri::command]
async fn add_project_permission(
    working_dir: String,
    tool_name: String,
) -> Result<(), String> {
    let settings_path = PathBuf::from(&working_dir)
        .join(".claude")
        .join("settings.local.json");

    // 讀取現有設定或建立新的
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?
    } else {
        // 確保 .claude 目錄存在
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
        }
        json!({
            "permissions": {
                "allow": []
            }
        })
    };

    // 確保 permissions.allow 陣列存在
    if settings.get("permissions").is_none() {
        settings["permissions"] = json!({ "allow": [] });
    }
    if settings["permissions"].get("allow").is_none() {
        settings["permissions"]["allow"] = json!([]);
    }

    // 工具權限格式：ToolName:* 表示允許該工具的所有操作
    let permission_entry = format!("{}:*", tool_name);

    // 檢查是否已存在
    let allow_array = settings["permissions"]["allow"]
        .as_array_mut()
        .ok_or("permissions.allow is not an array")?;

    let already_exists = allow_array
        .iter()
        .any(|v| v.as_str() == Some(&permission_entry));

    if !already_exists {
        allow_array.push(json!(permission_entry));
        eprintln!("📝 Added project permission: {}", permission_entry);

        // 寫回檔案
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&settings_path, content)
            .map_err(|e| format!("Failed to write settings: {}", e))?;
    } else {
        eprintln!("📝 Permission already exists: {}", permission_entry);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            claude_process: Arc::new(Mutex::new(claude::ClaudeProcess::default())),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_claude,
            send_message,
            send_to_claude,
            interrupt_claude,
            get_session_id,
            clear_session,
            add_project_permission
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
