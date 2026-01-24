// IDE Server 模組
// 提供 WebSocket server 讓 VS Code 等 IDE 連接並分享 context

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message};

// 預設 WebSocket port
pub const DEFAULT_WS_PORT: u16 = 19750;

// ============================================================================
// 資料結構
// ============================================================================

/// IDE 選取範圍
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// IDE 診斷資訊（錯誤/警告）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String, // "error", "warning", "info", "hint"
    pub message: String,
    pub range: SelectionRange,
}

/// IDE Context - 從 IDE 傳來的當前狀態
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdeContext {
    /// 當前檔案路徑
    pub file_path: Option<String>,
    /// 選取的文字內容
    pub selected_text: Option<String>,
    /// 選取範圍
    pub selection: Option<SelectionRange>,
    /// 整個檔案內容（可選）
    pub file_content: Option<String>,
    /// 當前語言 ID（如 "rust", "typescript"）
    pub language_id: Option<String>,
    /// 診斷資訊
    pub diagnostics: Vec<Diagnostic>,
    /// 最後更新時間
    pub last_updated: Option<String>,
    /// 來源客戶端 ID（用於判斷是哪個 IDE 發送的）
    pub client_id: Option<String>,
}

/// 連接的客戶端資訊
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClient {
    pub id: String,
    pub name: String,  // IDE 名稱，如 "VS Code", "PyCharm"
    pub connected_at: String,
    /// 工作區路徑（用於過濾同專案的 IDE）
    pub workspace_path: Option<String>,
}

/// IDE Server 狀態
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdeServerStatus {
    pub running: bool,
    pub port: u16,
    pub connected_clients: Vec<ConnectedClient>,
    pub current_context: Option<IdeContext>,
}

// ============================================================================
// JSON-RPC 訊息格式
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

// ============================================================================
// IDE Server
// ============================================================================

pub struct IdeServer {
    /// 當前 context
    context: Arc<RwLock<IdeContext>>,
    /// 連接的客戶端
    clients: Arc<RwLock<HashMap<String, ConnectedClient>>>,
    /// 廣播 channel，用於通知前端狀態變化
    event_tx: broadcast::Sender<IdeServerEvent>,
    /// Server 是否運行中
    running: Arc<RwLock<bool>>,
    /// Port
    port: u16,
}

#[derive(Debug, Clone)]
pub enum IdeServerEvent {
    ClientConnected(ConnectedClient),
    ClientDisconnected(String),
    ContextUpdated(IdeContext),
}

impl IdeServer {
    pub fn new(port: u16) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            context: Arc::new(RwLock::new(IdeContext::default())),
            clients: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            running: Arc::new(RwLock::new(false)),
            port,
        }
    }

    /// 訂閱事件
    pub fn subscribe(&self) -> broadcast::Receiver<IdeServerEvent> {
        self.event_tx.subscribe()
    }

    /// 取得當前狀態
    pub async fn get_status(&self) -> IdeServerStatus {
        let running = *self.running.read().await;
        let clients = self.clients.read().await;
        let context = self.context.read().await;

        IdeServerStatus {
            running,
            port: self.port,
            connected_clients: clients.values().cloned().collect(),
            current_context: if context.file_path.is_some() {
                Some(context.clone())
            } else {
                None
            },
        }
    }

    /// 取得當前 context
    pub async fn get_context(&self) -> Option<IdeContext> {
        let context = self.context.read().await;
        if context.file_path.is_some() {
            Some(context.clone())
        } else {
            None
        }
    }

    /// 啟動 WebSocket server
    pub async fn start(&self) -> Result<(), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("無法綁定 port {}: {}", self.port, e))?;

        *self.running.write().await = true;
        println!("🌐 IDE WebSocket Server 啟動於 ws://{}", addr);

        let context = self.context.clone();
        let clients = self.clients.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            while *running.read().await {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let context = context.clone();
                        let clients = clients.clone();
                        let event_tx = event_tx.clone();
                        tokio::spawn(handle_connection(stream, addr, context, clients, event_tx));
                    }
                    Err(e) => {
                        eprintln!("❌ 接受連接失敗: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}

/// 處理單一 WebSocket 連接
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    context: Arc<RwLock<IdeContext>>,
    clients: Arc<RwLock<HashMap<String, ConnectedClient>>>,
    event_tx: broadcast::Sender<IdeServerEvent>,
) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("❌ WebSocket handshake 失敗 ({}): {}", addr, e);
            return;
        }
    };

    let client_id = format!("{}", addr);
    println!("✅ IDE 客戶端連接: {}", client_id);

    // 暫時的客戶端資訊（等待 hello 訊息更新）
    let client = ConnectedClient {
        id: client_id.clone(),
        name: "Unknown IDE".to_string(),
        connected_at: chrono::Utc::now().to_rfc3339(),
        workspace_path: None,
    };
    clients.write().await.insert(client_id.clone(), client.clone());
    let _ = event_tx.send(IdeServerEvent::ClientConnected(client));

    let (mut write, mut read) = ws_stream.split();

    // 發送歡迎訊息
    let welcome = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "server/hello",
        "params": {
            "name": "Tsunu Alive IDE Server",
            "version": "0.1.0"
        }
    });
    if let Err(e) = write.send(Message::Text(welcome.to_string().into())).await {
        eprintln!("❌ 發送歡迎訊息失敗: {}", e);
    }

    // 處理訊息
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Some(response) = handle_message(&text, &context, &clients, &event_tx, &client_id).await {
                    if let Err(e) = write.send(Message::Text(response.into())).await {
                        eprintln!("❌ 發送回應失敗: {}", e);
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                println!("👋 客戶端斷開: {}", client_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                if let Err(e) = write.send(Message::Pong(data)).await {
                    eprintln!("❌ 發送 pong 失敗: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("❌ 接收訊息錯誤 ({}): {}", client_id, e);
                break;
            }
            _ => {}
        }
    }

    // 清理
    clients.write().await.remove(&client_id);
    let _ = event_tx.send(IdeServerEvent::ClientDisconnected(client_id.clone()));
    println!("🔌 客戶端已斷開: {}", client_id);
}

/// 處理 JSON-RPC 訊息
async fn handle_message(
    text: &str,
    context: &Arc<RwLock<IdeContext>>,
    clients: &Arc<RwLock<HashMap<String, ConnectedClient>>>,
    event_tx: &broadcast::Sender<IdeServerEvent>,
    client_id: &str,
) -> Option<String> {
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(req) => req,
        Err(e) => {
            let error_response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
            return Some(serde_json::to_string(&error_response).unwrap());
        }
    };

    println!("📨 收到 {} 訊息: {}", client_id, request.method);

    match request.method.as_str() {
        // 客戶端 hello - 更新客戶端資訊
        "client/hello" => {
            if let Some(params) = request.params {
                let mut clients_guard = clients.write().await;
                if let Some(client) = clients_guard.get_mut(client_id) {
                    if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                        client.name = name.to_string();
                    }
                    if let Some(workspace) = params.get("workspacePath").and_then(|v| v.as_str()) {
                        client.workspace_path = Some(workspace.to_string());
                        println!("🏷️ 客戶端識別為: {} (workspace: {})", client.name, workspace);
                    } else {
                        println!("🏷️ 客戶端識別為: {}", client.name);
                    }
                }
            }
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({ "status": "ok" })),
                error: None,
            };
            Some(serde_json::to_string(&response).unwrap())
        }

        // 更新 context
        "context/update" => {
            if let Some(params) = request.params {
                let mut ctx = context.write().await;

                if let Some(file_path) = params.get("filePath").and_then(|v| v.as_str()) {
                    ctx.file_path = Some(file_path.to_string());
                }
                if let Some(selected_text) = params.get("selectedText").and_then(|v| v.as_str()) {
                    ctx.selected_text = Some(selected_text.to_string());
                }
                if let Some(selection) = params.get("selection") {
                    if let Ok(sel) = serde_json::from_value::<SelectionRange>(selection.clone()) {
                        ctx.selection = Some(sel);
                    }
                }
                if let Some(file_content) = params.get("fileContent").and_then(|v| v.as_str()) {
                    ctx.file_content = Some(file_content.to_string());
                }
                if let Some(language_id) = params.get("languageId").and_then(|v| v.as_str()) {
                    ctx.language_id = Some(language_id.to_string());
                }
                if let Some(diagnostics) = params.get("diagnostics") {
                    if let Ok(diags) = serde_json::from_value::<Vec<Diagnostic>>(diagnostics.clone()) {
                        ctx.diagnostics = diags;
                    }
                }

                ctx.last_updated = Some(chrono::Utc::now().to_rfc3339());
                // 記錄來源客戶端 ID
                ctx.client_id = Some(client_id.to_string());

                let updated_ctx = ctx.clone();
                drop(ctx);

                let _ = event_tx.send(IdeServerEvent::ContextUpdated(updated_ctx));
                println!("📝 Context 已更新: {:?}", context.read().await.file_path);
            }

            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({ "status": "ok" })),
                error: None,
            };
            Some(serde_json::to_string(&response).unwrap())
        }

        // 清除 context
        "context/clear" => {
            let mut ctx = context.write().await;
            *ctx = IdeContext::default();
            println!("🧹 Context 已清除");

            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({ "status": "ok" })),
                error: None,
            };
            Some(serde_json::to_string(&response).unwrap())
        }

        // 選取變更（輕量版本，只更新選取範圍）
        "selection/changed" => {
            if let Some(params) = request.params {
                let mut ctx = context.write().await;

                if let Some(selected_text) = params.get("selectedText").and_then(|v| v.as_str()) {
                    ctx.selected_text = Some(selected_text.to_string());
                }
                if let Some(selection) = params.get("selection") {
                    if let Ok(sel) = serde_json::from_value::<SelectionRange>(selection.clone()) {
                        ctx.selection = Some(sel);
                    }
                }

                ctx.last_updated = Some(chrono::Utc::now().to_rfc3339());
                // 記錄來源客戶端 ID
                ctx.client_id = Some(client_id.to_string());
            }

            // 不發送事件，避免過於頻繁
            None // 不回應，減少網路開銷
        }

        // 未知方法
        _ => {
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
            };
            Some(serde_json::to_string(&response).unwrap())
        }
    }
}

// ============================================================================
// 全域 IDE Server 實例
// ============================================================================

use std::sync::OnceLock;

static IDE_SERVER: OnceLock<IdeServer> = OnceLock::new();

/// 取得或初始化全域 IDE Server
pub fn get_ide_server() -> &'static IdeServer {
    IDE_SERVER.get_or_init(|| IdeServer::new(DEFAULT_WS_PORT))
}

/// 啟動 IDE Server（Tauri command 會呼叫）
pub async fn start_ide_server() -> Result<(), String> {
    get_ide_server().start().await
}

/// 取得 IDE Server 狀態
pub async fn get_ide_status() -> IdeServerStatus {
    get_ide_server().get_status().await
}

/// 取得當前 IDE context
pub async fn get_ide_context() -> Option<IdeContext> {
    get_ide_server().get_context().await
}

// ============================================================================
// 測試
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_parse() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"context/update","params":{"filePath":"/test.rs"}}"#;
        let request: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.method, "context/update");
    }

    #[test]
    fn test_selection_range_default() {
        let range = SelectionRange::default();
        assert_eq!(range.start_line, 0);
        assert_eq!(range.end_line, 0);
    }

    #[test]
    fn test_ide_context_serialization() {
        let ctx = IdeContext {
            file_path: Some("/test/file.rs".to_string()),
            selected_text: Some("let x = 1;".to_string()),
            selection: Some(SelectionRange {
                start_line: 10,
                start_character: 0,
                end_line: 10,
                end_character: 10,
            }),
            file_content: None,
            language_id: Some("rust".to_string()),
            diagnostics: vec![],
            last_updated: None,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("file.rs"));
        assert!(json.contains("rust"));
    }

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            result: Some(serde_json::json!({ "status": "ok" })),
            error: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: None,
            error: Some(JsonRpcError {
                code: -32700,
                message: "Parse error".to_string(),
            }),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32700"));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_ide_server_status_serialization() {
        let status = IdeServerStatus {
            running: true,
            port: 19750,
            connected_clients: vec![
                ConnectedClient {
                    id: "127.0.0.1:12345".to_string(),
                    name: "VS Code".to_string(),
                    connected_at: "2026-01-23T12:00:00Z".to_string(),
                    workspace_path: Some("D:\\project_A".to_string()),
                },
            ],
            current_context: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("19750"));
        assert!(json.contains("VS Code"));
        assert!(json.contains("project_A"));
    }

    #[test]
    fn test_diagnostic_serialization() {
        let diag = Diagnostic {
            severity: "error".to_string(),
            message: "undefined variable".to_string(),
            range: SelectionRange {
                start_line: 5,
                start_character: 10,
                end_line: 5,
                end_character: 15,
            },
        };
        let json = serde_json::to_string(&diag).unwrap();
        assert!(json.contains("\"severity\":\"error\""));
        assert!(json.contains("undefined variable"));
    }

    #[test]
    fn test_ide_context_with_diagnostics() {
        let ctx = IdeContext {
            file_path: Some("/src/main.rs".to_string()),
            selected_text: None,
            selection: None,
            file_content: None,
            language_id: Some("rust".to_string()),
            diagnostics: vec![
                Diagnostic {
                    severity: "warning".to_string(),
                    message: "unused variable".to_string(),
                    range: SelectionRange::default(),
                },
            ],
            last_updated: Some("2026-01-23T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("warning"));
        assert!(json.contains("unused variable"));
    }

    #[test]
    fn test_json_rpc_request_without_params() {
        let json = r#"{"jsonrpc":"2.0","id":2,"method":"context/clear"}"#;
        let request: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.method, "context/clear");
        assert!(request.params.is_none());
    }

    #[test]
    fn test_connected_client_serialization() {
        let client = ConnectedClient {
            id: "client-123".to_string(),
            name: "PyCharm".to_string(),
            connected_at: "2026-01-23T10:30:00Z".to_string(),
            workspace_path: Some("/home/user/my_project".to_string()),
        };
        let json = serde_json::to_string(&client).unwrap();
        assert!(json.contains("PyCharm"));
        assert!(json.contains("client-123"));
        assert!(json.contains("my_project"));
    }
}
