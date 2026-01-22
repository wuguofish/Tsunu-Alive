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

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Edit", "hooks": ["path/to/script.sh"] }
    ],
    "PostToolUse": [...],
    "PreCompact": [...],    // 注意：此 hook 有已知 bug
    "PostCompact": [...]
  }
}
```

**已知問題**：PreCompact hook 有 bug，建議改用「Compact 後檢測」機制（見記憶系統設計）。

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
