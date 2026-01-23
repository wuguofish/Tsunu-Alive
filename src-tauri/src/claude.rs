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
        // Context 相關資訊
        #[serde(skip_serializing_if = "Option::is_none")]
        total_tokens_in_conversation: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_window_max: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_window_used_percent: Option<f64>,
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

/// 取得 Claude CLI 執行檔路徑
/// 依序嘗試多個可能的安裝位置
fn get_claude_path() -> String {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

    if let Some(home) = std::env::var_os(home_var) {
        let home_path = std::path::Path::new(&home);

        if cfg!(windows) {
            // Windows: 依序嘗試
            let candidates = [
                // 1. 新版原生 exe（推薦）
                home_path.join(".local").join("bin").join("claude.exe"),
                // 2. 舊版 .claude/local/
                home_path.join(".claude").join("local").join("claude.cmd"),
            ];

            for path in candidates {
                if path.exists() {
                    return path.to_string_lossy().to_string();
                }
            }
        } else {
            // macOS/Linux
            let candidates = [
                // 1. ~/.local/bin/claude
                home_path.join(".local").join("bin").join("claude"),
                // 2. ~/.claude/local/claude
                home_path.join(".claude").join("local").join("claude"),
            ];

            for path in candidates {
                if path.exists() {
                    return path.to_string_lossy().to_string();
                }
            }
        }
    }

    // Fallback: 依賴 PATH
    if cfg!(windows) {
        "claude.exe".to_string()
    } else {
        "claude".to_string()
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

    let mut cmd = Command::new(get_claude_path());
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
    allowed_tools: Option<Vec<String>>,
    permission_mode: Option<String>,
    extended_thinking: Option<bool>,
) -> Result<(), String> {
    let cwd = working_dir.unwrap_or_else(|| ".".to_string());

    let mut cmd = Command::new(get_claude_path());
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

    // 如果有 allowedTools，加入參數
    if let Some(tools) = &allowed_tools {
        if !tools.is_empty() {
            let tools_arg = tools.join(",");
            eprintln!("🔧 Adding --allowedTools: {}", tools_arg);
            cmd.arg("--allowedTools").arg(&tools_arg);
        }
    }

    // 如果有 permissionMode，加入參數
    if let Some(mode) = &permission_mode {
        cmd.arg("--permission-mode").arg(mode);
    }

    // 如果啟用 extended thinking
    if extended_thinking.unwrap_or(false) {
        eprintln!("🧠 Extended thinking enabled");
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

            // 解析 context window 相關欄位（支援蛇底式和駝峰式）
            let total_tokens_in_conversation = json.get("total_tokens_in_conversation")
                .or_else(|| json.get("totalTokensInConversation"))
                .and_then(|v| v.as_u64());

            let context_window_max = json.get("context_window_max")
                .or_else(|| json.get("contextWindowMax"))
                .or_else(|| json.get("max_context_tokens"))
                .or_else(|| json.get("maxContextTokens"))
                .and_then(|v| v.as_u64());

            let context_window_used_percent = json.get("context_window_used_percent")
                .or_else(|| json.get("contextWindowUsedPercent"))
                .and_then(|v| v.as_f64());

            // Debug: 印出完整的 result JSON 來檢查結構
            eprintln!("🔍 Result JSON keys: {:?}", json.as_object().map(|o| o.keys().collect::<Vec<_>>()));
            // 額外印出完整 JSON 以便 debug
            eprintln!("🔍 Full result JSON: {}", serde_json::to_string_pretty(json).unwrap_or_default());
            // 印出解析到的 context 資訊
            if total_tokens_in_conversation.is_some() || context_window_max.is_some() || context_window_used_percent.is_some() {
                eprintln!("📊 Context info: tokens={:?}, max={:?}, percent={:?}",
                    total_tokens_in_conversation, context_window_max, context_window_used_percent);
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
            ClaudeEvent::Init { session_id, model } => {
                assert_eq!(session_id, "test-session-123");
                assert_eq!(model, "claude-sonnet-4-20250514");
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
