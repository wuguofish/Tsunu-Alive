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
        // 本次 turn 的 input token 數（用於追蹤 system-reminder 注入造成的 token 膨脹）
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens_this_turn: Option<u64>,
    },
    // Compact 完成（對話摘要壓縮）
    Compacted {
        summary: String,
    },
    // API 錯誤（isApiErrorMessage: true，例如 "Request too large"）
    ApiError {
        error_code: String,
        message: String,
    },
    // 錯誤
    Error {
        message: String,
    },
    // 連線狀態
    Connected,
    // CLI process 已退出（互動模式下 process 結束時發送）
    ProcessExited,
}

// 全域的 Claude 程序管理
pub struct ClaudeProcess {
    pub child: Option<Child>,
    pub stdin_writer: Option<tokio::process::ChildStdin>,
    pub session_id: Option<String>,
    /// 目前的權限模式（由前端設定，用於 control_request 判斷）
    pub permission_mode: Option<String>,
    /// 暫存 pending permission request 的 tool_input，key = request_id
    /// 前端確認後回應時需要帶回 updatedInput
    pub pending_tool_inputs: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for ClaudeProcess {
    fn default() -> Self {
        Self {
            child: None,
            stdin_writer: None,
            session_id: None,
            permission_mode: None,
            pending_tool_inputs: std::collections::HashMap::new(),
        }
    }
}

// 發送給 Claude CLI 的訊息格式（互動模式，匹配 VS Code extension 格式）
#[derive(Debug, Serialize)]
struct InteractiveUserMessage {
    #[serde(rename = "type")]
    msg_type: String,
    session_id: String,
    parent_tool_use_id: Option<String>,
    #[serde(rename = "isSynthetic")]
    is_synthetic: bool,
    message: InteractiveMessageContent,
}

#[derive(Debug, Serialize)]
struct InteractiveMessageContent {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

impl InteractiveUserMessage {
    fn new(content: &str) -> Self {
        Self {
            msg_type: "user".to_string(),
            session_id: String::new(),
            parent_tool_use_id: None,
            is_synthetic: false,
            message: InteractiveMessageContent {
                role: "user".to_string(),
                content: vec![ContentBlock {
                    block_type: "text".to_string(),
                    text: content.to_string(),
                }],
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

/// 啟動 Claude CLI 程序（互動式長駐模式，匹配 VS Code extension 架構）
///
/// 不使用 `-p` 單次模式，而是透過 stdin/stdout 雙向 JSON 通訊。
/// Process 會持續運行，直到被 interrupt_claude() 終止或 CLI 自行退出。
pub async fn start_claude(
    app: AppHandle,
    process: Arc<Mutex<ClaudeProcess>>,
    working_dir: Option<String>,
    session_id: Option<String>,
    permission_mode: Option<String>,
    thinking_mode: Option<String>,
    channels: Option<Vec<String>>,
) -> Result<(), String> {
    // 先中斷舊的 process（如果有的話）
    {
        let mut proc = process.lock().await;
        if let Some(ref mut child) = proc.child {
            let _ = child.kill().await;
        }
        proc.child = None;
        proc.stdin_writer = None;
    }

    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = create_claude_command();
    // 避免 "nested session" 檢查（僅開發環境：在 Claude Code terminal 裡跑 tauri dev 時會遇到）
    #[cfg(debug_assertions)]
    cmd.env_remove("CLAUDECODE");
    // 互動模式核心參數（不帶 -p）
    cmd.arg("--input-format").arg("stream-json")
        .arg("--output-format").arg("stream-json")
        .arg("--verbose")
        .arg("--permission-prompt-tool").arg("stdio")
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // 如果有 session_id，繼續該 session
    if let Some(sid) = &session_id {
        cmd.arg("--resume").arg(sid);
    }

    // 權限模式：只有 plan 模式需要傳給 CLI（影響 Claude 行為，不只是權限）
    // 其他模式（default, acceptEdits, bypassPermissions）由我們透過 control_request 自行處理
    if let Some(ref mode) = permission_mode {
        if mode == "plan" {
            cmd.arg("--permission-mode").arg(mode);
        }
    }

    // Thinking 模式（adaptive / enabled）— off 時不傳參數
    if let Some(ref mode) = thinking_mode {
        if mode != "off" {
            cmd.arg("--thinking").arg(mode);
        }
    }

    // Channels（例如 Discord 插件的即時通訊模式）
    if let Some(ref ch_list) = channels {
        for ch in ch_list {
            cmd.arg("--channels").arg(ch);
        }
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn claude: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;

    // 讀取 stderr 並輸出到 eprintln（debug 用）
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("[Claude CLI stderr] {}", line);
        }
    });

    // 儲存 child 和 stdin
    {
        let mut proc = process.lock().await;
        proc.child = Some(child);
        proc.stdin_writer = Some(stdin);
        proc.session_id = session_id;
        proc.permission_mode = permission_mode;
    }

    // 送出 initialize 訊息（透過 control_request 包裝，與 VS Code extension SDK 格式一致）
    // SDK 的 request() 方法會將 initialize 包成 control_request 送到 CLI stdin，
    // CLI 處理完後會回送 control_response（在 stdout reader 中忽略即可）
    {
        let request_id = format!("init-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());
        let init_msg = serde_json::json!({
            "type": "control_request",
            "request_id": request_id,
            "request": {
                "subtype": "initialize",
                "hooks": {},
                "sdkMcpServers": serde_json::Value::Null,
                "jsonSchema": serde_json::Value::Null,
                "systemPrompt": serde_json::Value::Null,
                "appendSystemPrompt": GUI_SYSTEM_PROMPT,
                "agents": serde_json::Value::Null,
                "promptSuggestions": serde_json::Value::Null,
                "agentProgressSummaries": serde_json::Value::Null
            }
        });
        // 送出 initialize 訊息
        let mut proc = process.lock().await;
        if let Some(ref mut stdin) = proc.stdin_writer {
            let json_str = serde_json::to_string(&init_msg)
                .map_err(|e| format!("Failed to serialize initialize: {}", e))?;
            stdin.write_all(format!("{}\n", json_str).as_bytes()).await
                .map_err(|e| format!("Failed to write initialize: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush initialize: {}", e))?;
        }
    }

    // 發送連線成功事件
    let _ = app.emit("claude-event", ClaudeEvent::Connected);

    // 在背景任務中讀取輸出
    let app_clone = app.clone();
    let process_clone = process.clone();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut parser = ClaudeEventParser::new();

        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(json) => {
                    // 檢查是否為 control_request（權限審核等）
                    let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if msg_type == "control_request" {
                        handle_control_request(&app_clone, &process_clone, &json).await;
                        continue;
                    }
                    // 忽略 keep_alive
                    if msg_type == "keep_alive" {
                        continue;
                    }
                    // control_response（CLI 回應我們的 control_request，例如 initialize）
                    if msg_type == "control_response" {
                        // 從 initialize 的成功回應中提取 commands → 轉成 Init 事件
                        if let Some(resp) = json.get("response") {
                            if resp.get("subtype").and_then(|s| s.as_str()) == Some("success") {
                                if let Some(inner) = resp.get("response") {
                                    // 提取 slash_commands（從 commands 陣列的 name 欄位）
                                    let slash_commands = inner.get("commands")
                                        .and_then(|arr| arr.as_array())
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                                                .collect::<Vec<String>>()
                                        });

                                    // 提取 session_id（從 pid 來推斷，或者等 stdout 的 init 事件）
                                    // init 的 control_response 沒有 session_id，但有 pid
                                    let pid = inner.get("pid").and_then(|p| p.as_u64());
                                    // 先發送一個只帶 slash_commands 的 Init 事件
                                    // session_id 會在後續的 stdout init 事件中取得
                                    if slash_commands.is_some() {
                                        let _ = app_clone.emit("claude-event", ClaudeEvent::Init {
                                            session_id: String::new(),
                                            model: String::new(),
                                            slash_commands,
                                        });
                                    }
                                }
                            }
                        }

                        continue;
                    }
                    // 忽略 streamlined 類型
                    if msg_type == "streamlined_text" || msg_type == "streamlined_tool_use_summary" {
                        continue;
                    }

                    let events = parser.parse(&json);
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

        // 程序結束時清理並通知前端
        let mut proc = process_clone.lock().await;
        if let Some(ref mut child) = proc.child {
            match child.wait().await {
                Ok(status) if !status.success() => {
                    let _ = app_clone.emit("claude-event", ClaudeEvent::Error {
                        message: format!("Claude CLI exited with status: {}", status),
                    });
                }
                Err(e) => {
                    let _ = app_clone.emit("claude-event", ClaudeEvent::Error {
                        message: format!("Failed to wait for Claude CLI: {}", e),
                    });
                }
                _ => {}
            }
        }
        proc.child = None;
        proc.stdin_writer = None;
        let _ = app_clone.emit("claude-event", ClaudeEvent::ProcessExited);
    });

    Ok(())
}

/// 發送訊息給 Claude CLI（互動模式，透過 stdin）
pub async fn send_message(
    process: Arc<Mutex<ClaudeProcess>>,
    message: String,
) -> Result<(), String> {
    let mut proc = process.lock().await;

    if let Some(ref mut stdin) = proc.stdin_writer {
        let user_msg = InteractiveUserMessage::new(&message);
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

/// 不需要用戶確認的工具列表（與 permission_server.rs 和前端 autoAllowTools.ts 同步）
const AUTO_ALLOW_TOOLS: &[&str] = &[
    "AskUserQuestion", "TodoRead", "TodoWrite",
    "Read", "Glob", "Grep",
    "Task", "TaskOutput",
    "WebSearch", "WebFetch",
    "CronCreate", "CronDelete", "CronList",
    "EnterPlanMode",
];

/// 處理來自 CLI 的 control_request（如權限審核）
///
/// CLI 在需要權限審核時會透過 stdout 送出 control_request，
/// 我們需要透過 stdin 回送 control_response。
async fn handle_control_request(
    app: &AppHandle,
    process: &Arc<Mutex<ClaudeProcess>>,
    json: &serde_json::Value,
) {
    let request_id = json.get("request_id")
        .or_else(|| json.get("request").and_then(|r| r.get("request_id")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Debug: 印出完整 control_request 結構
    // 處理 Claude CLI 發來的 control_request（權限請求等）

    // 嘗試取得工具名稱（permission prompt 的 control_request 格式）
    let tool_name = json.get("request")
        .and_then(|r| r.get("tool_name").or_else(|| r.get("toolName")))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let tool_use_id = json.get("request")
        .and_then(|r| r.get("tool_use_id").or_else(|| r.get("toolUseId")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tool_input = json.get("request")
        .and_then(|r| r.get("tool_input").or_else(|| r.get("input")))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // 根據 permission_mode 決定是否自動放行
    let permission_mode = {
        let proc = process.lock().await;
        proc.permission_mode.clone().unwrap_or_else(|| "default".to_string())
    };

    // AskUserQuestion 永遠不 auto-allow：需要透過 control_response 的 updatedInput 回傳用戶答案
    let should_auto_allow = if tool_name == "AskUserQuestion" {
        false
    } else {
        match permission_mode.as_str() {
            // bypassPermissions：所有工具自動放行（ExitPlanMode 例外，需使用者確認）
            "bypassPermissions" => tool_name != "ExitPlanMode",
            // acceptEdits：AUTO_ALLOW_TOOLS + 編輯相關工具自動放行
            "acceptEdits" => {
                AUTO_ALLOW_TOOLS.contains(&tool_name)
                    || matches!(tool_name, "Edit" | "Write" | "NotebookEdit" | "Bash")
            }
            // default / plan：只有 AUTO_ALLOW_TOOLS 自動放行
            _ => AUTO_ALLOW_TOOLS.contains(&tool_name),
        }
    };

    if should_auto_allow {
        let _ = send_control_response(process, &request_id, "allow", Some(&tool_use_id), Some(&tool_input), None).await;
        return;
    }

    // 需要用戶確認，emit 事件到前端
    // 前端收到後會顯示 PermissionDialog，用戶選擇後呼叫 respond_to_permission
    // request_id: control_request 的 ID（用於回送 control_response）
    // tool_use_id: CLI 工具呼叫的 ID（用於 control_response 的 toolUseID 欄位）

    // 暫存 tool_input，前端確認後回應時需要帶回 updatedInput
    {
        let mut proc = process.lock().await;
        proc.pending_tool_inputs.insert(request_id.clone(), tool_input.clone());
    }

    let _ = app.emit("permission-request", serde_json::json!({
        "tool_use_id": request_id,
        "tool_name": tool_name,
        "tool_input": tool_input,
        "session_id": "",
        "cli_tool_use_id": tool_use_id,
    }));
}

/// 透過 stdin 回送 control_response 給 CLI
///
/// CLI 的 Zod 驗證期望 response.response 是一個 union type：
/// - 允許：`{ updatedInput: Record<string, any>, toolUseID: string }` （必須有 updatedInput）
/// - 拒絕：`{ behavior: "deny", message: string, toolUseID: string }`
pub async fn send_control_response(
    process: &Arc<Mutex<ClaudeProcess>>,
    request_id: &str,
    behavior: &str,
    tool_use_id: Option<&str>,
    tool_input: Option<&serde_json::Value>,
    message: Option<&str>,
) -> Result<(), String> {
    let response_data = if behavior == "allow" {
        // 允許：必須同時帶 behavior + updatedInput（Zod union 要求兩者都有）
        // VS Code extension: return { behavior: "allow", updatedInput: U, toolUseID: ... }
        let mut data = serde_json::json!({
            "behavior": "allow",
            "updatedInput": tool_input.unwrap_or(&serde_json::Value::Object(serde_json::Map::new()))
        });
        if let Some(id) = tool_use_id {
            data["toolUseID"] = serde_json::Value::String(id.to_string());
        }
        data
    } else {
        // 拒絕：帶 behavior: "deny" + message
        let mut data = serde_json::json!({
            "behavior": "deny"
        });
        if let Some(id) = tool_use_id {
            data["toolUseID"] = serde_json::Value::String(id.to_string());
        }
        if let Some(msg) = message {
            data["message"] = serde_json::Value::String(msg.to_string());
        }
        data
    };

    let control_response = serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": response_data
        }
    });

    // 寫入 control_response 到 stdin

    let mut proc = process.lock().await;
    if let Some(ref mut stdin) = proc.stdin_writer {
        let json_str = serde_json::to_string(&control_response)
            .map_err(|e| format!("Failed to serialize control_response: {}", e))?;
        stdin.write_all(format!("{}\n", json_str).as_bytes()).await
            .map_err(|e| format!("Failed to write control_response: {}", e))?;
        stdin.flush().await
            .map_err(|e| format!("Failed to flush control_response: {}", e))?;
        Ok(())
    } else {
        Err("Claude process not running".to_string())
    }
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

/// Claude CLI 事件解析器（有狀態，追蹤每個 turn 的 token 用量）
struct ClaudeEventParser {
    /// 最近一次 assistant turn 的 input_tokens（代表當前 context window 使用量）
    last_turn_input_tokens: u64,
    /// 最近一次 assistant turn 的 output_tokens
    last_turn_output_tokens: u64,
}

impl ClaudeEventParser {
    fn new() -> Self {
        Self {
            last_turn_input_tokens: 0,
            last_turn_output_tokens: 0,
        }
    }
}

/// 解析 Claude CLI 輸出並轉換為事件（向下相容的包裝函式，用於測試）
#[cfg(test)]
fn parse_claude_output(json: &serde_json::Value) -> Vec<ClaudeEvent> {
    let mut parser = ClaudeEventParser::new();
    parser.parse(json)
}

impl ClaudeEventParser {
/// 解析 Claude CLI 輸出並轉換為事件
fn parse(&mut self, json: &serde_json::Value) -> Vec<ClaudeEvent> {
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
                    // compact_boundary 偵測到，後續 user 訊息中會有摘要
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
            // 優先檢查是否是 API 錯誤訊息（例如 "Request too large"）
            let is_api_error = json.get("isApiErrorMessage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_api_error {
                let error_code = json.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                // 從 message.content[0].text 提取錯誤訊息
                let message_text = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown API error")
                    .to_string();

                eprintln!("⚠️ API error detected: [{}] {}", error_code, message_text);
                events.push(ClaudeEvent::ApiError {
                    error_code,
                    message: message_text,
                });
                return events;
            }

            if let Some(message) = json.get("message") {
                // 追蹤每個 turn 的 token 用量
                // assistant 事件的 message.usage 包含該次 turn 的 input/output tokens
                // input_tokens 代表送入的完整 context 大小，是 context window 使用量的最佳近似值
                if let Some(usage) = message.get("usage") {
                    // 更新 token 使用量追蹤
                    // input_tokens 只是 non-cached 部分，要加上 cache tokens 才是真正的 context 大小
                    let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_create = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    self.last_turn_input_tokens = input + cache_create + cache_read;
                    if let Some(output) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                        self.last_turn_output_tokens = output;
                    }
                }

                if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                    // stop_reason 為 null 時代表訊息還在串流中，只有非 null 值才表示完成
                    let is_complete = message.get("stop_reason")
                        .map(|v| !v.is_null())
                        .unwrap_or(false);

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
            // 檢查是否是 Compact 摘要（isSynthetic=true 且非 isReplay）
            let is_synthetic = json.get("isSynthetic")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_replay = json.get("isReplay")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // 也支援舊格式 isCompactSummary
            let is_compact_legacy = json.get("isCompactSummary")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_compact_legacy || (is_synthetic && !is_replay) {
                // 提取 Compact 摘要內容（content 可能是字串或陣列）
                let summary = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| {
                        // 字串格式
                        if let Some(s) = c.as_str() {
                            return Some(s.to_string());
                        }
                        // 陣列格式 [{"type":"text","text":"..."}]
                        if let Some(arr) = c.as_array() {
                            let texts: Vec<String> = arr.iter()
                                .filter_map(|item| {
                                    item.get("text").and_then(|t| t.as_str())
                                        .or_else(|| item.get("content").and_then(|t| t.as_str()))
                                        .map(|s| s.to_string())
                                })
                                .collect();
                            if !texts.is_empty() {
                                return Some(texts.join("\n"));
                            }
                        }
                        None
                    })
                    .unwrap_or_default();

                if !summary.is_empty() {
                    // Compact 摘要收到
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
            // result 事件的 usage.input_tokens 是所有 turn 的累積值，不能直接用來算百分比
            // 改用最近一次 assistant turn 的 input_tokens + output_tokens 作為當前 context 使用量
            // （每次 turn 都會送入完整對話 context，所以 input_tokens 就是 context 大小）

            // 從 modelUsage 中取得 context window 大小
            let model_usage = json.get("modelUsage").and_then(|m| m.as_object());
            if let Some(mu) = &model_usage {
                eprintln!("📋 modelUsage raw: {:?}", mu);
            }
            let context_window_max = model_usage.and_then(|m| {
                // 取第一個模型的 contextWindow
                m.values().next().and_then(|v| v.get("contextWindow")).and_then(|c| c.as_u64())
            });

            // 用最近一次 assistant turn 的 token 數計算 context window 使用量
            // last_turn_input_tokens 已包含 cache tokens（在 assistant 事件處理時加總）
            let last_turn_tokens = self.last_turn_input_tokens + self.last_turn_output_tokens;
            let (total_tokens_in_conversation, context_window_used_percent) = if let Some(max) = context_window_max {
                if last_turn_tokens > 0 {
                    let percent = (last_turn_tokens as f64 / max as f64) * 100.0;
                    (Some(last_turn_tokens), Some(percent))
                } else {
                    (None, None)
                }
            } else {
                (if last_turn_tokens > 0 { Some(last_turn_tokens) } else { None }, None)
            };

            // Debug: 印出 context 資訊
            if last_turn_tokens > 0 {
                eprintln!("📊 Context usage (last turn): {} tokens (in={}, out={}) / {:?} max = {:?}%",
                    last_turn_tokens, self.last_turn_input_tokens, self.last_turn_output_tokens,
                    context_window_max, context_window_used_percent);
            }

            // 互動模式：permission_denials 已透過 control_request/control_response 即時處理
            // 僅 log，不產生 PermissionDenied 事件（避免前端重複彈出對話框）
            let denial_count = json.get("permission_denials")
                .or_else(|| json.get("permissionDenials"))
                .and_then(|d| d.as_array())
                .map(|d| d.len())
                .unwrap_or(0);
            if denial_count > 0 {
                eprintln!("ℹ️ Skipping {} permission_denials in result (already handled via control_response)", denial_count);
            }

            events.push(ClaudeEvent::Complete {
                result,
                cost_usd,
                total_tokens_in_conversation,
                context_window_max,
                context_window_used_percent,
                input_tokens_this_turn: if self.last_turn_input_tokens > 0 { Some(self.last_turn_input_tokens) } else { None },
            });
        }
        "progress" => {
            // Bash 長時間執行中的進度回報
            if let Some(data) = json.get("data") {
                let data_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let parent_tool_id = json.get("parentToolUseID").and_then(|t| t.as_str()).unwrap_or("");

                match data_type {
                    "bash_progress" => {
                        let elapsed = data.get("elapsedTimeSeconds").and_then(|v| v.as_u64()).unwrap_or(0);
                        let output = data.get("output").and_then(|v| v.as_str()).unwrap_or("");
                        if !parent_tool_id.is_empty() && (elapsed % 10 == 0 || !output.is_empty()) {
                            // 將進度回報轉為 ToolResult 事件更新（覆蓋同一工具的結果）
                            let progress_text = if output.is_empty() {
                                format!("⏳ 執行中... ({}s)", elapsed)
                            } else {
                                format!("⏳ 執行中... ({}s)\n{}", elapsed, output)
                            };
                            events.push(ClaudeEvent::ToolResult {
                                tool_id: parent_tool_id.to_string(),
                                result: progress_text,
                                is_error: false,
                                structured_patch: None,
                                image_base64: None,
                            });
                        }
                    }
                    "agent_progress" => {
                        // Sub-agent 進度：暫時只記錄 log
                        eprintln!("🤖 Sub-agent progress for tool {}", parent_tool_id);
                    }
                    _ => {
                        eprintln!("📡 Unknown progress type: {}", data_type);
                    }
                }
            }
        }
        other => {
            // 記錄未處理的事件類型（debug 用）
            eprintln!("⚠️ Unhandled event type: {}", other);
        }
    }

    events
}
} // impl ClaudeEventParser

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
            ClaudeEvent::Complete { result, cost_usd, total_tokens_in_conversation, context_window_max, context_window_used_percent, input_tokens_this_turn } => {
                assert_eq!(result, "Task completed successfully");
                assert!((cost_usd - 0.05).abs() < 0.001);
                assert!(total_tokens_in_conversation.is_none());
                assert!(context_window_max.is_none());
                assert!(context_window_used_percent.is_none());
                assert!(input_tokens_this_turn.is_none());
            }
            _ => panic!("Expected Complete event"),
        }
    }

    #[test]
    fn test_parse_complete_event_with_context_info() {
        // 模擬真實 CLI 流程：先收到 assistant 事件（帶 per-turn usage），再收到 result 事件
        let mut parser = ClaudeEventParser::new();

        // 1. 先解析 assistant 事件，讓 parser 追蹤 per-turn usage
        let assistant_json = json!({
            "type": "assistant",
            "message": {
                "content": [{ "type": "text", "text": "Hello" }],
                "usage": {
                    "input_tokens": 12000,
                    "output_tokens": 3000
                },
                "stop_reason": "end_turn"
            }
        });
        parser.parse(&assistant_json);

        // 2. 解析 result 事件（modelUsage 包含 contextWindow）
        let result_json = json!({
            "type": "result",
            "result": "Done",
            "total_cost_usd": 0.10,
            "usage": {
                "input_tokens": 50000,
                "output_tokens": 10000
            },
            "modelUsage": {
                "claude-sonnet-4-6-20260101": {
                    "inputTokens": 50000,
                    "outputTokens": 10000,
                    "contextWindow": 200000,
                    "maxOutputTokens": 64000
                }
            }
        });
        let events = parser.parse(&result_json);

        let complete_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, ClaudeEvent::Complete { .. }))
            .collect();

        assert_eq!(complete_events.len(), 1);

        match complete_events[0] {
            ClaudeEvent::Complete { result, cost_usd, total_tokens_in_conversation, context_window_max, context_window_used_percent, input_tokens_this_turn } => {
                assert_eq!(result, "Done");
                assert!((cost_usd - 0.10).abs() < 0.001);
                // 應該用 per-turn usage (12000 + 3000 = 15000)，而非累積值 (50000 + 10000 = 60000)
                assert_eq!(*total_tokens_in_conversation, Some(15000));
                assert_eq!(*context_window_max, Some(200000));
                // 15000 / 200000 = 7.5%
                assert!((context_window_used_percent.unwrap() - 7.5).abs() < 0.001);
                // input_tokens_this_turn 應為 12000
                assert_eq!(*input_tokens_this_turn, Some(12000));
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

    // === 互動模式訊息格式測試 ===

    #[test]
    fn test_interactive_user_message_format() {
        let msg = InteractiveUserMessage::new("Hello, Claude!");
        let json = serde_json::to_value(&msg).unwrap();

        assert_eq!(json["type"], "user");
        assert_eq!(json["session_id"], "");
        assert_eq!(json["parent_tool_use_id"], serde_json::Value::Null);
        assert_eq!(json["isSynthetic"], false);
        assert_eq!(json["message"]["role"], "user");
        assert_eq!(json["message"]["content"][0]["type"], "text");
        assert_eq!(json["message"]["content"][0]["text"], "Hello, Claude!");
    }

    #[test]
    fn test_interactive_user_message_content_is_array() {
        // VS Code extension 格式要求 content 是陣列，不是字串
        let msg = InteractiveUserMessage::new("test");
        let json = serde_json::to_value(&msg).unwrap();

        assert!(json["message"]["content"].is_array());
        assert_eq!(json["message"]["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_control_response_format_allow() {
        // 驗證 control_response JSON 結構（匹配 VS Code extension SDK 格式）
        // 允許時必須同時帶 behavior: "allow" + updatedInput（Zod union type 要求）
        let request_id = "req-abc-123";
        let tool_input = json!({"file_path": "/tmp/test.txt"});

        let control_response = json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": request_id,
                "response": {
                    "behavior": "allow",
                    "updatedInput": tool_input,
                    "toolUseID": "toolu_test123"
                }
            }
        });

        assert_eq!(control_response["type"], "control_response");
        assert_eq!(control_response["response"]["subtype"], "success");
        assert_eq!(control_response["response"]["request_id"], "req-abc-123");
        assert_eq!(control_response["response"]["response"]["behavior"], "allow");
        assert_eq!(control_response["response"]["response"]["updatedInput"]["file_path"], "/tmp/test.txt");
        assert_eq!(control_response["response"]["response"]["toolUseID"], "toolu_test123");
    }

    #[test]
    fn test_control_response_format_deny_with_message() {
        let control_response = json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": "req-deny-456",
                "response": {
                    "behavior": "deny",
                    "message": "使用者拒絕了這個操作",
                    "toolUseID": "toolu_deny789"
                }
            }
        });

        assert_eq!(control_response["response"]["response"]["behavior"], "deny");
        assert_eq!(
            control_response["response"]["response"]["message"],
            "使用者拒絕了這個操作"
        );
    }

    #[test]
    fn test_auto_allow_tools_contains_expected() {
        // 驗證 AUTO_ALLOW_TOOLS 包含常用的安全工具
        assert!(AUTO_ALLOW_TOOLS.contains(&"Read"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"Glob"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"Grep"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"TodoWrite"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"CronCreate"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"WebSearch"));
    }

    #[test]
    fn test_auto_allow_tools_excludes_dangerous() {
        // 驗證 AUTO_ALLOW_TOOLS 不包含需要審核的危險工具
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Bash"));
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Edit"));
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Write"));
    }

    #[test]
    fn test_parse_control_request_permission() {
        // 模擬 CLI 發送的 control_request
        let json = json!({
            "type": "control_request",
            "request": {
                "type": "permission",
                "request_id": "perm-req-001",
                "tool_name": "Bash",
                "input": {
                    "command": "ls -la"
                }
            }
        });

        // 驗證可以正確解析欄位
        let request = &json["request"];
        assert_eq!(request["type"], "permission");
        assert_eq!(request["request_id"], "perm-req-001");
        assert_eq!(request["tool_name"], "Bash");
        assert_eq!(request["input"]["command"], "ls -la");
    }

    #[test]
    fn test_initialize_message_format() {
        // 驗證 initialize 訊息的結構
        let init_msg = json!({
            "type": "initialize",
            "appendSystemPrompt": GUI_SYSTEM_PROMPT,
            "hooks": {},
            "sdkMcpServers": {},
            "permissionMode": "default"
        });

        assert_eq!(init_msg["type"], "initialize");
        assert!(init_msg["appendSystemPrompt"].as_str().unwrap().contains("tsunu_alive"));
        assert!(init_msg["hooks"].is_object());
        assert!(init_msg["sdkMcpServers"].is_object());
    }

    #[test]
    fn test_parse_api_error_event() {
        // 模擬 CLI 送出的 "Request too large" API 錯誤
        let json = json!({
            "type": "assistant",
            "isApiErrorMessage": true,
            "error": "invalid_request",
            "message": {
                "model": "<synthetic>",
                "role": "assistant",
                "stop_reason": "stop_sequence",
                "content": [
                    {
                        "type": "text",
                        "text": "Request too large (max 20MB). Try with a smaller file."
                    }
                ],
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "cache_creation_input_tokens": 0,
                    "cache_read_input_tokens": 0
                }
            }
        });

        let events = parse_claude_output(&json);
        assert_eq!(events.len(), 1);

        match &events[0] {
            ClaudeEvent::ApiError { error_code, message } => {
                assert_eq!(error_code, "invalid_request");
                assert!(message.contains("Request too large"));
            }
            _ => panic!("Expected ApiError event, got {:?}", events[0]),
        }
    }

    #[test]
    fn test_parse_api_error_not_triggered_for_normal_assistant() {
        // 一般 assistant 訊息不應被誤判為 API 錯誤
        let json = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "Hello, world!"
                    }
                ],
                "stop_reason": "end_turn",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "cache_creation_input_tokens": 0,
                    "cache_read_input_tokens": 0
                }
            }
        });

        let events = parse_claude_output(&json);
        assert!(!events.is_empty());

        // 不應該是 ApiError
        for event in &events {
            match event {
                ClaudeEvent::ApiError { .. } => panic!("Normal assistant should not be ApiError"),
                _ => {}
            }
        }
    }
}
