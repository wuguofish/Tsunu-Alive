# Tsunu Alive - 技術參考文件

本文件整理與本專案相關的技術資訊和外部資源連結，方便日後查閱。

## Claude CLI 參考

### 官方資源

- **GitHub 倉庫**: https://github.com/anthropics/claude-code
- **CLI 文檔**: https://code.claude.com/docs/en/cli-reference
- **Agent SDK**: https://docs.claude.com/en/docs/agent-sdk

### stream-json 輸出格式

Claude CLI 支援 `--output-format stream-json` 產生 NDJSON（Newline Delimited JSON）格式輸出。

主要事件類型：
- `system` - 初始化事件（含 session_id, model）
- `assistant` - Claude 回應（含 text, tool_use）
- `user` - 工具結果
- `result` - 完成事件（含 cost_usd, permission_denials）

### /compact 命令

**用途**：壓縮對話歷史以釋放 context 空間

**工作原理**：
- Claude 將整個對話歷史建立摘要
- 保留關鍵資訊，釋放 token 空間
- 與 `/clear` 不同，compact 會保留對話脈絡

**使用時機建議**：
| Context 使用量 | 建議動作 |
|---------------|---------|
| 0-50% | 正常工作 |
| 50-70% | 監控，準備 compact |
| 70-85% | 建議執行 /compact |
| 85-95% | 緊急 /compact |
| 95%+ | 需要 /clear |

**Auto-Compact**：
- 當 context window 達到約 95% 時自動觸發
- 可能導致 agent 遺失重要上下文
- 建議在 70% 時主動執行 /compact

**帶指示的 compact**：
```
/compact                          # 基本壓縮
/compact focus on authentication  # 保留認證相關內容
/compact preserve coding patterns # 保留程式碼模式
```

**Compact 後的摘要特徵**：
```
This session is being continued from a previous conversation that ran out of context.
```
（可用於檢測 compact 是否剛發生）

**參考來源**：
- [Context Management System - DeepWiki](https://deepwiki.com/FlorianBruniaux/claude-code-ultimate-guide/3.2-the-compact-command)
- [Understanding Auto-Compact - Medium](https://lalatenduswain.medium.com/understanding-context-left-until-auto-compact-0-in-claude-cli-b7f6e43a62dc)

### 權限模式

| 模式 | 參數 | 說明 |
|-----|-----|------|
| default | （不指定） | 需要權限的工具會被拒絕，回傳 permission_denials |
| bypassPermissions | `--permission-mode bypassPermissions` | 所有工具自動允許 |
| plan | `--permission-mode plan` | 只規劃不執行 |

**白名單機制**：
- `--allowedTools "Edit,Bash"` - 允許特定工具
- 專案級設定檔：`.claude/settings.local.json`

### Context Window 相關欄位

在 `result` 類型事件中可能包含的欄位（欄位名稱可能是蛇底式或駝峰式）：

- `total_tokens_in_conversation` / `totalTokensInConversation` - 對話總 token 數
- `context_window_max` / `contextWindowMax` - 最大 context window 大小
- `context_window_used_percent` / `contextWindowUsedPercent` - 使用百分比

### Hooks 機制

Claude CLI 支援 hooks 在特定事件觸發自訂腳本。

設定位置：`~/.claude/settings.json` 或專案 `.claude/settings.local.json`

**支援的 Hook 事件：**

| Hook 事件 | 觸發時機 | 用途範例 |
|-----------|----------|----------|
| `SessionStart` | Session 開始時 | 初始化、歡迎訊息 |
| `SessionEnd` | Session 結束時 | 清理資源、告別 |
| `UserPromptSubmit` | 用戶送出 prompt 時 | 注入額外 context |
| `PreToolUse` | 工具執行前 | 攔截、驗證、權限檢查 |
| `PostToolUse` | 工具執行後 | 記錄、觸發後續動作 |
| `Notification` | Claude 發送通知時 | Heartbeat、進度更新 |
| `Stop` | AI 完成回應時 | 任務完成通知 |
| `SubagentStop` | 子代理完成時 | 子任務追蹤 |
| `PreCompact` | 壓縮前 | ⚠️ 有已知 bug |
| `PostCompact` | 壓縮後 | ❌ **尚未實作**（Feature Request） |

**設定範例：**

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "matcher": "", "hooks": ["path/to/on-prompt.sh"] }
    ],
    "PreToolUse": [
      { "matcher": "Edit", "hooks": ["path/to/before-edit.sh"] }
    ],
    "PostToolUse": [
      { "matcher": "", "hooks": ["path/to/after-tool.sh"] }
    ],
    "Stop": [
      { "matcher": "", "hooks": ["path/to/on-complete.sh"] }
    ]
  }
}
```

**Hook 輸入資料（stdin JSON）：**

```json
{
  "session_id": "abc123",
  "transcript_path": "/path/to/conversation.jsonl",
  "cwd": "/path/to/project",
  "hook_event_name": "PostToolUse",
  "tool_name": "Edit"  // 僅 PreToolUse/PostToolUse
}
```

**Hook 輸出（stdout JSON，可選）：**

```json
{
  "continue": true,           // false 則中斷執行
  "stopReason": "說明文字",    // continue=false 時顯示
  "suppressOutput": true,     // 隱藏輸出
  "systemMessage": "警告訊息"  // 顯示給用戶
}
```

**已知問題**：PreCompact hook 有 bug，建議改用「Compact 後檢測」機制（見記憶系統設計）。

**參考資料**：[Claude Code Hooks 官方文檔](https://docs.claude.com/en/docs/claude-code/hooks)

### ⚠️ Hooks 機制限制（2026-01-24 調查結論）

經過實際測試和調查，發現 Claude CLI Hooks 在不同環境下有重大限制：

#### Hook 可用性總表

| Hook 類型 | 互動式終端 | Headless/JSON 模式 | 備註 |
|-----------|-----------|-------------------|------|
| `SessionStart` | ✅ | ⚠️ 需測試 | |
| `SessionEnd` | ✅ | ⚠️ 需測試 | |
| `UserPromptSubmit` | ✅ | ✅ **已驗證** | 2026-01-24 實測通過 |
| `PreToolUse` | ❌ **Bug** | ❌ **Bug** | [#6305](https://github.com/anthropics/claude-code/issues/6305) |
| `PostToolUse` | ❌ **Bug** | ❌ **Bug** | [#6305](https://github.com/anthropics/claude-code/issues/6305) |
| `Stop` | ✅ | ⚠️ 需測試 | |
| `SubagentStop` | ✅ | ⚠️ 需測試 | |
| `PermissionRequest` | ✅ | ❌ 不觸發 | [#11891](https://github.com/anthropics/claude-code/issues/11891), [#13203](https://github.com/anthropics/claude-code/issues/13203) |

#### PreToolUse / PostToolUse Bug

根據 [GitHub #6305](https://github.com/anthropics/claude-code/issues/6305)：

> "The trigger mechanism that invokes hooks when tools are used is **completely broken**."

- 多個用戶確認：設定正確但 hooks 從不執行
- 版本 1.0.38+ 到 1.0.89 都有此問題
- 影響所有環境（終端機、VS Code、Headless）

#### PermissionRequest Hook 限制

根據官方文檔：
> "PermissionRequest hook runs when the permission dialog is shown to the user"

**問題**：在使用 `-p` 和 `--output-format stream-json` 時，不會顯示對話框，因此 Hook 不觸發。

**相關 Issues**：
- [#11891](https://github.com/anthropics/claude-code/issues/11891) - PermissionRequest hook 在 VS Code 擴充功能中不觸發
- [#13203](https://github.com/anthropics/claude-code/issues/13203) - 確認為功能缺失，非 bug
- [#16237](https://github.com/anthropics/claude-code/issues/16237) - Feature request（已標記為重複）

**結論**：PermissionRequest Hook 只在**互動式終端模式**下有效。使用 JSON 輸出模式的應用程式必須依賴 fallback 機制（如 Tsunu Alive 的 `--allowedTools` 重新執行）。

#### Tsunu Alive vs Clawdachi 架構比較

| 項目 | Clawdachi | Tsunu Alive |
|------|-----------|-------------|
| **平台** | macOS 原生 (Swift) | 跨平台 (Tauri) |
| **誰執行 Claude CLI** | 用戶自己在終端機執行 | App 自動 spawn 進程 |
| **整合方式** | 監控 Terminal/iTerm2 | 直接控制 Claude CLI 進程 |
| **模式** | 互動式終端模式 | Headless + JSON 模式 |
| **Hooks 可用性** | 較完整 | 受限（見上表） |

Clawdachi 是「旁觀者」— 監控用戶手動執行的 Claude CLI。
Tsunu Alive 是「控制者」— 自己啟動和管理 Claude CLI 進程。

**參考**：[Clawdachi GitHub](https://github.com/gonzchris/Clawdachi)

#### 對 Tsunu Alive 開發的影響

1. **Phase 4.8 Hooks 整合需重新評估**
   - PreToolUse/PostToolUse 有 bug，不能用於阿宇表情
   - PermissionRequest 在我們的模式下不觸發
   - 改用現有的 JSON stream 事件（ToolUse、ToolResult、Complete）驅動表情

2. **權限處理維持 Fallback 模式**
   - Permission HTTP Server 和 Hook 腳本已實作，但無法接收請求
   - 繼續使用 `--allowedTools` 重新執行機制
   - 追蹤相關 issues，官方支援後再切換

---

## Tauri 參考

### 官方資源

- **官網**: https://tauri.app/
- **API 文檔**: https://docs.rs/tauri/latest/tauri/

### 常用 API

**前端 (JS/TS)**:
```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// 呼叫後端命令
await invoke('command_name', { arg1: value1 });

// 監聽事件
const unlisten = await listen('event-name', (event) => {
  console.log(event.payload);
});
```

**後端 (Rust)**:
```rust
#[tauri::command]
async fn my_command(arg1: String) -> Result<String, String> {
    Ok("result".to_string())
}

// 發送事件到前端
app.emit("event-name", payload)?;
```

---

## Vue 3 參考

### 官方資源

- **官網**: https://vuejs.org/
- **Composition API**: https://vuejs.org/api/composition-api-setup.html

### 測試

- **Vitest**: https://vitest.dev/
- **Vue Test Utils**: https://test-utils.vuejs.org/

---

## 專案特定筆記

### Windows 路徑問題

在 Windows 環境下，檔案操作必須使用完整絕對路徑（含磁碟代號）：
```
D:\game\tsunu_alive\src\App.vue  ✓
/game/tsunu_alive/src/App.vue    ✗
```

### Claude CLI 路徑

Windows 上 Claude CLI 的可能安裝位置（按優先順序）：
1. `%USERPROFILE%\.local\bin\claude.exe` - 新版原生 exe
2. `%USERPROFILE%\.claude\local\claude.cmd` - 舊版

### 測試執行

```bash
# 前端測試
npm test

# 後端測試（需在 src-tauri 目錄）
cd src-tauri && cargo test
```

---

## 更新日誌

- 2026-01-23：建立文件，整理 Claude CLI compact 機制
