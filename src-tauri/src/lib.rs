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

/// 將工具加入專案級白名單的核心邏輯
/// 回傳 (是否新增, 設定檔路徑)
fn add_project_permission_core(
    working_dir: &str,
    tool_name: &str,
) -> Result<(bool, PathBuf), String> {
    let settings_path = PathBuf::from(working_dir)
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

        Ok((true, settings_path))
    } else {
        eprintln!("📝 Permission already exists: {}", permission_entry);
        Ok((false, settings_path))
    }
}

/// 將工具加入專案級白名單（寫入 .claude/settings.local.json）
#[tauri::command]
async fn add_project_permission(
    working_dir: String,
    tool_name: String,
) -> Result<(), String> {
    add_project_permission_core(&working_dir, &tool_name)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_add_permission_creates_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_str().unwrap();

        // 第一次新增權限
        let (added, path) = add_project_permission_core(working_dir, "Edit").unwrap();
        assert!(added);
        assert!(path.exists());

        // 讀取檔案確認內容
        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();

        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 1);
        assert_eq!(allow[0], "Edit:*");
    }

    #[test]
    fn test_add_permission_appends_to_existing() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_str().unwrap();

        // 先新增一個權限
        add_project_permission_core(working_dir, "Edit").unwrap();

        // 再新增另一個權限
        let (added, path) = add_project_permission_core(working_dir, "Bash").unwrap();
        assert!(added);

        // 讀取檔案確認內容
        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();

        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 2);
        assert!(allow.contains(&json!("Edit:*")));
        assert!(allow.contains(&json!("Bash:*")));
    }

    #[test]
    fn test_add_permission_no_duplicate() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_str().unwrap();

        // 新增權限
        let (added1, _) = add_project_permission_core(working_dir, "Read").unwrap();
        assert!(added1);

        // 嘗試新增相同權限
        let (added2, path) = add_project_permission_core(working_dir, "Read").unwrap();
        assert!(!added2); // 應該回傳 false（已存在）

        // 確認沒有重複
        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();

        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 1);
    }

    #[test]
    fn test_add_permission_preserves_existing_settings() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_str().unwrap();

        // 建立包含其他設定的檔案
        let claude_dir = temp_dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();

        let existing_settings = json!({
            "permissions": {
                "allow": ["Existing:*"]
            },
            "other_setting": "should_be_preserved"
        });
        fs::write(
            claude_dir.join("settings.local.json"),
            serde_json::to_string_pretty(&existing_settings).unwrap()
        ).unwrap();

        // 新增權限
        let (added, path) = add_project_permission_core(working_dir, "NewTool").unwrap();
        assert!(added);

        // 確認其他設定被保留
        let content = fs::read_to_string(&path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["other_setting"], "should_be_preserved");

        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 2);
        assert!(allow.contains(&json!("Existing:*")));
        assert!(allow.contains(&json!("NewTool:*")));
    }
}
