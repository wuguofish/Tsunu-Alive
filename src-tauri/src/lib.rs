mod claude;

use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

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
) -> Result<(), String> {
    // 取得現有的 session_id
    let process = state.claude_process.clone();
    let session_id = claude::get_session_id(process.clone()).await;

    // 執行 Claude CLI
    claude::run_claude(app, process, prompt, working_dir, session_id).await
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
            clear_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
