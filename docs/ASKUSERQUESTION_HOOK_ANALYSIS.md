# AskUserQuestion Hook 機制分析與實作記錄

> 文件建立日期：2026-01-24
> 最後更新：2026-01-24

## 問題背景

在實作 Tsunu Alive 的 PermissionRequest Hook 機制時，發現 `AskUserQuestion` 工具無法透過 Hook 機制正常運作。

### 問題現象

1. Claude 發送 `AskUserQuestion` ToolUse
2. Hook 機制回應 `allow`
3. Claude CLI 執行 AskUserQuestion，但在 JSON 模式下無法取得用戶輸入
4. Claude CLI 返回 `is_error: true, "Answer questions?"`
5. 用戶無法正確回答問題

### 根本原因

**PermissionRequest Hook 只處理「是否允許執行工具」，不處理「工具需要的用戶輸入」。**

這兩者是不同的概念：
- **權限確認**：「你允許這個工具執行嗎？」→ 是/否
- **用戶輸入**：「你的答案是什麼？」→ 具體內容

---

## 官方現狀（截至 2026-01）

### 目前沒有 AskUserQuestion Hook 機制

根據調查，官方 Claude Code 目前**沒有提供** AskUserQuestion 的 Hook 機制。

### 相關 GitHub Issues

1. **[Issue #10168](https://github.com/anthropics/claude-code/issues/10168)** (OPEN)
   - 請求 `UserInputRequired` Hook
   - 當 Claude 暫停等待用戶輸入時觸發

2. **[Issue #12605](https://github.com/anthropics/claude-code/issues/12605)** (Closed as duplicate of #10168)
   - 詳細的 AskUserQuestion Hook 提案
   - 包含完整的設計方案

### 社群建議的解決方案

Issue #12605 提出了完整的設計：

```json
// PreAskUserQuestion Hook 事件
{
  "hook_event_name": "PreAskUserQuestion",
  "session_id": "abc-123",
  "questions": [
    {
      "question": "Which approach should we use?",
      "header": "Implementation",
      "multiSelect": false,
      "options": [
        {"label": "Option A", "description": "Use existing pattern"},
        {"label": "Option B", "description": "Create new pattern"}
      ]
    }
  ]
}
```

**回應機制（三種選項）：**

**Option A: Hook 腳本返回值**
```bash
#!/bin/bash
echo '{"answers": {"0": "Option A"}}'
exit 0  # exit 0 = 答案已提供，跳過 CLI 提示
exit 1  # exit 1 = 沒有答案，顯示 CLI 提示
```

**Option B: 答案檔案**
```bash
ANSWER_FILE="/tmp/claude-answer-${SESSION_ID}.json"
echo '{"answers": {"0": "Option A"}}' > "$ANSWER_FILE"
echo "$ANSWER_FILE"
```

**Option C: 輪詢端點**
```json
{
  "hooks": {
    "answer_poll_url": "http://localhost:3001/api/claude/poll-answer?session={session_id}",
    "answer_poll_timeout": 30000
  }
}
```

---

## Tsunu Alive 目前的實作（Workaround）

由於官方沒有提供 AskUserQuestion Hook 機制，我們採用以下方案：

### 1. Hook 腳本跳過 AskUserQuestion

**檔案**：`resources/hooks/tsunu-permission.ps1`

```powershell
$hookInput = $inputJson | ConvertFrom-Json

# AskUserQuestion 需要特殊處理：
# 這個工具需要用戶輸入答案，不只是「允許/拒絕」
# Hook 機制無法處理用戶輸入，所以跳過讓 fallback 模式處理
if ($hookInput.tool_name -eq "AskUserQuestion") {
    exit 1
}
```

### 2. 透過 ToolUse 事件顯示對話框

**檔案**：`src/App.vue`

```typescript
// 特殊處理 AskUserQuestion 工具
if (event.event_type === 'ToolUse' && event.tool_name === 'AskUserQuestion') {
  const input = event.input as { questions?: Question[] } | undefined;
  if (input?.questions && Array.isArray(input.questions)) {
    pendingQuestion.value = {
      toolId: event.tool_id || '',
      questions: input.questions,
    };
    avatarState.value = 'waiting';
    busyStatus.value = '等待回答...';
  }
}
```

### 3. Fallback 模式自動允許

**檔案**：`src/constants/autoAllowTools.ts`

AskUserQuestion 保留在 `AUTO_ALLOW_TOOLS` 中，這樣在 fallback 模式下不會顯示「權限確認」對話框。

### 4. 對話框互斥顯示

**檔案**：`src/App.vue`

```vue
<!-- ExitPlanMode 專用對話框 -->
<PlanApprovalDialog
  v-if="pendingPermission && pendingPermission.toolName === 'ExitPlanMode'"
  ...
/>

<!-- 一般權限確認對話框 -->
<PermissionDialog
  v-else-if="pendingPermission"
  ...
/>

<!-- AskUserQuestion 對話框 -->
<!-- 使用 v-else-if 確保與 Permission 對話框互斥顯示 -->
<AskUserQuestionDialog
  v-else-if="pendingQuestion"
  ...
/>
```

### 5. 錯誤時清除狀態

**檔案**：`src/App.vue`

```typescript
// 特殊處理 AskUserQuestion 的 ToolResult (is_error)
if (event.event_type === 'ToolResult' && event.is_error) {
  const tool = currentToolUses.value.find(t => t.id === event.tool_id);
  if (tool?.name === 'AskUserQuestion') {
    console.log('⚠️ AskUserQuestion ToolResult with error, clearing pendingQuestion');
    if (pendingQuestion.value?.toolId === event.tool_id) {
      pendingQuestion.value = null;
    }
  }
}
```

---

## 未來官方支援後的改動計畫

當官方提供 AskUserQuestion Hook 機制後，需要進行以下改動：

### Phase 1: Hook 腳本更新

**檔案**：`resources/hooks/tsunu-permission.ps1`

```powershell
# 移除 AskUserQuestion 的跳過邏輯
# if ($hookInput.tool_name -eq "AskUserQuestion") {
#     exit 1
# }
```

### Phase 2: 新增 AskUserQuestion Hook 處理

**新增檔案**：`resources/hooks/tsunu-askuserquestion.ps1`

```powershell
# 處理 PreAskUserQuestion 事件
$hookInput = $inputJson | ConvertFrom-Json

# 發送問題到 Permission Server
$body = @{
    questions = $hookInput.questions
    session_id = $hookInput.session_id
} | ConvertTo-Json -Depth 10 -Compress

$response = Invoke-RestMethod -Uri "http://localhost:19751/askuserquestion/request" `
    -Method Post -Body $body -ContentType "application/json" -TimeoutSec 300

# 回傳答案
@{
    answers = $response.answers
} | ConvertTo-Json -Depth 10 -Compress
```

### Phase 3: Permission Server 新增端點

**檔案**：`src-tauri/src/permission_server.rs`

```rust
// 新增 AskUserQuestion 請求處理
async fn handle_askuserquestion_request(
    State(state): State<SharedPermissionState>,
    Json(req): Json<AskUserQuestionRequest>,
) -> Result<Json<AskUserQuestionResponse>, StatusCode> {
    // 發送事件到前端
    if let Some(app) = &state.lock().await.app_handle {
        let _ = app.emit("askuserquestion-request", &req);
    }

    // 等待用戶回答
    // ...
}
```

### Phase 4: 前端整合

**檔案**：`src/App.vue`

```typescript
// 監聽 AskUserQuestion Hook 事件
unlistenAskUserQuestion = await listen<AskUserQuestionEvent>('askuserquestion-request', (event) => {
  console.log('🤔 AskUserQuestion from Hook:', event.payload);
  pendingQuestion.value = {
    toolId: event.payload.tool_use_id,
    questions: event.payload.questions,
    isFromHook: true,  // 新增：標記來自 Hook
  };
});

// 回答問題時，透過 Permission Server 回傳
async function handleQuestionSubmit(answers: Record<string, string>) {
  if (pendingQuestion.value?.isFromHook) {
    await invoke('respond_to_askuserquestion', {
      toolUseId: pendingQuestion.value.toolId,
      answers,
    });
  } else {
    // 原有的 fallback 邏輯
  }
}
```

---

## 相關檔案清單

| 檔案 | 說明 |
|------|------|
| `resources/hooks/tsunu-permission.ps1` | Hook 腳本，目前跳過 AskUserQuestion |
| `src/App.vue` | 前端主邏輯，處理對話框顯示 |
| `src/components/AskUserQuestionDialog.vue` | 問題對話框元件 |
| `src/constants/autoAllowTools.ts` | AUTO_ALLOW_TOOLS 定義 |
| `src-tauri/src/permission_server.rs` | Permission HTTP Server |
| `src/utils/claudeEventHandler.ts` | 事件處理邏輯 |

---

## 參考資料

- [Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks)
- [GitHub Issue #10168 - Add Hook for User Input/Question Events](https://github.com/anthropics/claude-code/issues/10168)
- [GitHub Issue #12605 - AskUserQuestion Hook Support](https://github.com/anthropics/claude-code/issues/12605)
