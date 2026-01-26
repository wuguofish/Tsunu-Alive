mod claude;
mod ide_server;
mod permission_server;

use std::sync::{Arc, OnceLock};
use std::path::PathBuf;
use std::fs;
use tauri::State;
use tokio::sync::Mutex;
use serde_json::{json, Value};

// 命令列指定的工作目錄（全域）
static CUSTOM_WORKING_DIR: OnceLock<String> = OnceLock::new();

// 全域狀態
struct AppState {
    claude_process: Arc<Mutex<claude::ClaudeProcess>>,
    permission_state: permission_server::SharedPermissionState,
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
    resume_session_id: Option<String>,
) -> Result<(), String> {
    // 取得 session_id：優先使用傳入的 resume_session_id，否則使用當前進程的 session_id
    let process = state.claude_process.clone();
    let session_id = if resume_session_id.is_some() {
        resume_session_id
    } else {
        claude::get_session_id(process.clone()).await
    };

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
/// 優先返回命令列參數指定的目錄，否則返回當前目錄
#[tauri::command]
fn get_working_directory() -> Result<String, String> {
    // 優先使用命令列參數指定的工作目錄
    if let Some(custom_dir) = CUSTOM_WORKING_DIR.get() {
        return Ok(custom_dir.clone());
    }

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

/// Skill 項目（用於斜線選單）
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillItem {
    pub name: String,           // Skill 名稱（例如 "gget-analyzer"）
    pub description: String,    // Skill 說明
    pub source: String,         // 來源："builtin", "user", "project"
}

/// Session 項目（用於歷史對話列表）
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionItem {
    pub session_id: String,
    pub created_at: String,      // ISO 8601 格式
    pub last_modified: String,   // ISO 8601 格式
    pub summary: Option<String>, // 對話摘要（第一條訊息）
}

/// 歷史訊息項目（對應前端的 ChatItem）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum HistoryChatItem {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "tool")]
    Tool { tool: HistoryToolUse },
}

/// 工具使用記錄
#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryToolUse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub input: Value,
    pub result: Option<String>,
    /// Edit 工具的結構化差異（VS Code 風格 Diff View）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_patch: Option<Value>,
    /// 工具執行是否失敗（對應 Claude CLI 的 is_error 欄位）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// 歷史訊息（對應前端的 Message）
#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryMessage {
    pub role: String,
    pub items: Vec<HistoryChatItem>,
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

/// 從 SKILL.md 檔案中解析 name 和 description
/// 格式：YAML frontmatter 在檔案開頭
fn parse_skill_md(content: &str, fallback_name: &str) -> (String, String) {
    let mut name = fallback_name.to_string();
    let mut description = String::new();

    // 檢查是否有 YAML frontmatter（以 --- 開頭）
    if content.starts_with("---") {
        // 找到結束的 ---
        if let Some(end_idx) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end_idx];

            // 簡單解析 YAML（不用外部庫）
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("name:") {
                    name = value.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(value) = line.strip_prefix("description:") {
                    description = value.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }

    // 如果沒有從 frontmatter 取得 description，嘗試從內容第一行取得
    if description.is_empty() {
        // 跳過 frontmatter，取得第一個非空行
        let content_after_frontmatter = if content.starts_with("---") {
            if let Some(end_idx) = content[3..].find("---") {
                &content[3 + end_idx + 3..]
            } else {
                content
            }
        } else {
            content
        };

        for line in content_after_frontmatter.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                description = line.chars().take(100).collect();
                break;
            }
        }
    }

    (name, description)
}

/// 掃描指定目錄下的 skills
fn scan_skills_in_dir(dir: &PathBuf, source: &str) -> Vec<SkillItem> {
    let mut skills = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return skills;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    let folder_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if let Ok(content) = fs::read_to_string(&skill_md) {
                        let (name, description) = parse_skill_md(&content, &folder_name);
                        skills.push(SkillItem {
                            name,
                            description,
                            source: source.to_string(),
                        });
                    }
                }
            }
        }
    }

    skills
}

/// 掃描所有可用的 Slash Commands（Skills）
#[tauri::command]
fn scan_skills(working_dir: Option<String>) -> Result<Vec<SkillItem>, String> {
    let mut all_skills = Vec::new();

    // 注意：CLI 內建命令（clear, compact, export, init, memory, plan, rename, rewind, todos 等）
    // 在 SDK 模式下都不支援，只會返回 "Unknown skill: xxx"
    // 只有透過 Skill 工具呼叫的 Skills 才能在 SDK 模式下使用
    // 這些 Skills 會在 init 事件的 slash_commands 中列出，由前端從事件中取得
    // 這裡只掃描使用者自定義的 Skills

    // 1. 使用者級 Skills（~/.claude/skills/）
    if let Some(home) = dirs::home_dir() {
        let user_skills_dir = home.join(".claude").join("skills");
        let user_skills = scan_skills_in_dir(&user_skills_dir, "user");
        all_skills.extend(user_skills);
    }

    // 3. 專案級 Skills（.claude/skills/）
    if let Some(ref wd) = working_dir {
        let project_skills_dir = PathBuf::from(wd).join(".claude").join("skills");
        let project_skills = scan_skills_in_dir(&project_skills_dir, "project");
        all_skills.extend(project_skills);
    }

    // 按名稱排序
    all_skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(all_skills)
}

/// 將專案路徑轉換為 Claude CLI 的目錄名稱格式
/// 例如：d:\game\tsunu_alive → d--game-tsunu-alive
fn get_project_dir_name(working_dir: &str) -> String {
    // 標準化路徑分隔符
    let path = working_dir
        .replace('\\', "/")
        .replace(':', "")
        .replace('_', "-");

    // 分割路徑並重新組合
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return "unknown".to_string();
    }

    // 第一個部分是磁碟機代號（如果有的話），用 -- 連接
    // 其餘部分用 - 連接
    let mut result = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            result.push_str(&part.to_lowercase());
            result.push_str("--");
        } else {
            if i > 1 {
                result.push('-');
            }
            result.push_str(&part.to_lowercase());
        }
    }

    result
}

/// 取得 Claude CLI 的 sessions 目錄
fn get_claude_sessions_dir(working_dir: &str) -> Option<PathBuf> {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = std::env::var_os(home_var)?;
    let home_path = PathBuf::from(home);

    // Claude CLI 的專案資料存放在 ~/.claude/projects/[project-dir-name]/
    let project_dir_name = get_project_dir_name(working_dir);
    let sessions_dir = home_path
        .join(".claude")
        .join("projects")
        .join(&project_dir_name);

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
                                // 截斷長文字（使用字元數而非 byte 數，避免切到 UTF-8 字元中間）
                                let chars: Vec<char> = s.chars().collect();
                                if chars.len() > 50 {
                                    format!("{}...", chars[..50].iter().collect::<String>())
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

/// 檢查文字是否為系統訊息（不應該顯示在對話中）
fn is_system_message(text: &str) -> bool {
    let trimmed = text.trim();

    // 過濾 Claude CLI 內部訊息
    trimmed.starts_with("Caveat:") ||
    trimmed.starts_with("Unknown skill:") ||
    trimmed.starts_with("[Request interrupted") ||
    trimmed.starts_with("No response requested") ||
    // 過濾 system-reminder 標籤
    trimmed.starts_with("<system-reminder>") ||
    trimmed.contains("</system-reminder>") ||
    // 過濾其他系統標籤
    trimmed.starts_with("<ide_") ||
    trimmed.starts_with("<command-name>")
}

/// 載入指定 session 的歷史訊息
#[tauri::command]
fn load_session_history(
    working_dir: String,
    session_id: String,
) -> Result<Vec<HistoryMessage>, String> {
    let sessions_dir = match get_claude_sessions_dir(&working_dir) {
        Some(dir) => dir,
        None => return Err("Session directory not found".to_string()),
    };

    let session_file = sessions_dir.join(format!("{}.jsonl", session_id));

    if !session_file.exists() {
        return Err(format!("Session file not found: {}", session_id));
    }

    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;

    let mut messages: Vec<HistoryMessage> = Vec::new();
    let mut current_assistant_msg: Option<HistoryMessage> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let json: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // 跳過無效的 JSON 行
        };

        let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "user" => {
                // 先儲存當前的 assistant 訊息（如果有的話）
                if let Some(msg) = current_assistant_msg.take() {
                    if !msg.items.is_empty() {
                        messages.push(msg);
                    }
                }

                // 解析 user 訊息內容
                if let Some(content_array) = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                {
                    let mut user_text_content: Vec<String> = Vec::new();

                    for content_item in content_array {
                        let item_type = content_item.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        match item_type {
                            "text" => {
                                // 一般文字（user 訊息可能有多段 text）
                                if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                                    // 過濾掉 IDE 相關的系統訊息
                                    if !text.starts_with("<ide_") {
                                        user_text_content.push(text.to_string());
                                    }
                                }
                            }
                            "tool_result" => {
                                // 工具結果 - 需要更新之前的 assistant 訊息中的工具
                                let tool_use_id = content_item.get("tool_use_id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("");

                                // 取得工具結果（需要處理圖片等特殊格式）
                                let tool_use_result = json.get("toolUseResult");
                                let result_content: Option<String> = {
                                    // 優先檢查 toolUseResult
                                    if let Some(tur) = tool_use_result {
                                        if let Some(file_path) = tur.get("filePath").and_then(|p| p.as_str()) {
                                            // ExitPlanMode 的計畫檔案路徑
                                            Some(format!("Your plan has been saved to: {}", file_path))
                                        } else if let Some(file) = tur.get("file") {
                                            // 檢查是否有 filePath 或 base64
                                            if let Some(fp) = file.get("filePath").and_then(|p| p.as_str()) {
                                                Some(fp.to_string())
                                            } else if file.get("base64").is_some() {
                                                // 圖片結果
                                                Some("image".to_string())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }.or_else(|| {
                                    // 從 content_item.content 取得
                                    let content = content_item.get("content")?;
                                    if let Some(s) = content.as_str() {
                                        Some(s.to_string())
                                    } else if content.is_array() {
                                        // 陣列格式（如圖片）
                                        let first = content.as_array()?.first()?;
                                        if first.get("type").and_then(|t| t.as_str()) == Some("image") {
                                            Some("image".to_string())
                                        } else {
                                            Some("success".to_string())
                                        }
                                    } else {
                                        None
                                    }
                                });

                                // 提取 is_error 欄位（工具執行失敗時會有這個欄位）
                                let is_error = content_item.get("is_error")
                                    .and_then(|e| e.as_bool());

                                // 檢查 toolUseResult.filePath（ExitPlanMode 的計畫檔案路徑）
                                let plan_file_path = tool_use_result
                                    .and_then(|r| r.get("filePath"))
                                    .and_then(|p| p.as_str())
                                    .map(|s| s.to_string());

                                // 提取 Edit 工具的 structuredPatch（VS Code 風格 Diff View）
                                let structured_patch = tool_use_result
                                    .and_then(|r| r.get("structuredPatch"))
                                    .cloned();

                                // 在之前的 messages 中找到對應的工具並更新結果
                                for msg in messages.iter_mut().rev() {
                                    if msg.role == "assistant" {
                                        let mut found = false;
                                        for item in &mut msg.items {
                                            if let HistoryChatItem::Tool { tool } = item {
                                                if tool.id == tool_use_id {
                                                    tool.result = result_content.clone();
                                                    // 設定工具執行錯誤狀態
                                                    if is_error == Some(true) {
                                                        tool.is_error = Some(true);
                                                    }
                                                    // 如果有計畫檔案路徑，加入到 input 中
                                                    if let Some(ref path) = plan_file_path {
                                                        if let Some(input_obj) = tool.input.as_object_mut() {
                                                            input_obj.insert("_planFilePath".to_string(), json!(path));
                                                        }
                                                    }
                                                    // 設定 Edit 工具的結構化差異
                                                    if structured_patch.is_some() {
                                                        tool.structured_patch = structured_patch.clone();
                                                    }
                                                    found = true;
                                                    break;
                                                }
                                            }
                                        }
                                        if found {
                                            break;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    // 只有當有實際的文字內容時才新增 user 訊息（tool_result 不算）
                    if !user_text_content.is_empty() {
                        messages.push(HistoryMessage {
                            role: "user".to_string(),
                            items: vec![HistoryChatItem::Text {
                                content: user_text_content.join("\n"),
                            }],
                        });
                    }
                } else if let Some(msg_content) = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                {
                    // 舊格式：content 是字串
                    messages.push(HistoryMessage {
                        role: "user".to_string(),
                        items: vec![HistoryChatItem::Text {
                            content: msg_content.to_string(),
                        }],
                    });
                }
            }
            "assistant" => {
                // 先儲存當前的 assistant 訊息（如果有的話）
                if let Some(msg) = current_assistant_msg.take() {
                    if !msg.items.is_empty() {
                        messages.push(msg);
                    }
                }

                // 解析 assistant 訊息內容
                if let Some(message) = json.get("message") {
                    if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                        let mut assistant_items: Vec<HistoryChatItem> = Vec::new();

                        for content_item in content_array {
                            let item_type = content_item.get("type").and_then(|t| t.as_str()).unwrap_or("");

                            match item_type {
                                "text" => {
                                    if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                                        // 過濾系統訊息和空白文字
                                        if !text.trim().is_empty() && !is_system_message(text) {
                                            assistant_items.push(HistoryChatItem::Text {
                                                content: text.to_string(),
                                            });
                                        }
                                    }
                                }
                                "tool_use" => {
                                    let tool_id = content_item.get("id")
                                        .and_then(|i| i.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let tool_name = content_item.get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let input = content_item.get("input")
                                        .cloned()
                                        .unwrap_or(json!({}));

                                    let tool = HistoryToolUse {
                                        id: tool_id,
                                        name: tool_name.clone(),
                                        tool_type: tool_name,
                                        input,
                                        result: None,  // 結果會在後續的 user 訊息中填充
                                        structured_patch: None,  // 會在後續的 user 訊息中填充
                                        is_error: None,  // 錯誤狀態會在後續的 user 訊息中填充
                                    };

                                    assistant_items.push(HistoryChatItem::Tool { tool });
                                }
                                // 忽略 thinking 等其他類型
                                _ => {}
                            }
                        }

                        // 如果這個 assistant 訊息有內容，創建訊息
                        if !assistant_items.is_empty() {
                            current_assistant_msg = Some(HistoryMessage {
                                role: "assistant".to_string(),
                                items: assistant_items,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // 儲存最後一個 assistant 訊息
    if let Some(msg) = current_assistant_msg.take() {
        if !msg.items.is_empty() {
            messages.push(msg);
        }
    }

    // 只返回最後 200 筆訊息，避免歷史過長影響效能
    const MAX_HISTORY_MESSAGES: usize = 200;
    if messages.len() > MAX_HISTORY_MESSAGES {
        messages = messages.split_off(messages.len() - MAX_HISTORY_MESSAGES);
    }

    Ok(messages)
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
// Tab Management Commands
// ============================================================================

/// 儲存標籤頁資料到 .claude/tabs.json
#[tauri::command]
async fn save_tabs(working_dir: String, data: Value) -> Result<(), String> {
    let tabs_path = PathBuf::from(&working_dir)
        .join(".claude")
        .join("tabs.json");

    // 確保 .claude 目錄存在
    if let Some(parent) = tabs_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
    }

    // 寫入檔案
    let content = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize tabs data: {}", e))?;
    fs::write(&tabs_path, content)
        .map_err(|e| format!("Failed to write tabs file: {}", e))?;

    eprintln!("💾 Tabs saved to {:?}", tabs_path);
    Ok(())
}

/// 載入標籤頁資料從 .claude/tabs.json
#[tauri::command]
async fn load_tabs(working_dir: String) -> Result<Option<Value>, String> {
    let tabs_path = PathBuf::from(&working_dir)
        .join(".claude")
        .join("tabs.json");

    if !tabs_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&tabs_path)
        .map_err(|e| format!("Failed to read tabs file: {}", e))?;

    let data: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse tabs file: {}", e))?;

    eprintln!("📂 Tabs loaded from {:?}", tabs_path);
    Ok(Some(data))
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

// ============================================================================
// Image Commands
// ============================================================================

/// 儲存圖片資料到臨時檔案
/// 接收 RGBA 圖片資料和尺寸，儲存為 PNG 檔案
#[tauri::command]
fn save_temp_image(rgba_data: Vec<u8>, width: u32, height: u32) -> Result<String, String> {
    use std::io::Write;

    // 確保臨時目錄存在
    let temp_dir = std::env::temp_dir().join("tsunu_alive");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    }

    // 產生唯一檔名
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("clipboard_{}.png", timestamp);
    let file_path = temp_dir.join(&filename);

    // 將 RGBA 資料編碼為 PNG
    let file = fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()
        .map_err(|e| format!("Failed to write PNG header: {}", e))?;
    writer.write_image_data(&rgba_data)
        .map_err(|e| format!("Failed to write PNG data: {}", e))?;

    // 回傳完整路徑
    Ok(file_path.to_string_lossy().to_string())
}

/// 儲存剪貼簿圖片到臨時檔案（直接接收 PNG 資料，效能更好）
#[tauri::command]
fn save_temp_image_png(png_data: Vec<u8>) -> Result<String, String> {
    use std::io::Write;

    // 確保臨時目錄存在
    let temp_dir = std::env::temp_dir().join("tsunu_alive");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    }

    // 產生唯一檔名
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("clipboard_{}.png", timestamp);
    let file_path = temp_dir.join(&filename);

    // 直接寫入 PNG 資料（不需要重新編碼）
    let mut file = fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(&png_data)
        .map_err(|e| format!("Failed to write PNG data: {}", e))?;

    // 回傳完整路徑
    Ok(file_path.to_string_lossy().to_string())
}

/// 清理臨時圖片檔案
#[tauri::command]
fn cleanup_temp_image(file_path: String) -> Result<(), String> {
    let path = PathBuf::from(&file_path);

    // 只允許刪除臨時目錄下的檔案
    let temp_dir = std::env::temp_dir().join("tsunu_alive");
    if !path.starts_with(&temp_dir) {
        return Err("Cannot delete file outside temp directory".to_string());
    }

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete temp file: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Plan File Commands
// ============================================================================

/// 計畫檔案資訊
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanFileInfo {
    pub path: String,          // 完整路徑
    pub name: String,          // 檔案名稱（不含副檔名）
    pub modified_at: String,   // ISO 8601 格式
}

// ============================================================================
// Permission Server Commands
// ============================================================================

/// 回應權限請求（前端呼叫）
#[tauri::command]
async fn respond_to_permission(
    state: State<'_, AppState>,
    tool_use_id: String,
    behavior: String,
    message: Option<String>,
) -> Result<(), String> {
    let mut perm_state = state.permission_state.lock().await;

    if let Some(tx) = perm_state.pending.remove(&tool_use_id) {
        let response = permission_server::PermissionResponse {
            behavior,
            message,
            updated_input: None,
        };

        tx.send(response).map_err(|_| "Failed to send permission response")?;
        Ok(())
    } else {
        Err(format!("No pending permission request found for {}", tool_use_id))
    }
}

/// 將工具加入 session 白名單（前端呼叫）
#[tauri::command]
async fn add_to_session_whitelist(
    state: State<'_, AppState>,
    session_id: String,
    tool_name: String,
) -> Result<(), String> {
    let mut perm_state = state.permission_state.lock().await;
    perm_state.add_to_session_whitelist(&session_id, &tool_name);
    eprintln!("📝 Added {} to session {} whitelist", tool_name, session_id);
    Ok(())
}

/// 清除 session 白名單（前端呼叫）
#[tauri::command]
async fn clear_session_whitelist(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let mut perm_state = state.permission_state.lock().await;
    perm_state.clear_session_whitelist(&session_id);
    eprintln!("🗑️ Cleared session {} whitelist", session_id);
    Ok(())
}

// ============================================================================
// Hook Installation
// ============================================================================

/// 安裝 PermissionRequest Hook 到 ~/.claude/settings.json
/// 這讓 Claude CLI 在需要權限確認時呼叫我們的 Hook 腳本
fn install_permission_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let settings_path = home.join(".claude").join("settings.json");

    // 確保 .claude 目錄存在
    let claude_dir = home.join(".claude");
    if !claude_dir.exists() {
        fs::create_dir_all(&claude_dir)
            .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
    }

    // 讀取現有設定或建立新的
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // 確定 Hook 腳本路徑
    let hook_script = if cfg!(windows) {
        "powershell.exe -ExecutionPolicy Bypass -File \"$HOME/.claude/hooks/tsunu-permission.ps1\""
    } else {
        "$HOME/.claude/hooks/tsunu-permission.sh"
    };

    // 檢查是否已經有 PermissionRequest hook
    let hooks = settings.get_mut("hooks")
        .and_then(|h| h.as_object_mut());

    let needs_install = if let Some(hooks_obj) = hooks {
        // 檢查 PermissionRequest 是否已存在我們的 hook
        if let Some(perm_hooks) = hooks_obj.get("PermissionRequest").and_then(|p| p.as_array()) {
            !perm_hooks.iter().any(|h| {
                h.get("hooks")
                    .and_then(|hs| hs.as_array())
                    .map(|arr| arr.iter().any(|item| {
                        item.get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s.contains("tsunu-permission"))
                            .unwrap_or(false)
                    }))
                    .unwrap_or(false)
            })
        } else {
            true
        }
    } else {
        true
    };

    if needs_install {
        // 建立 Hook 設定
        let hook_config = json!({
            "matcher": "*",  // 匹配所有工具
            "hooks": [{
                "type": "command",
                "command": hook_script,
                "timeout": 60000  // 60 秒 timeout
            }]
        });

        // 確保 hooks 物件存在
        if settings.get("hooks").is_none() {
            settings["hooks"] = json!({});
        }

        // 確保 PermissionRequest 陣列存在
        if settings["hooks"].get("PermissionRequest").is_none() {
            settings["hooks"]["PermissionRequest"] = json!([]);
        }

        // 新增我們的 hook
        if let Some(arr) = settings["hooks"]["PermissionRequest"].as_array_mut() {
            arr.push(hook_config);
        }

        // 寫回設定檔
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&settings_path, content)
            .map_err(|e| format!("Failed to write settings: {}", e))?;

        eprintln!("📝 Installed PermissionRequest hook to {:?}", settings_path);
    } else {
        eprintln!("✅ PermissionRequest hook already installed");
    }

    // 複製 Hook 腳本到 ~/.claude/hooks/
    let hooks_dir = home.join(".claude").join("hooks");
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)
            .map_err(|e| format!("Failed to create hooks directory: {}", e))?;
    }

    // 取得執行檔所在目錄（用於找到 resources）
    // 在開發模式下，資源會在不同的位置
    let resource_hook = if cfg!(windows) {
        "tsunu-permission.ps1"
    } else {
        "tsunu-permission.sh"
    };

    // 嘗試從多個可能的位置讀取 Hook 腳本
    let hook_content = get_hook_script_content(resource_hook);

    if let Some(content) = hook_content {
        let dest_path = hooks_dir.join(resource_hook);
        fs::write(&dest_path, &content)
            .map_err(|e| format!("Failed to write hook script: {}", e))?;

        // 在 Unix 系統上設定執行權限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest_path)
                .map_err(|e| format!("Failed to get permissions: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest_path, perms)
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        eprintln!("📝 Installed hook script to {:?}", dest_path);
    } else {
        eprintln!("⚠️ Could not find hook script resource, skipping installation");
    }

    Ok(())
}

/// 取得 Hook 腳本內容（內嵌在程式中作為 fallback）
fn get_hook_script_content(script_name: &str) -> Option<String> {
    // 內嵌的 Hook 腳本內容（作為 fallback）
    if script_name == "tsunu-permission.ps1" {
        Some(include_str!("../../resources/hooks/tsunu-permission.ps1").to_string())
    } else if script_name == "tsunu-permission.sh" {
        Some(include_str!("../../resources/hooks/tsunu-permission.sh").to_string())
    } else {
        None
    }
}

// ============================================================================
// Plan File Commands
// ============================================================================

/// 取得最近的計畫檔案路徑
/// 掃描 ~/.claude/plans/ 目錄，返回最近修改的 .md 檔案
#[tauri::command]
fn get_recent_plan_path() -> Result<Option<PlanFileInfo>, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let plans_dir = home.join(".claude").join("plans");

    if !plans_dir.exists() {
        return Ok(None);
    }

    // 找到最近修改的 .md 檔案
    let mut recent_file: Option<(PathBuf, std::time::SystemTime)> = None;

    let entries = fs::read_dir(&plans_dir)
        .map_err(|e| format!("Failed to read plans directory: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "md") {
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if recent_file.as_ref().map_or(true, |(_, time)| modified > *time) {
                        recent_file = Some((path, modified));
                    }
                }
            }
        }
    }

    match recent_file {
        Some((path, modified_time)) => {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let modified_at = {
                let datetime: chrono::DateTime<chrono::Local> = modified_time.into();
                datetime.to_rfc3339()
            };

            Ok(Some(PlanFileInfo {
                path: path.to_string_lossy().to_string(),
                name,
                modified_at,
            }))
        }
        None => Ok(None),
    }
}

/// Permission HTTP Server 預設埠號
const PERMISSION_SERVER_PORT: u16 = 19751;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 解析命令列參數
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--working-dir" || args[i] == "-w" {
            if let Some(dir) = args.get(i + 1) {
                let path = PathBuf::from(dir);
                if path.exists() && path.is_dir() {
                    let _ = CUSTOM_WORKING_DIR.set(path.to_string_lossy().to_string());
                    println!("📂 Working directory set to: {}", dir);
                } else {
                    eprintln!("⚠️ Warning: specified working directory does not exist: {}", dir);
                }
            }
            break;
        }
    }

    // 建立共享的 permission state
    let permission_state = permission_server::create_shared_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            claude_process: Arc::new(Mutex::new(claude::ClaudeProcess::default())),
            permission_state: permission_state.clone(),
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
            scan_skills,
            list_sessions,
            load_session_history,
            save_tabs,
            load_tabs,
            start_ide_server,
            get_ide_status,
            get_ide_context,
            get_recent_plan_path,
            respond_to_permission,
            add_to_session_whitelist,
            clear_session_whitelist,
            save_temp_image,
            save_temp_image_png,
            cleanup_temp_image
        ])
        .setup(move |app| {
            // 安裝 PermissionRequest Hook
            if let Err(e) = install_permission_hooks() {
                eprintln!("⚠️ Hook 安裝失敗: {}", e);
            }

            // 設定 AppHandle 到 permission state
            {
                let state = permission_state.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut guard = state.lock().await;
                    guard.set_app_handle(app_handle);
                });
            }

            // 啟動 IDE WebSocket Server
            tauri::async_runtime::spawn(async {
                if let Err(e) = ide_server::start_ide_server().await {
                    eprintln!("⚠️ IDE Server 啟動失敗: {}", e);
                }
            });

            // 啟動 Permission HTTP Server
            let perm_state = permission_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = permission_server::start_server(perm_state, PERMISSION_SERVER_PORT).await {
                    eprintln!("⚠️ Permission Server 啟動失敗: {}", e);
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
