mod claude;
mod ide_server;

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
    extended_thinking: Option<bool>,
) -> Result<(), String> {
    // 取得現有的 session_id
    let process = state.claude_process.clone();
    let session_id = claude::get_session_id(process.clone()).await;

    // 執行 Claude CLI
    claude::run_claude(app, process, prompt, working_dir, session_id, allowed_tools, permission_mode, extended_thinking).await
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

/// 取得當前工作目錄
#[tauri::command]
fn get_working_directory() -> Result<String, String> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to get working directory: {}", e))
}

/// 檔案項目（用於 @-mention 選單）
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileItem {
    pub name: String,
    pub path: String,       // 相對於工作目錄的路徑
    pub is_dir: bool,
}

/// Session 項目（用於歷史對話列表）
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionItem {
    pub session_id: String,
    pub created_at: String,      // ISO 8601 格式
    pub last_modified: String,   // ISO 8601 格式
    pub summary: Option<String>, // 對話摘要（第一條訊息）
}

/// 列出指定目錄下的檔案和資料夾（用於 @-mention 自動完成）
#[tauri::command]
fn list_files(
    working_dir: String,
    sub_path: Option<String>,
    filter: Option<String>,
) -> Result<Vec<FileItem>, String> {
    let base_path = PathBuf::from(&working_dir);
    let target_path = if let Some(ref sub) = sub_path {
        base_path.join(sub)
    } else {
        base_path.clone()
    };

    if !target_path.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&target_path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    let filter_lower = filter.map(|f| f.to_lowercase());

    let mut items: Vec<FileItem> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();

            // 過濾隱藏檔案和常見的忽略目錄
            if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" {
                return None;
            }

            // 如果有過濾條件，檢查是否匹配
            if let Some(ref filter) = filter_lower {
                if !name.to_lowercase().contains(filter) {
                    return None;
                }
            }

            let metadata = entry.metadata().ok()?;
            let is_dir = metadata.is_dir();

            // 計算相對路徑
            let relative_path = if let Some(ref sub) = sub_path {
                format!("{}/{}", sub, name)
            } else {
                name.clone()
            };

            Some(FileItem {
                name,
                path: relative_path,
                is_dir,
            })
        })
        .collect();

    // 排序：資料夾優先，然後按名稱排序
    items.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    // 限制回傳數量
    items.truncate(50);

    Ok(items)
}

/// 計算專案路徑的 hash（用於定位 Claude CLI session 目錄）
fn get_project_hash(working_dir: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    working_dir.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// 取得 Claude CLI 的 sessions 目錄
fn get_claude_sessions_dir(working_dir: &str) -> Option<PathBuf> {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = std::env::var_os(home_var)?;
    let home_path = PathBuf::from(home);

    // Claude CLI 的專案資料存放在 ~/.claude/projects/[project-hash]/
    let project_hash = get_project_hash(working_dir);
    let sessions_dir = home_path
        .join(".claude")
        .join("projects")
        .join(&project_hash);

    if sessions_dir.exists() {
        Some(sessions_dir)
    } else {
        None
    }
}

/// 列出專案的歷史 sessions
#[tauri::command]
fn list_sessions(working_dir: String) -> Result<Vec<SessionItem>, String> {
    let sessions_dir = match get_claude_sessions_dir(&working_dir) {
        Some(dir) => dir,
        None => return Ok(Vec::new()), // 沒有 session 目錄，回傳空列表
    };

    let mut sessions: Vec<SessionItem> = Vec::new();

    // 讀取目錄中的 .jsonl 檔案
    let entries = fs::read_dir(&sessions_dir)
        .map_err(|e| format!("Failed to read sessions directory: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        // 只處理 .jsonl 檔案
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }

        // 從檔名取得 session_id
        let session_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // 取得檔案 metadata
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // 取得修改時間
        let modified = metadata.modified().ok();
        let created = metadata.created().ok();

        let last_modified = modified
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Local> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_default();

        let created_at = created
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Local> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|| last_modified.clone());

        // 嘗試讀取第一行來取得摘要
        let summary = fs::read_to_string(&path)
            .ok()
            .and_then(|content| {
                content.lines().find_map(|line| {
                    let json: serde_json::Value = serde_json::from_str(line).ok()?;
                    // 尋找第一個 user message
                    if json.get("type")?.as_str()? == "user" {
                        json.get("message")
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                            .map(|s| {
                                // 截斷長文字
                                if s.len() > 100 {
                                    format!("{}...", &s[..100])
                                } else {
                                    s.to_string()
                                }
                            })
                    } else {
                        None
                    }
                })
            });

        sessions.push(SessionItem {
            session_id,
            created_at,
            last_modified,
            summary,
        });
    }

    // 按修改時間排序（最新的在前）
    sessions.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    // 限制回傳數量
    sessions.truncate(50);

    Ok(sessions)
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

// ============================================================================
// IDE Server Commands
// ============================================================================

/// 啟動 IDE WebSocket Server
#[tauri::command]
async fn start_ide_server() -> Result<(), String> {
    ide_server::start_ide_server().await
}

/// 取得 IDE Server 狀態
#[tauri::command]
async fn get_ide_status() -> Result<ide_server::IdeServerStatus, String> {
    Ok(ide_server::get_ide_status().await)
}

/// 取得當前 IDE context（用於發送給 Claude 時附加）
#[tauri::command]
async fn get_ide_context() -> Result<Option<ide_server::IdeContext>, String> {
    Ok(ide_server::get_ide_context().await)
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
            add_project_permission,
            get_working_directory,
            list_files,
            list_sessions,
            start_ide_server,
            get_ide_status,
            get_ide_context
        ])
        .setup(|_app| {
            // 啟動 IDE WebSocket Server
            tauri::async_runtime::spawn(async {
                if let Err(e) = ide_server::start_ide_server().await {
                    eprintln!("⚠️ IDE Server 啟動失敗: {}", e);
                }
            });
            Ok(())
        })
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
