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
    },
    // 完成
    Complete {
        result: String,
        cost_usd: f64,
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

/// 啟動 Claude CLI 程序（使用 stream-json 雙向通訊）
pub async fn start_claude(
    app: AppHandle,
    process: Arc<Mutex<ClaudeProcess>>,
    working_dir: Option<String>,
    session_id: Option<String>,
) -> Result<(), String> {
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
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
) -> Result<(), String> {
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg(&prompt)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // 如果有 session_id，繼續該 session
    if let Some(sid) = &session_id {
        cmd.arg("--resume").arg(sid);
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
                        events.push(ClaudeEvent::Init {
                            session_id: session_id.to_string(),
                            model: model.to_string(),
                        });
                    }
                }
            } else if let (Some(session_id), model) = (
                json.get("session_id").and_then(|s| s.as_str()),
                json.get("model").and_then(|m| m.as_str()).unwrap_or("unknown"),
            ) {
                events.push(ClaudeEvent::Init {
                    session_id: session_id.to_string(),
                    model: model.to_string(),
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
            // 工具結果
            if let Some(tool_result) = json.get("tool_use_result") {
                let tool_id = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("tool_use_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
                    .to_string();

                // 檢查是否是錯誤
                let is_error = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("is_error"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);

                // 取得結果內容
                let result = if let Some(file) = tool_result.get("file") {
                    file.get("filePath")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string()
                } else if let Some(content) = json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("content"))
                    .and_then(|c| c.as_str())
                {
                    content.to_string()
                } else {
                    tool_result.get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown")
                        .to_string()
                };

                events.push(ClaudeEvent::ToolResult { tool_id, result, is_error });
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

            // 檢查是否有權限被拒絕
            if let Some(denials) = json.get("permission_denials").and_then(|d| d.as_array()) {
                for denial in denials {
                    if let (Some(tool_name), Some(tool_id)) = (
                        denial.get("tool_name").and_then(|n| n.as_str()),
                        denial.get("tool_use_id").and_then(|i| i.as_str()),
                    ) {
                        let input = denial.get("tool_input").cloned().unwrap_or(serde_json::Value::Null);
                        events.push(ClaudeEvent::PermissionDenied {
                            tool_name: tool_name.to_string(),
                            tool_id: tool_id.to_string(),
                            input,
                        });
                    }
                }
            }

            events.push(ClaudeEvent::Complete { result, cost_usd });
        }
        _ => {}
    }

    events
}
