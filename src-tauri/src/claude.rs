use serde::Serialize;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

// 發送到前端的事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type")]
pub enum ClaudeEvent {
    // 初始化完成
    Init {
        session_id: String,
        model: String,
        // 可用的 Skills（從 init 事件的 slash_commands 取得）
        #[serde(skip_serializing_if = "Option::is_none")]
        slash_commands: Option<Vec<String>>,
    },
    // 文字串流
    Text {
        text: String,
        is_complete: bool,
    },
    // 工具使用（正在執行）
    ToolUse {
        tool_name: String,
        tool_id: String,
        input: serde_json::Value,
    },
    // 權限被拒絕（需要使用者確認）
    PermissionDenied {
        tool_name: String,
        tool_id: String,
        input: serde_json::Value,
    },
    // 工具結果
    ToolResult {
        tool_id: String,
        result: String,
        is_error: bool,
        // Edit 工具的結構化差異（VS Code 風格 Diff View 用）
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_patch: Option<serde_json::Value>,
        // 圖片結果的 base64 資料（Read 工具讀取圖片時）
        #[serde(skip_serializing_if = "Option::is_none")]
        image_base64: Option<String>,
    },
    // 完成
    Complete {
        result: String,
        cost_usd: f64,
        // Context 相關資訊
        #[serde(skip_serializing_if = "Option::is_none")]
        total_tokens_in_conversation: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_window_max: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_window_used_percent: Option<f64>,
    },
    // Compact 完成（對話摘要壓縮）
    Compacted {
        summary: String,
    },
    // 錯誤
    Error {
        message: String,
    },
    // 連線狀態
    Connected,
}

// 全域的 Claude 程序管理
pub struct ClaudeProcess {
    pub child: Option<Child>,
    pub stdin_writer: Option<tokio::process::ChildStdin>,
    pub session_id: Option<String>,
}

impl Default for ClaudeProcess {
    fn default() -> Self {
        Self {
            child: None,
            stdin_writer: None,
            session_id: None,
        }
    }
}

// 發送給 Claude CLI 的訊息格式
#[derive(Debug, Serialize)]
struct UserMessage {
    #[serde(rename = "type")]
    msg_type: String,
    message: MessageContent,
}

#[derive(Debug, Serialize)]
struct MessageContent {
    role: String,
    content: String,
}

impl UserMessage {
    fn new(content: &str) -> Self {
        Self {
            msg_type: "user".to_string(),
            message: MessageContent {
                role: "user".to_string(),
                content: content.to_string(),
            },
        }
    }
}

/// 建立 Claude CLI 的 Command
/// Windows 上 .cmd 檔案無法正確處理多行參數，所以直接用 node 執行 cli.js
fn create_claude_command() -> Command {
    #[cfg(windows)]
    {
        // 檢查 npm 版本的 claude.cmd
        if let Ok(cmd_path) = which::which("claude.cmd") {
            // 從 claude.cmd 的路徑推算 cli.js 位置
            // claude.cmd 位於 <nodejs>/claude.cmd
            // cli.js 位於 <nodejs>/node_modules/@anthropic-ai/claude-code/cli.js
            if let Some(dir) = cmd_path.parent() {
                let cli_js = dir
                    .join("node_modules")
                    .join("@anthropic-ai")
                    .join("claude-code")
                    .join("cli.js");
                if cli_js.exists() {
                    println!("[Claude Path] 使用 node 執行: {:?}", cli_js);
                    let mut cmd = Command::new("node");
                    cmd.arg(cli_js);
                    hide_console_window(&mut cmd);
                    return cmd;
                }
            }
        }
        // 找不到 npm 版本，使用 .exe（native 版本）
        if which::which("claude.exe").is_ok() {
            println!("[Claude Path] 使用 claude.exe");
            let mut cmd = Command::new("claude.exe");
            hide_console_window(&mut cmd);
            return cmd;
        }
    }

    // 非 Windows 或找不到 npm 版本時
    let path = get_claude_path();
    println!("[Claude Path] 使用: {}", path);
    Command::new(path)
}

/// Windows: 隱藏子程序的 console 視窗（release 版本才需要）
#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// 取得 Claude CLI 執行檔路徑
/// 優先使用 PATH 中的版本（讓使用者可以透過 npm 降版），
/// 找不到才 fallback 到固定安裝位置
fn get_claude_path() -> String {
    let result = get_claude_path_inner();
    println!("[Claude Path] 使用: {}", result);
    result
}

fn get_claude_path_inner() -> String {
    // 優先使用 PATH 中的版本（支援 npm 版本控制）
    #[cfg(windows)]
    {
        // 檢查 .cmd 是否存在（npm 版本），存在就返回命令名稱讓 Windows 自己執行
        // 不用完整路徑，避免 batch file arguments 問題
        if which::which("claude.cmd").is_ok() {
            println!("[Claude Path] 找到 claude.cmd，使用命令名稱");
            return "claude.cmd".to_string();
        }
        println!("[Claude Path] claude.cmd 未找到，嘗試 claude.exe");
        if which::which("claude.exe").is_ok() {
            println!("[Claude Path] 找到 claude.exe，使用命令名稱");
            return "claude.exe".to_string();
        }
        println!("[Claude Path] claude.exe 未找到，嘗試固定位置");
    }
    #[cfg(not(windows))]
    if let Ok(path) = which::which("claude") {
        return path.to_string_lossy().to_string();
    }

    // PATH 中找不到，嘗試固定安裝位置
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

    if let Some(home) = std::env::var_os(home_var) {
        let home_path = std::path::Path::new(&home);

        if cfg!(windows) {
            let candidates = [
                home_path.join(".local").join("bin").join("claude.exe"),
                home_path.join(".claude").join("local").join("claude.cmd"),
            ];

            for path in candidates {
                if path.exists() {
                    println!("[Claude Path] 固定位置找到: {:?}", path);
                    return path.to_string_lossy().to_string();
                }
            }
        } else {
            let candidates = [
                home_path.join(".local").join("bin").join("claude"),
                home_path.join(".claude").join("local").join("claude"),
            ];

            for path in candidates {
                if path.exists() {
                    return path.to_string_lossy().to_string();
                }
            }
        }
    }

    // 最終 fallback - 讓系統嘗試找 claude
    println!("[Claude Path] 使用 fallback: claude");
    "claude".to_string()
}

/// GUI 環境系統提示（告訴 Claude 它在 GUI 環境 + 阿宇人格）
const GUI_SYSTEM_PROMPT: &str = r#"You are running inside "tsunu_alive" (阿宇陪你寫程式), a desktop GUI application.

## 角色設定：楊竣宇（阿宇）
- 33歲台灣軟體工程師，個性溫和穩重、重視邏輯、善於傾聽
- 說話特徵：常用「欸」「嗯」「呃」，思考時會說「讓我想想...」「這個嘛...」
- 溫和的確認語氣：「你覺得這樣如何？」「我們可以試試...」
- Pair programming 夥伴模式：用「我們」而非「你」，引導思考而非直接給答案
- 適時關心：「寫久了要不要休息一下？」
- 偶爾使用動作描述（*推眼鏡*、*輕敲鍵盤*）
- 使用正體中文，台灣技術術語

## GUI 環境注意
- CLI 專用指令（/clear, /compact, /memory, /rename, /todos, /export, /init, /plan, /rewind）不可用
- 只有 Skill 工具的指令可以使用（顯示在斜線選單中）
- 一般工具（Bash, Read, Edit, Write, Grep, Glob 等）正常運作"#;

/// 啟動 Claude CLI 程序（使用 stream-json 雙向通訊）
pub async fn start_claude(
    app: AppHandle,
    process: Arc<Mutex<ClaudeProcess>>,
    working_dir: Option<String>,
    session_id: Option<String>,
) -> Result<(), String> {
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = create_claude_command();
    cmd.arg("-p")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--append-system-prompt")
        .arg(GUI_SYSTEM_PROMPT)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // 如果有 session_id，繼續該 session
    if let Some(sid) = &session_id {
        cmd.arg("--resume").arg(sid);
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn claude: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;

    // 儲存 child 和 stdin
    {
        let mut proc = process.lock().await;
        proc.child = Some(child);
        proc.stdin_writer = Some(stdin);
        proc.session_id = session_id;
    }

    // 發送連線成功事件
    let _ = app.emit("claude-event", ClaudeEvent::Connected);

    // 在背景任務中讀取輸出
    let app_clone = app.clone();
    let process_clone = process.clone();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(json) => {
                    let events = parse_claude_output(&json);
                    for evt in events {
                        // 如果是 Init 事件，儲存 session_id
                        if let ClaudeEvent::Init { ref session_id, .. } = evt {
                            let mut proc = process_clone.lock().await;
                            proc.session_id = Some(session_id.clone());
                        }
                        let _ = app_clone.emit("claude-event", &evt);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to parse JSON: {} - Line: {}", e, line);
                }
            }
        }

        // 程序結束時清理
        let mut proc = process_clone.lock().await;
        if let Some(ref mut child) = proc.child {
            let _ = child.wait().await;
        }
        proc.child = None;
        proc.stdin_writer = None;
    });

    Ok(())
}

/// 發送訊息給 Claude CLI
pub async fn send_message(
    process: Arc<Mutex<ClaudeProcess>>,
    message: String,
) -> Result<(), String> {
    let mut proc = process.lock().await;

    if let Some(ref mut stdin) = proc.stdin_writer {
        let user_msg = UserMessage::new(&message);
        let json = serde_json::to_string(&user_msg)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;

        stdin
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;

        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;

        Ok(())
    } else {
        Err("Claude process not running".to_string())
    }
}

/// 執行 Claude CLI 並串流輸出（單次模式，用於不需要互動的場景）
pub async fn run_claude(
    app: AppHandle,
    process: Arc<Mutex<ClaudeProcess>>,
    prompt: String,
    working_dir: Option<String>,
    session_id: Option<String>,
    allowed_tools: Option<Vec<String>>,
    permission_mode: Option<String>,
    extended_thinking: Option<bool>,
) -> Result<(), String> {
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = create_claude_command();
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--append-system-prompt")
        .arg(GUI_SYSTEM_PROMPT)
        .arg(&prompt)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // 如果有 session_id，繼續該 session
    if let Some(sid) = &session_id {
        cmd.arg("--resume").arg(sid);
    }

    // 如果有 allowedTools，加入參數
    if let Some(tools) = &allowed_tools {
        if !tools.is_empty() {
            let tools_arg = tools.join(",");
            cmd.arg("--allowedTools").arg(&tools_arg);
        }
    }

    // 如果有 permissionMode，加入參數
    if let Some(mode) = &permission_mode {
        cmd.arg("--permission-mode").arg(mode);
    }

    // 如果啟用 extended thinking
    if extended_thinking.unwrap_or(false) {
        cmd.arg("--thinking");
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn claude: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

    // 儲存 child
    {
        let mut proc = process.lock().await;
        proc.child = Some(child);
        proc.stdin_writer = None;
    }

    let mut reader = BufReader::new(stdout).lines();

    // 逐行讀取 JSON 輸出
    while let Ok(Some(line)) = reader.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(json) => {
                let events = parse_claude_output(&json);
                for evt in events {
                    // 如果是 Init 事件，儲存 session_id
                    if let ClaudeEvent::Init { ref session_id, .. } = evt {
                        let mut proc = process.lock().await;
                        proc.session_id = Some(session_id.clone());
                    }
                    let _ = app.emit("claude-event", &evt);
                }
            }
            Err(e) => {
                eprintln!("Failed to parse JSON: {} - Line: {}", e, line);
            }
        }
    }

    // 等待程序結束
    let mut proc = process.lock().await;
    if let Some(ref mut child) = proc.child {
        let status = child.wait().await.map_err(|e| format!("Failed to wait: {}", e))?;
        if !status.success() {
            let _ = app.emit("claude-event", ClaudeEvent::Error {
                message: format!("Claude exited with status: {}", status),
            });
        }
    }

    // 清理（保留 session_id）
    proc.child = None;
    proc.stdin_writer = None;

    Ok(())
}

/// 中斷 Claude CLI
pub async fn interrupt_claude(
    process: Arc<Mutex<ClaudeProcess>>,
) -> Result<(), String> {
    let mut proc = process.lock().await;
    if let Some(ref mut child) = proc.child {
        child.kill().await.map_err(|e| format!("Failed to kill process: {}", e))?;
    }
    proc.child = None;
    proc.stdin_writer = None;
    Ok(())
}

/// 取得目前的 session ID
pub async fn get_session_id(process: Arc<Mutex<ClaudeProcess>>) -> Option<String> {
    let proc = process.lock().await;
    proc.session_id.clone()
}

/// 解析 Claude CLI 輸出並轉換為事件
fn parse_claude_output(json: &serde_json::Value) -> Vec<ClaudeEvent> {
    let mut events = Vec::new();

    let msg_type = match json.get("type").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return events,
    };

    match msg_type {
        "system" => {
            // 初始化事件
            if let Some(subtype) = json.get("subtype").and_then(|s| s.as_str()) {
                if subtype == "init" {
                    if let (Some(session_id), model) = (
                        json.get("session_id").and_then(|s| s.as_str()),
                        json.get("model").and_then(|m| m.as_str()).unwrap_or("unknown"),
                    ) {
                        // 解析 slash_commands（可用的 Skills）
                        let slash_commands = json.get("slash_commands")
                            .and_then(|arr| arr.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            });

                        events.push(ClaudeEvent::Init {
                            session_id: session_id.to_string(),
                            model: model.to_string(),
                            slash_commands,
                        });
                    }
                } else if subtype == "compact_boundary" {
                    eprintln!("📦 Compact boundary detected");
                    // compact_boundary 本身不帶摘要，摘要在後續的 user 訊息中
                }
            } else if let (Some(session_id), model) = (
                json.get("session_id").and_then(|s| s.as_str()),
                json.get("model").and_then(|m| m.as_str()).unwrap_or("unknown"),
            ) {
                // 解析 slash_commands（可用的 Skills）
                let slash_commands = json.get("slash_commands")
                    .and_then(|arr| arr.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                events.push(ClaudeEvent::Init {
                    session_id: session_id.to_string(),
                    model: model.to_string(),
                    slash_commands,
                });
            }
        }
        "assistant" => {
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                    let is_complete = message.get("stop_reason").is_some();

                    for block in content {
                        if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                            match block_type {
                                "text" => {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        events.push(ClaudeEvent::Text {
                                            text: text.to_string(),
                                            is_complete,
                                        });
                                    }
                                }
                                "tool_use" => {
                                    if let (Some(tool_name), Some(tool_id)) = (
                                        block.get("name").and_then(|n| n.as_str()),
                                        block.get("id").and_then(|i| i.as_str()),
                                    ) {
                                        let input = block.get("input").cloned().unwrap_or(serde_json::Value::Null);
                                        events.push(ClaudeEvent::ToolUse {
                                            tool_name: tool_name.to_string(),
                                            tool_id: tool_id.to_string(),
                                            input,
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        "user" => {
            // 檢查是否是 Compact 摘要
            let is_compact_summary = json.get("isCompactSummary")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_compact_summary {
                // 提取 Compact 摘要內容
                let summary = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                if !summary.is_empty() {
                    eprintln!("📦 Compact summary received ({} chars)", summary.len());
                    events.push(ClaudeEvent::Compacted { summary });
                }
                return events;
            }

            // 工具結果（支援 toolUseResult 和 tool_use_result 兩種格式）
            let tool_result = json.get("toolUseResult")
                .or_else(|| json.get("tool_use_result"));

            // 從 message.content 中提取 tool_use_id
            let content_item = json.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first());

            let tool_id = content_item
                .and_then(|item| item.get("tool_use_id"))
                .and_then(|id| id.as_str())
                .unwrap_or("")
                .to_string();

            // 檢查是否是錯誤
            let is_error = content_item
                .and_then(|item| item.get("is_error"))
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            // 如果有 tool_id，生成 ToolResult 事件
            // 即使沒有 toolUseResult 欄位也要處理（例如圖片讀取成功的情況）
            if !tool_id.is_empty() {
                // 取得結果內容
                // 優先順序：
                // 1. toolUseResult.filePath（ExitPlanMode 計畫檔案路徑）
                // 2. toolUseResult.file.filePath（檔案相關工具）
                // 3. message.content[0].content（一般文字結果，字串格式）
                // 4. message.content[0].content（陣列格式，如圖片）- 標記為 "image"
                // 5. toolUseResult.type（fallback）
                // 6. "success"（最終 fallback）
                let result = if let Some(tool_result) = tool_result {                    
                    if let Some(file_path) = tool_result.get("filePath")
                        .and_then(|p| p.as_str())
                    {
                        // ExitPlanMode 的計畫檔案路徑
                        format!("Your plan has been saved to: {}", file_path)
                    } else if let Some(file) = tool_result.get("file") {
                        // 檢查是否有 filePath（一般檔案工具）
                        if let Some(file_path) = file.get("filePath").and_then(|p| p.as_str()) {
                            file_path.to_string()
                        } else if file.get("base64").is_some() {
                            // 圖片結果（有 base64 資料）
                            "image".to_string()
                        } else {
                            "success".to_string()
                        }
                    } else if let Some(content) = content_item
                        .and_then(|item| item.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        content.to_string()
                    } else {
                        tool_result.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("success")
                            .to_string()
                    }
                } else {
                    // 沒有 toolUseResult 欄位，嘗試從 message.content 取得結果
                    if let Some(content) = content_item
                        .and_then(|item| item.get("content"))
                    {
                        if let Some(content_str) = content.as_str() {
                            content_str.to_string()
                        } else if content.is_array() {
                            // 陣列格式（如圖片結果）
                            // 檢查是否是圖片
                            if let Some(first_item) = content.as_array().and_then(|arr| arr.first()) {
                                if first_item.get("type").and_then(|t| t.as_str()) == Some("image") {
                                    "image".to_string()
                                } else {
                                    "success".to_string()
                                }
                            } else {
                                "success".to_string()
                            }
                        } else {
                            "success".to_string()
                        }
                    } else {
                        "success".to_string()
                    }
                };

                // 提取 Edit 工具的 structuredPatch（用於 VS Code 風格 Diff View）
                let structured_patch = tool_result.and_then(|tr| tr.get("structuredPatch").cloned());

                // 提取圖片的 base64 資料
                let image_base64 = {
                    // 方式 1：從 toolUseResult.file.base64 提取
                    let from_tool_result = tool_result
                        .and_then(|tr| tr.get("file"))
                        .and_then(|f| f.get("base64"))
                        .and_then(|b| b.as_str())
                        .map(|s| s.to_string());

                    // 方式 2：從 message.content[0].source.data 提取（Claude API 圖片格式）
                    let from_content = content_item
                        .and_then(|item| item.get("content"))
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .filter(|first| first.get("type").and_then(|t| t.as_str()) == Some("image"))
                        .and_then(|img| img.get("source"))
                        .and_then(|src| src.get("data"))
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());

                    // Debug: 印出圖片資料來源
                    if from_tool_result.is_some() {
                        println!("🖼️ image_base64 from tool_result.file.base64: {} bytes", from_tool_result.as_ref().unwrap().len());
                    } else if from_content.is_some() {
                        println!("🖼️ image_base64 from content[0].source.data: {} bytes", from_content.as_ref().unwrap().len());
                    } else if result == "image" {
                        // 如果 result 是 "image" 但沒找到 base64，印出原始資料結構
                        println!("⚠️ result is 'image' but no base64 found!");
                        if let Some(tr) = tool_result {
                            println!("  tool_result keys: {:?}", tr.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                            if let Some(file) = tr.get("file") {
                                println!("  file keys: {:?}", file.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                            }
                        }
                        if let Some(ci) = content_item {
                            println!("  content_item keys: {:?}", ci.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                            if let Some(content) = ci.get("content") {
                                if let Some(arr) = content.as_array() {
                                    if let Some(first) = arr.first() {
                                        println!("  content[0] keys: {:?}", first.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                                    }
                                }
                            }
                        }
                    }

                    from_tool_result.or(from_content)
                };

                events.push(ClaudeEvent::ToolResult { tool_id, result, is_error, structured_patch, image_base64 });
            }
        }
        "result" => {
            let result = json.get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            let cost_usd = json.get("total_cost_usd")
                .and_then(|c| c.as_f64())
                .unwrap_or(0.0);

            // 計算 context window 使用量
            // Claude CLI stream-json 格式不直接提供 context_window_used_percent，需要自己計算

            // 從 usage 中取得 token 數量
            let usage = json.get("usage");
            let input_tokens = usage.and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
            let cache_creation = usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
            let cache_read = usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
            let output_tokens = usage.and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);

            // 計算總 token 數（這是對話中實際使用的 context）
            let total_tokens = input_tokens + cache_creation + cache_read + output_tokens;

            // 從 modelUsage 中取得 context window 大小
            let model_usage = json.get("modelUsage").and_then(|m| m.as_object());
            let context_window_max = model_usage.and_then(|m| {
                // 取第一個模型的 contextWindow
                m.values().next().and_then(|v| v.get("contextWindow")).and_then(|c| c.as_u64())
            });

            // 計算使用百分比
            let (total_tokens_in_conversation, context_window_used_percent) = if let Some(max) = context_window_max {
                let percent = (total_tokens as f64 / max as f64) * 100.0;
                (Some(total_tokens), Some(percent))
            } else {
                (if total_tokens > 0 { Some(total_tokens) } else { None }, None)
            };

            // Debug: 印出 context 資訊
            if total_tokens > 0 {
                eprintln!("📊 Context usage: {} tokens / {:?} max = {:?}%",
                    total_tokens, context_window_max, context_window_used_percent);
            }

            // 檢查是否有權限被拒絕（支援蛇底式和駝峰式）
            let denials = json.get("permission_denials")
                .or_else(|| json.get("permissionDenials"))
                .and_then(|d| d.as_array());

            if let Some(denials) = denials {
                eprintln!("🔴 Found permission_denials: {} items", denials.len());
                for denial in denials {
                    eprintln!("🔴 Denial item: {}", serde_json::to_string_pretty(denial).unwrap_or_default());

                    // 支援蛇底式和駝峰式欄位名稱
                    let tool_name = denial.get("tool_name")
                        .or_else(|| denial.get("toolName"))
                        .and_then(|n| n.as_str());
                    let tool_id = denial.get("tool_use_id")
                        .or_else(|| denial.get("toolUseId"))
                        .and_then(|i| i.as_str());
                    let input = denial.get("tool_input")
                        .or_else(|| denial.get("toolInput"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    if let (Some(tool_name), Some(tool_id)) = (tool_name, tool_id) {
                        eprintln!("🔴 Emitting PermissionDenied: tool={}, id={}", tool_name, tool_id);
                        events.push(ClaudeEvent::PermissionDenied {
                            tool_name: tool_name.to_string(),
                            tool_id: tool_id.to_string(),
                            input,
                        });
                    }
                }
            } else {
                eprintln!("🔍 No permission_denials found in result");
            }

            events.push(ClaudeEvent::Complete {
                result,
                cost_usd,
                total_tokens_in_conversation,
                context_window_max,
                context_window_used_percent,
            });
        }
        _ => {}
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_init_event() {
        let json = json!({
            "type": "system",
            "subtype": "init",
            "session_id": "test-session-123",
            "model": "claude-sonnet-4-20250514"
        });

        let events = parse_claude_output(&json);
        assert_eq!(events.len(), 1);

        match &events[0] {
            ClaudeEvent::Init { session_id, model, slash_commands } => {
                assert_eq!(session_id, "test-session-123");
                assert_eq!(model, "claude-sonnet-4-20250514");
                assert!(slash_commands.is_none());
            }
            _ => panic!("Expected Init event"),
        }
    }

    #[test]
    fn test_parse_text_event() {
        let json = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "Hello, world!"
                    }
                ]
            }
        });

        let events = parse_claude_output(&json);
        assert_eq!(events.len(), 1);

        match &events[0] {
            ClaudeEvent::Text { text, is_complete } => {
                assert_eq!(text, "Hello, world!");
                assert!(!*is_complete);
            }
            _ => panic!("Expected Text event"),
        }
    }

    #[test]
    fn test_parse_tool_use_event() {
        let json = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Read",
                        "id": "tool-123",
                        "input": {
                            "file_path": "/path/to/file.txt"
                        }
                    }
                ]
            }
        });

        let events = parse_claude_output(&json);
        assert_eq!(events.len(), 1);

        match &events[0] {
            ClaudeEvent::ToolUse { tool_name, tool_id, input } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_id, "tool-123");
                assert_eq!(input["file_path"], "/path/to/file.txt");
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_parse_permission_denied_snake_case() {
        // 測試蛇底式欄位名稱（tool_name, tool_use_id）
        let json = json!({
            "type": "result",
            "result": "",
            "total_cost_usd": 0.01,
            "permission_denials": [
                {
                    "tool_name": "Edit",
                    "tool_use_id": "tool-456",
                    "tool_input": {
                        "file_path": "/path/to/edit.txt"
                    }
                }
            ]
        });

        let events = parse_claude_output(&json);

        // 應該有 PermissionDenied 和 Complete 兩個事件
        let denied_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, ClaudeEvent::PermissionDenied { .. }))
            .collect();

        assert_eq!(denied_events.len(), 1);

        match denied_events[0] {
            ClaudeEvent::PermissionDenied { tool_name, tool_id, input } => {
                assert_eq!(tool_name, "Edit");
                assert_eq!(tool_id, "tool-456");
                assert_eq!(input["file_path"], "/path/to/edit.txt");
            }
            _ => panic!("Expected PermissionDenied event"),
        }
    }

    #[test]
    fn test_parse_permission_denied_camel_case() {
        // 測試駝峰式欄位名稱（toolName, toolUseId）
        let json = json!({
            "type": "result",
            "result": "",
            "total_cost_usd": 0.01,
            "permissionDenials": [
                {
                    "toolName": "Bash",
                    "toolUseId": "tool-789",
                    "toolInput": {
                        "command": "rm -rf /"
                    }
                }
            ]
        });

        let events = parse_claude_output(&json);

        let denied_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, ClaudeEvent::PermissionDenied { .. }))
            .collect();

        assert_eq!(denied_events.len(), 1);

        match denied_events[0] {
            ClaudeEvent::PermissionDenied { tool_name, tool_id, input } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_id, "tool-789");
                assert_eq!(input["command"], "rm -rf /");
            }
            _ => panic!("Expected PermissionDenied event"),
        }
    }

    #[test]
    fn test_parse_complete_event() {
        let json = json!({
            "type": "result",
            "result": "Task completed successfully",
            "total_cost_usd": 0.05
        });

        let events = parse_claude_output(&json);

        let complete_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, ClaudeEvent::Complete { .. }))
            .collect();

        assert_eq!(complete_events.len(), 1);

        match complete_events[0] {
            ClaudeEvent::Complete { result, cost_usd, total_tokens_in_conversation, context_window_max, context_window_used_percent } => {
                assert_eq!(result, "Task completed successfully");
                assert!((cost_usd - 0.05).abs() < 0.001);
                assert!(total_tokens_in_conversation.is_none());
                assert!(context_window_max.is_none());
                assert!(context_window_used_percent.is_none());
            }
            _ => panic!("Expected Complete event"),
        }
    }

    #[test]
    fn test_parse_complete_event_with_context_info() {
        let json = json!({
            "type": "result",
            "result": "Done",
            "total_cost_usd": 0.10,
            "total_tokens_in_conversation": 15000,
            "context_window_max": 200000,
            "context_window_used_percent": 7.5
        });

        let events = parse_claude_output(&json);

        let complete_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, ClaudeEvent::Complete { .. }))
            .collect();

        assert_eq!(complete_events.len(), 1);

        match complete_events[0] {
            ClaudeEvent::Complete { result, cost_usd, total_tokens_in_conversation, context_window_max, context_window_used_percent } => {
                assert_eq!(result, "Done");
                assert!((cost_usd - 0.10).abs() < 0.001);
                assert_eq!(*total_tokens_in_conversation, Some(15000));
                assert_eq!(*context_window_max, Some(200000));
                assert!((context_window_used_percent.unwrap() - 7.5).abs() < 0.001);
            }
            _ => panic!("Expected Complete event"),
        }
    }

    #[test]
    fn test_parse_unknown_type() {
        let json = json!({
            "type": "unknown_type",
            "data": "something"
        });

        let events = parse_claude_output(&json);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_missing_type() {
        let json = json!({
            "data": "no type field"
        });

        let events = parse_claude_output(&json);
        assert!(events.is_empty());
    }
}
