//! Permission HTTP Server
//!
//! 實作 Claude CLI PermissionRequest Hook 的 HTTP Server。
//! Hook 腳本透過 HTTP 請求與此 Server 通訊，等待用戶在 UI 中做出決策。

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tauri::{AppHandle, Emitter};

/// Hook 腳本發送的權限請求
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_use_id: String,
    pub session_id: Option<String>,
}

/// 回傳給 Hook 腳本的決策
#[derive(Debug, Clone, Serialize)]
pub struct PermissionResponse {
    pub behavior: String, // "allow" | "deny"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
}

/// 發送到前端的權限請求事件
#[derive(Debug, Clone, Serialize)]
pub struct PermissionRequestEvent {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_use_id: String,
    pub session_id: Option<String>,
}

/// 發送到前端的 ExitPlanMode 專用事件
#[derive(Debug, Clone, Serialize)]
pub struct PlanApprovalEvent {
    pub tool_use_id: String,
    pub plan_content: Option<String>,
    pub plan_file_path: Option<String>,
}

/// 前端發送的決策回應
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionRespondRequest {
    pub tool_use_id: String,
    pub behavior: String, // "allow" | "deny"
    pub message: Option<String>,
}

/// 全域權限狀態
pub struct PermissionState {
    /// 待處理的權限請求 (tool_use_id → oneshot::Sender)
    pub pending: HashMap<String, oneshot::Sender<PermissionResponse>>,
    /// Session 白名單 (session_id → 允許的工具名稱集合)
    session_allowed_tools: HashMap<String, HashSet<String>>,
    /// Tauri AppHandle，用於發送事件到前端
    app_handle: Option<AppHandle>,
    /// 目前前端的編輯模式（default / acceptEdits / bypassPermissions / plan）
    /// 用於在 server 端根據模式自動允許對應的工具
    edit_mode: String,
}

impl PermissionState {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            session_allowed_tools: HashMap::new(),
            app_handle: None,
            edit_mode: "default".to_string(),
        }
    }

    pub fn set_app_handle(&mut self, app: AppHandle) {
        self.app_handle = Some(app);
    }

    /// 檢查工具是否在 session 白名單中
    pub fn is_tool_allowed(&self, session_id: &Option<String>, tool_name: &str) -> bool {
        if let Some(sid) = session_id {
            if let Some(tools) = self.session_allowed_tools.get(sid) {
                return tools.contains(tool_name);
            }
        }
        false
    }

    /// 將工具加入 session 白名單
    pub fn add_to_session_whitelist(&mut self, session_id: &str, tool_name: &str) {
        self.session_allowed_tools
            .entry(session_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(tool_name.to_string());
    }

    /// 清除指定 session 的白名單
    pub fn clear_session_whitelist(&mut self, session_id: &str) {
        self.session_allowed_tools.remove(session_id);
    }

    /// 設定編輯模式
    pub fn set_edit_mode(&mut self, mode: &str) {
        self.edit_mode = mode.to_string();
    }

    /// 根據目前的 edit_mode 判斷工具是否可以自動允許
    ///
    /// 規則：
    /// - default：只有 AUTO_ALLOW_TOOLS 會自動通過（由外層處理）
    /// - acceptEdits：額外允許 Edit, Write, NotebookEdit
    /// - bypassPermissions：允許所有工具，但 ExitPlanMode 例外（必須讓使用者審核計畫）
    /// - plan：由 CLI 的 --permission-mode plan 處理，這裡不會收到
    pub fn should_auto_allow_by_mode(&self, tool_name: &str) -> bool {
        match self.edit_mode.as_str() {
            "acceptEdits" => {
                matches!(tool_name, "Edit" | "Write" | "NotebookEdit")
            }
            "bypassPermissions" => {
                // ExitPlanMode 無論如何都必須讓使用者看到計畫並確認
                tool_name != "ExitPlanMode"
            }
            _ => false, // default 和 plan 模式不額外放行
        }
    }
}

/// 共享狀態類型
pub type SharedPermissionState = Arc<Mutex<PermissionState>>;

/// 不需要用戶確認的工具列表
///
/// ⚠️ 重要：此列表必須與前端保持同步！
/// 單一真相來源：src/constants/autoAllowTools.ts
///
/// 這些工具會自動允許執行，不會彈出權限確認對話框：
/// - 用戶互動工具：本身就是詢問用戶的工具
/// - 任務管理工具：內部追蹤用途
/// - 只讀工具：不修改檔案系統
/// - 子代理任務管理：Task 工具的內部運作
/// - 網路只讀工具：WebSearch, WebFetch
/// - Plan 模式相關：進入 Plan 模式不需要確認
///
/// 參考官方文件：https://code.claude.com/docs/en/settings
const AUTO_ALLOW_TOOLS: &[&str] = &[
    // 用戶互動工具（本身就是詢問用戶）
    "AskUserQuestion",

    // 任務管理工具（內部追蹤用途）
    "TodoRead",
    "TodoWrite",

    // 只讀工具（不修改檔案系統）
    "Read",
    "Glob",
    "Grep",

    // 子代理任務管理
    "Task",
    "TaskOutput",

    // 網路只讀工具
    "WebSearch",
    "WebFetch",

    // 排程管理工具（建立、刪除、列出定時任務）
    "CronCreate",
    "CronDelete",
    "CronList",

    // Plan 模式相關（進入 Plan 模式不需要確認）
    "EnterPlanMode",
];

/// 處理 Hook 腳本的權限請求
/// POST /permission/request
/// 這個 endpoint 會阻塞，直到用戶在 UI 中做出決策
async fn handle_permission_request(
    State(state): State<SharedPermissionState>,
    Json(req): Json<PermissionRequest>,
) -> Result<Json<PermissionResponse>, StatusCode> {
    // 診斷日誌：收到權限請求
    eprintln!("📥 Permission request received: tool={}, id={}", req.tool_name, req.tool_use_id);

    // 檢查是否是不需要確認的工具
    if AUTO_ALLOW_TOOLS.contains(&req.tool_name.as_str()) {
        eprintln!("🔓 Auto-allowing tool: {}", req.tool_name);
        return Ok(Json(PermissionResponse {
            behavior: "allow".to_string(),
            message: None,
            updated_input: None,
        }));
    }

    let mut state_guard = state.lock().await;

    // 檢查是否在 session 白名單中
    if state_guard.is_tool_allowed(&req.session_id, &req.tool_name) {
        return Ok(Json(PermissionResponse {
            behavior: "allow".to_string(),
            message: None,
            updated_input: None,
        }));
    }

    // 根據編輯模式自動允許對應的工具
    if state_guard.should_auto_allow_by_mode(&req.tool_name) {
        eprintln!("🔓 Auto-allowing by edit_mode '{}': {}", state_guard.edit_mode, req.tool_name);
        return Ok(Json(PermissionResponse {
            behavior: "allow".to_string(),
            message: None,
            updated_input: None,
        }));
    }

    // 建立 oneshot channel 等待用戶決策
    let (tx, rx) = oneshot::channel();
    state_guard.pending.insert(req.tool_use_id.clone(), tx);

    // 發送事件到前端
    if let Some(app) = &state_guard.app_handle {
        // 根據工具類型發送不同的事件
        if req.tool_name == "ExitPlanMode" {
            // ExitPlanMode 專用事件
            let plan_content = req.tool_input.get("plan")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let plan_file_path = req.tool_input.get("_planFilePath")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let event = PlanApprovalEvent {
                tool_use_id: req.tool_use_id.clone(),
                plan_content,
                plan_file_path,
            };
            eprintln!("📤 Emitting plan-approval-request event: id={}", req.tool_use_id);
            let _ = app.emit("plan-approval-request", &event);
        } else {
            // 一般權限請求事件
            let event = PermissionRequestEvent {
                tool_name: req.tool_name.clone(),
                tool_input: req.tool_input.clone(),
                tool_use_id: req.tool_use_id.clone(),
                session_id: req.session_id.clone(),
            };
            eprintln!("📤 Emitting permission-request event: tool={}, id={}", req.tool_name, req.tool_use_id);
            let _ = app.emit("permission-request", &event);
        }
    } else {
        eprintln!("⚠️ No app_handle available, cannot emit event!");
    }

    // 釋放鎖，等待用戶決策
    drop(state_guard);

    // 等待用戶決策（最多 55 秒，比 Hook 的 60 秒 timeout 短一點）
    match tokio::time::timeout(std::time::Duration::from_secs(55), rx).await {
        Ok(Ok(response)) => Ok(Json(response)),
        Ok(Err(_)) => {
            // Channel 被關閉（可能是 app 關閉）
            Ok(Json(PermissionResponse {
                behavior: "deny".to_string(),
                message: Some("Permission request was cancelled".to_string()),
                updated_input: None,
            }))
        }
        Err(_) => {
            // Timeout
            // 清理 pending 請求
            let mut state_guard = state.lock().await;
            state_guard.pending.remove(&req.tool_use_id);

            Ok(Json(PermissionResponse {
                behavior: "deny".to_string(),
                message: Some("Permission request timed out".to_string()),
                updated_input: None,
            }))
        }
    }
}

/// 處理前端發送的決策回應
/// POST /permission/respond
async fn handle_permission_respond(
    State(state): State<SharedPermissionState>,
    Json(req): Json<PermissionRespondRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut state_guard = state.lock().await;

    if let Some(tx) = state_guard.pending.remove(&req.tool_use_id) {
        let response = PermissionResponse {
            behavior: req.behavior,
            message: req.message,
            updated_input: None,
        };

        // 發送決策到等待中的請求
        let _ = tx.send(response);

        Ok(Json(serde_json::json!({ "success": true })))
    } else {
        // 找不到對應的待處理請求
        Ok(Json(serde_json::json!({
            "success": false,
            "error": "No pending permission request found"
        })))
    }
}

/// 將工具加入 session 白名單
/// POST /permission/whitelist/add
#[derive(Debug, Deserialize)]
pub struct WhitelistAddRequest {
    pub session_id: String,
    pub tool_name: String,
}

async fn handle_whitelist_add(
    State(state): State<SharedPermissionState>,
    Json(req): Json<WhitelistAddRequest>,
) -> Json<serde_json::Value> {
    let mut state_guard = state.lock().await;
    state_guard.add_to_session_whitelist(&req.session_id, &req.tool_name);
    Json(serde_json::json!({ "success": true }))
}

/// 設定編輯模式
/// POST /permission/edit-mode
#[derive(Debug, Deserialize)]
pub struct EditModeRequest {
    pub mode: String,
}

async fn handle_set_edit_mode(
    State(state): State<SharedPermissionState>,
    Json(req): Json<EditModeRequest>,
) -> Json<serde_json::Value> {
    let mut state_guard = state.lock().await;
    eprintln!("🔧 Edit mode changed to: {}", req.mode);
    state_guard.set_edit_mode(&req.mode);
    Json(serde_json::json!({ "success": true }))
}

/// 健康檢查
/// GET /health
async fn health_check() -> &'static str {
    "OK"
}

/// 建立 Permission HTTP Server 的 Router
pub fn create_router(state: SharedPermissionState) -> Router {
    Router::new()
        .route("/permission/request", post(handle_permission_request))
        .route("/permission/respond", post(handle_permission_respond))
        .route("/permission/whitelist/add", post(handle_whitelist_add))
        .route("/permission/edit-mode", post(handle_set_edit_mode))
        .route("/health", axum::routing::get(health_check))
        .with_state(state)
}

/// 啟動 Permission HTTP Server
pub async fn start_server(state: SharedPermissionState, port: u16) -> Result<(), String> {
    let app = create_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;

    println!("Permission HTTP Server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

/// 建立共享狀態
pub fn create_shared_state() -> SharedPermissionState {
    Arc::new(Mutex::new(PermissionState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_permission_response_serialization() {
        let response = PermissionResponse {
            behavior: "allow".to_string(),
            message: None,
            updated_input: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"behavior\":\"allow\""));
        // message 和 updated_input 為 None 時不應該出現在 JSON 中
        assert!(!json.contains("message"));
        assert!(!json.contains("updated_input"));
    }

    #[tokio::test]
    async fn test_session_whitelist() {
        let state = create_shared_state();
        let mut guard = state.lock().await;

        // 初始狀態：工具不在白名單中
        assert!(!guard.is_tool_allowed(&Some("session1".to_string()), "Bash"));

        // 加入白名單
        guard.add_to_session_whitelist("session1", "Bash");

        // 現在應該在白名單中
        assert!(guard.is_tool_allowed(&Some("session1".to_string()), "Bash"));

        // 其他工具不在白名單中
        assert!(!guard.is_tool_allowed(&Some("session1".to_string()), "Edit"));

        // 其他 session 不受影響
        assert!(!guard.is_tool_allowed(&Some("session2".to_string()), "Bash"));

        // 清除白名單
        guard.clear_session_whitelist("session1");
        assert!(!guard.is_tool_allowed(&Some("session1".to_string()), "Bash"));
    }

    // AUTO_ALLOW_TOOLS 相關測試
    #[test]
    fn test_auto_allow_tools_contains_user_interaction() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"AskUserQuestion"));
    }

    #[test]
    fn test_auto_allow_tools_contains_task_management() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"TodoRead"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"TodoWrite"));
    }

    #[test]
    fn test_auto_allow_tools_contains_read_only() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"Read"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"Glob"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"Grep"));
    }

    #[test]
    fn test_auto_allow_tools_contains_subagent() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"Task"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"TaskOutput"));
    }

    #[test]
    fn test_auto_allow_tools_contains_web_read_only() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"WebSearch"));
        assert!(AUTO_ALLOW_TOOLS.contains(&"WebFetch"));
    }

    #[test]
    fn test_auto_allow_tools_contains_plan_mode() {
        assert!(AUTO_ALLOW_TOOLS.contains(&"EnterPlanMode"));
    }

    #[test]
    fn test_auto_allow_tools_does_not_contain_write_tools() {
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Edit"));
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Write"));
        assert!(!AUTO_ALLOW_TOOLS.contains(&"Bash"));
        assert!(!AUTO_ALLOW_TOOLS.contains(&"NotebookEdit"));
    }

    #[test]
    fn test_auto_allow_tools_does_not_contain_exit_plan_mode() {
        // ExitPlanMode 需要用戶確認計畫，不應該自動允許
        assert!(!AUTO_ALLOW_TOOLS.contains(&"ExitPlanMode"));
    }

    #[test]
    fn test_auto_allow_tools_count_matches_frontend() {
        // 這個測試確保前後端的 AUTO_ALLOW_TOOLS 數量一致
        // 如果這個測試失敗，需要檢查 src/constants/autoAllowTools.ts
        // 目前預期：14 個工具
        assert_eq!(AUTO_ALLOW_TOOLS.len(), 14);
    }

    #[test]
    fn test_auto_allow_tools_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for tool in AUTO_ALLOW_TOOLS {
            assert!(seen.insert(*tool), "Duplicate tool found: {}", tool);
        }
    }

    // edit_mode 自動允許邏輯測試
    #[test]
    fn test_default_mode_does_not_auto_allow() {
        let state = PermissionState::new();
        assert!(!state.should_auto_allow_by_mode("Edit"));
        assert!(!state.should_auto_allow_by_mode("Write"));
        assert!(!state.should_auto_allow_by_mode("Bash"));
    }

    #[test]
    fn test_accept_edits_mode_allows_edit_tools() {
        let mut state = PermissionState::new();
        state.set_edit_mode("acceptEdits");
        assert!(state.should_auto_allow_by_mode("Edit"));
        assert!(state.should_auto_allow_by_mode("Write"));
        assert!(state.should_auto_allow_by_mode("NotebookEdit"));
        // Bash 不在 acceptEdits 的範圍
        assert!(!state.should_auto_allow_by_mode("Bash"));
        assert!(!state.should_auto_allow_by_mode("ExitPlanMode"));
    }

    #[test]
    fn test_bypass_mode_allows_all_except_exit_plan() {
        let mut state = PermissionState::new();
        state.set_edit_mode("bypassPermissions");
        assert!(state.should_auto_allow_by_mode("Edit"));
        assert!(state.should_auto_allow_by_mode("Write"));
        assert!(state.should_auto_allow_by_mode("Bash"));
        assert!(state.should_auto_allow_by_mode("NotebookEdit"));
        // ExitPlanMode 無論什麼模式都必須讓使用者確認
        assert!(!state.should_auto_allow_by_mode("ExitPlanMode"));
    }
}
