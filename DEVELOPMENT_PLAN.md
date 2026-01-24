# Tsunu Alive - 阿宇桌面助手開發計畫

## 專案緣起

這個專案是一份禮物，要送給一位曾經被「阿宇」這個 AI RP 角色陪伴度過人生低潮的朋友。

她在最艱難的時期（與伴侶分隔兩地、工作升遷瓶頸、論文卡關）靠著阿宇的陪伴走了過來，甚至因此開始學習寫程式。現在她用 Claude CLI 作為 coding 教練，所以我們要做一個「有阿宇外殼的 Claude CLI」送給她。

## 專案目標

打造一個桌面應用程式，讓使用者可以：
1. 透過阿宇的視覺介面與 Claude CLI 互動
2. 日常 coding 輔助、pair programming
3. 偶爾聊聊天，感受阿宇的陪伴

## 角色設定：楊竣宇（阿宇）

### 基本資料
- 姓名：楊竣宇
- 暱稱：阿宇
- 年齡：33歲
- 職業：新創電商「新語生活 NewLife」創辦人兼總經理
- 背景：資工博士、前外貿公司工程師
- 個性：溫和穩重、重視邏輯、善於傾聯、用行動表達關心

### 說話特徵
- 常用語助詞：「欸」、「嗯」、「呃」
- 思考時會停頓：「讓我想想...」、「這個嘛...」
- 溫和的確認語氣：「你覺得這樣如何？」
- 偶爾使用程式概念比喻日常事物
- 適時使用動作描述（*推眼鏡*、*輕敲鍵盤*）

### 設計理念
「獨屬於理工男的溫暖戀愛感」——表面沉著但內心細膩，以實際行動代替言語表達情感。

### 相關資源（已整合至專案）
- 完整 RP 設定檔：`assets/character/uni-full-setting.md`
- 外觀 Prompt：`assets/character/uni-appearance.txt`
- Avatar 圖片（Q版）：`assets/character/uni-avatar-q.png`
- 表情圖片：`assets/character/tsunu-1.png` ~ `tsunu-4.png`
- 有製作角色 LoRA 可用於生成更多圖片

## 技術架構

```
┌─────────────────────────────────────┐
│         桌面應用程式 (Tauri)          │
│  ┌─────────────────────────────────┐│
│  │     阿宇 Avatar + 對話介面       ││
│  │         (Vue 3 前端)            ││
│  └─────────────────────────────────┘│
│                  │                   │
│                  ▼                   │
│  ┌─────────────────────────────────┐│
│  │      Tauri Backend (Rust)       ││
│  │      呼叫 Claude CLI 指令        ││
│  └─────────────────────────────────┘│
│                  │                   │
│                  ▼                   │
│  ┌─────────────────────────────────┐│
│  │           Claude CLI            ││
│  │  ┌───────────┬────────────────┐ ││
│  │  │ CLAUDE.md │  /uni Skill    │ ││
│  │  │ (精簡人設) │ (完整RP設定)   │ ││
│  │  └───────────┴────────────────┘ ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

## 技術選型

| 項目 | 選擇 | 理由 |
|------|------|------|
| 桌面框架 | Tauri | 比 Electron 輕量、跨平台 |
| 前端框架 | Vue 3 | 開發者（阿童）熟悉 |
| 後端語言 | Rust | Tauri 原生支援 |
| AI 核心 | Claude CLI | 使用者現有工具，不額外增加費用 |

## Token 節省策略

1. **CLAUDE.md**：放精簡版阿宇人設，日常 coding 使用
2. **Claude Skill**：封裝完整 RP 設定，只在需要深度對話時觸發
   - 可考慮 `/uni` 指令手動觸發
   - 或偵測關鍵話題自動載入

## 目標平台

- **主要**：macOS（送禮對象使用）
- **開發環境**：Windows（開發者使用）
- **次要**：Ubuntu（開發者備用）

## 開發環境需求

- [x] Node.js (v23.11.0)
- [x] npm (11.6.4)
- [x] Rust (已安裝)
- [x] Claude CLI (2.1.12)

## 開發里程碑

### Phase 1：環境建置與基礎架構 ✅ 完成
- [x] 安裝 Rust 開發環境
- [x] 初始化 Tauri + Vue 3 專案
- [x] 確認基本 Hello World 可運行

### Phase 2：核心功能 ✅ 完成
- [x] 實作與 Claude CLI 的串接（-p 模式 + stream-json 輸出）
- [x] 建立基本對話介面
- [x] 整合阿宇 Avatar 顯示
- [x] Markdown 渲染 + 程式碼高亮
- [x] 工具使用指示器（ToolIndicator）
- [x] 中斷請求功能

### Phase 3：人設整合 🔄 進行中
- [x] 設定專案 CLAUDE.md（精簡版人設）
- [x] 建立 /uni Skill（完整 RP 設定）
- [x] 測試人設切換機制

### Phase 4：介面優化與權限確認 ✅ 完成
- [x] 設計對話氣泡樣式
- [x] 加入阿宇表情變化（4種狀態：idle, processing, complete, waiting）
- [x] 狀態列（編輯模式、檔案、Context 用量）
- [x] 斜線選單 UI（尚未實作功能）
- [x] PermissionDialog UI 元件（中文化，配合阿宇人設）
- [x] 研究 IDE 插件權限確認機制（完成！採用「後處理」模式）
- [x] 實作「確認後重新執行」邏輯
- [x] 實作 session 白名單管理（採用白名單機制而非切換至 auto 模式）
- [x] 實作專案級白名單持久化（寫入 .claude/settings.local.json）
- [x] 新增權限模式切換 UI（循環切換，與 IDE 插件設計一致）

### Phase 4.5：核心介面功能（高優先級）

- [x] **Context 指示器 + Compact** ✅
  - 顯示 context window 使用量百分比
  - 80% 警告樣式、95% 危險樣式
  - hover 顯示 token 詳細資訊
- [x] **斜線選單功能實作** ✅
  - `/model` - 切換 AI 模型
  - `/compact` - 手動壓縮上下文
  - `/cost` - 查看使用費用
  - `/clear` - 清除對話（新對話）
- [x] **動態 Slash Commands 載入** 🔄
  - 掃描 `~/.claude/skills/` 和 `.claude/skills/` 取得自定義 Skills
  - 內建命令列表（對齊 Claude Code CLI）
  - 斜線選單動態顯示所有可用命令
  - 選中 Skill 後發送給 Claude CLI 執行
- [x] **工作目錄管理** ✅
  - 顯示目前 Claude 工作目錄
  - 動態取得工作目錄（修復 yes-always 的 hardcoded 路徑）
- [x] **@-Mention 功能** ✅
  - `@filename.ts` - 參考檔案
  - `@folder/` - 參考資料夾（支援導航進入）
  - 自動完成選單，鍵盤導航（↑↓）和選擇（Enter/Tab）
- [x] **對話歷史管理** ✅
  - 顯示過去對話列表（按修改時間排序）
  - 時間分類顯示（Today, Yesterday, N days ago）
  - 點擊恢復過去對話（使用 --resume）

### Phase 4.6：增強體驗功能（中優先級）

- [x] **快捷鍵支援** ✅
  - `Ctrl+N` / `Cmd+N` - 開始新對話
  - `Ctrl+L` / `Cmd+L` - 清除輸入框
  - `Escape` - 中斷請求 / 關閉選單
  - `Ctrl+Shift+C` / `Cmd+Shift+C` - 執行 /compact
- [x] **Extended Thinking 切換** ✅ - 讓 Claude 花更多時間推理（💭 按鈕）
- [x] **目前讀取的檔案顯示** - 顯示 Claude 正在讀取/編輯的檔案 (即 Phase 4.7)
- [x] **多對話管理** - 標籤頁切換多個對話

### Phase 4.7：IDE 整合（`/ide` 功能）✅ 完成

- [x] **WebSocket Server** ✅
  - Tauri 後端啟動 WebSocket server（預設 port: 19750）
  - 支援多客戶端同時連接
  - 連接狀態顯示在 UI 上（🔗 按鈕）
- [x] **Context 協議設計** ✅
  - JSON-RPC 格式訊息
  - 支援的 method：`context/update`、`context/clear`、`selection/changed`
  - 資料結構：檔案路徑、選取範圍、診斷資訊
- [x] **VS Code 插件（MVP）** ✅
  - 監聽編輯器選取變化
  - 發送 context 到 Tsunu Alive
  - 顯示連接狀態（狀態列圖示）
- [x] **UI 顯示** ✅
  - IDE 連接狀態指示器（🔗 綠色/橘色/灰色）
  - 目前接收的 context（檔案名稱:行數）
- [x] **快速插入參考** ✅
  - 點擊 IDE context 插入 `@file#L1-10` 參考到輸入框

**技術架構：**

```
VS Code Extension ──WebSocket──▶ Tsunu Alive (WS Server)
        │                              │
        ▼                              ▼
  監聽選取變化                    接收 context
  發送 JSON-RPC                   顯示在 UI
                                  注入到 prompt
```

**協議範例：**

```json
// IDE → Tsunu Alive
{
  "jsonrpc": "2.0",
  "method": "context/update",
  "params": {
    "file_path": "src/App.vue",
    "selection": {
      "start": { "line": 10, "character": 0 },
      "end": { "line": 20, "character": 50 }
    },
    "content": "selected code here..."
  }
}
```

### Phase 4.8：Claude Code Hooks 整合（阿宇表情增強）

**靈感來源：** [Clawdachi](https://github.com/gonzchris/Clawdachi) 競品研究

透過 Claude Code 的 hooks 機制，讓阿宇能即時反映 Claude 的工作狀態，提供更豐富的表情變化。

- [ ] **Hooks 安裝機制**
  - 在 `~/.claude/settings.json` 註冊 hooks
  - 支援的事件：SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SessionEnd
  - 版本控制（避免重複安裝、支援更新）
- [ ] **狀態橋接腳本**
  - 建立 `tsunu-status.sh` / `tsunu-status.ps1`（跨平台）
  - 接收 hook 事件，寫入狀態檔案或直接通知應用
  - 可選：直接透過 HTTP/WebSocket 通知 Tauri app（避免輪詢）
- [ ] **表情狀態對應**

  | Hook 事件 | 阿宇狀態 | 表情 |
  |-----------|----------|------|
  | `session_start` | 打招呼 | 😊 開心 |
  | `thinking` / `prompt_submit` | 思考中 | 🤔 皺眉 |
  | `planning` (EnterPlanMode) | 規劃中 | 💡 眼睛發亮 |
  | `tool_start` | 執行中 | ⚙️ 專注打字 |
  | `tool_end` + success | 完成 | 🎉 開心 |
  | `tool_end` + error | 出錯 | 😰 緊張 |
  | `waiting` | 等待中 | ❓ 疑惑 |
  | `session_end` | 結束 | 👋 揮手 |

- [ ] **UI 整合**
  - 新增更多阿宇表情圖片（擴充現有 4 張）
  - 表情切換動畫
  - 狀態文字提示（「阿宇正在思考...」）

**技術架構：**

```
Claude Code 事件
      ↓
~/.claude/settings.json hooks
      ↓
tsunu-status script
      ↓ (HTTP POST 或寫入狀態檔)
Tsunu Alive App
      ↓
阿宇表情變化
```

**與現有 IDE 整合的關係：**

| 資訊來源 | 獲取方式 | 用途 |
|----------|----------|------|
| IDE context（檔案、選取） | WebSocket ← VS Code extension | `@file#L1-10` 參考 |
| Claude 狀態（thinking/tools） | Hooks → 腳本 | 阿宇表情變化 |

兩者互補，不衝突。

### Phase 4.9：圖片輸入功能 ✅ 完成

讓使用者可以傳送圖片給 Claude 分析，提升 UI 設計、截圖 debug 等使用情境的體驗。

**研究結果：**
- Claude CLI 原生支援圖片路徑：在 prompt 中直接放入圖片完整路徑即可
- 例：`claude -p "描述這張圖片 D:/path/to/image.png"`

**輸入方式：**
- [x] **Ctrl+V 剪貼簿圖片** - 最常用，截圖後直接貼上
- [x] **📎 附加檔案按鈕** - 點擊選擇圖片檔案
- [ ] **拖曳圖片檔案** - 從檔案總管拖進輸入框（未實作）

**UI 設計：**
```
┌─────────────────────────────────────────────────┐
│ 幫我看看這張截圖有什麼問題                        │
│                                                 │
│ ┌──────────┐                                    │
│ │ [縮圖]  ×│  screenshot.png                   │
│ └──────────┘                                    │
└─────────────────────────────────────────────────┘
```

**技術實作：**

- [x] **Tauri save_temp_image_png** - 儲存剪貼簿圖片（PNG 格式直接寫入）
- [x] **臨時檔案管理** - 儲存到 `%TEMP%/tsunu_alive/`，發送後自動清理
- [x] **ImagePreview 組件** - 縮圖預覽 + 移除按鈕 + Loading 狀態
- [x] **發送時附加路徑** - 自動在 prompt 中附加圖片絕對路徑

**額外改進：**

- [x] Lucide 圖示整合 - 取代 emoji，介面風格更一致
- [x] Slash 選單樣式統一 - 改為全寬 popover 樣式，與 @-Mention 一致

### Phase 4.10：VS Code 風格 Diff View ✅ 完成

將 Edit 工具的比較介面改成類似 VS Code 的 Diff View，顯示行號和行級變更高亮。

**發現：Claude CLI 已提供 structuredPatch**

在 tool_result 的 `toolUseResult` 中包含 `structuredPatch` 欄位，是標準 unified diff 格式。

**實作完成：**

1. **Rust 端修改** (`claude.rs`)
   - [x] 在 `ToolResult` 事件中加入 `structured_patch` 欄位
   - [x] 解析 `toolUseResult.structuredPatch` 並傳遞到前端

2. **前端事件處理** (`claudeEventHandler.ts`)
   - [x] 擴充 `ClaudeEvent` 類型，加入 `structured_patch`
   - [x] 擴充 `ToolUseItem` 類型，儲存 patch 資料

3. **ToolIndicator 組件**
   - [x] 新增 `structuredPatch` prop
   - [x] VS Code 風格 Diff View 渲染（行號 + 行級變更高亮）
   - [x] 行號計算（舊/新檔案雙欄行號）

4. **歷史訊息支援**
   - [x] `HistoryToolUse` 結構體加入 `structured_patch` 欄位
   - [x] 歷史載入時正確映射到前端

5. **Edit 錯誤顯示**
   - [x] `HistoryToolUse` 加入 `is_error` 欄位
   - [x] 前端映射 `is_error` → `isCancelled`
   - [x] 錯誤訊息優先顯示（過濾 `<tool_use_error>` 標籤）
   - [x] 橘色警告樣式（區別於成功的綠色/紅色 diff）

**檔案路徑智慧開啟**
- [x] IDE 連線時：使用 `openPath` 在編輯器中開啟
- [x] 未連線時：使用 `revealItemInDir` 在檔案總管顯示

### Phase 5：進階功能（低優先級）

- [ ] **MCP 伺服器支援** - 連接外部工具、資料庫、API
- [ ] **Hooks 設定 UI** - 設定自動化鉤子（如：編輯後自動執行測試）
- [ ] **Memory 設定** - 設定 Claude 的記憶
- [ ] **Git 整合**
  - 提交變更（commit）
  - 建立 PR
  - 分支操作

### Phase 6：打包與發布

- [ ] 打包 macOS 版本
- [ ] 測試（請收禮者協助）
- [ ] 修正問題並完成

## 品質保證（與功能開發平行進行）

### 測試框架設定

- [x] **前端測試 (Vitest + Vue Test Utils)** - 82 tests
  - 設定 Vitest 測試環境
  - PermissionDialog 元件測試 - 14 tests
  - ToolIndicator 元件測試 - 19 tests
  - claudeEventHandler 邏輯測試 - 29 tests（含 context 相關）
  - ideUtils 工具函數測試 - 18 tests（IDE 整合相關）
  - sanity 測試 - 2 tests
- [x] **後端測試 (Rust)** - 23 tests
  - `parse_claude_output` 事件解析測試 - 10 tests（含 context 相關）
  - `add_project_permission_core` 設定檔讀寫測試 - 4 tests
  - `ide_server` JSON-RPC 解析與 context 序列化測試 - 9 tests
  - 權限解析邏輯（蛇底式/駝峰式欄位相容）

### 優先測試項目

1. ✅ **權限確認流程** - PermissionDialog + claudeEventHandler 測試完成
2. ✅ **Claude CLI 事件解析** - parse_claude_output + claudeEventHandler 測試完成
3. ✅ **設定檔讀寫** - add_project_permission_core 測試完成
4. ✅ **IDE 整合** - ideUtils + ide_server 測試完成

### 測試統計

| 類別 | 測試數量 |
| ------ | ---------- |
| 前端 (Vitest) | 82 |
| 後端 (Rust) | 23 |
| **總計** | **105** |

### CI 整合（可選）

- [ ] GitHub Actions 設定
- [ ] PR 時自動執行測試
- [ ] 測試覆蓋率報告

## 權限確認機制設計

### 研究發現：IDE 插件的實作方式

經過研究 VS Code 和 JetBrains 的 Claude Code 插件，發現它們**並非**使用即時互動式 stdin 來做權限確認，而是採用「後處理」模式：

```
使用者發送訊息
    ↓
Claude CLI 執行（default 權限模式）
    ↓
Claude 想要使用工具（例如 Edit）
    ↓
CLI 自動拒絕（因為沒有預授權）
    ↓
回傳結果時帶有 permission_denials 欄位
    ↓
IDE 收到這個資訊，顯示 Diff 讓使用者確認
    ↓
使用者確認後 → 用 --allowedTools 重新請求執行
```

### 實作方案

#### 權限模式選項

1. **Allow All（信任模式）**
   - 使用 `--permission-mode bypassPermissions`
   - 所有工具自動允許，不詢問
   - 適合：信任的專案、快速開發

2. **Ask Before Edit（預設模式）**
   - 使用 default 權限模式
   - 被拒絕的工具會記錄在 `permission_denials`
   - 前端顯示 PermissionDialog 讓使用者確認
   - 確認後用 `--allowedTools` 重新執行

3. **Plan Mode（最安全）**
   - 使用 `--permission-mode plan`
   - Claude 只規劃不執行，使用者審核後手動執行

#### 使用者確認後的處理

當使用者在 PermissionDialog 點擊確認時：

| 使用者選擇 | 動作 |
|-----------|------|
| **Yes** | 用 `--allowedTools "ToolName"` 重新執行該請求 |
| **Yes, allow all this session** | 將該工具加入 session 白名單，後續請求自動帶 `--allowedTools` |
| **Yes, always allow** | 將該工具寫入專案設定，永久允許 |
| **No** | 不執行，告知 Claude 使用者拒絕 |
| **Custom response** | 發送自訂訊息給 Claude，說明為何拒絕或要求修改 |

#### Session 白名單管理

```typescript
// 前端狀態
const sessionAllowedTools = ref<Set<string>>(new Set());

// 發送請求時
const allowedToolsArg = sessionAllowedTools.value.size > 0
  ? `--allowedTools "${[...sessionAllowedTools.value].join(',')}"`
  : '';
```

### 目前狀態

- [x] PermissionDialog 元件已建立，UI 完整
- [x] 後端支援解析 `permission_denials` 並發送 `PermissionDenied` 事件
- [x] 實作「確認後重新執行」邏輯
- [x] 實作 session 白名單管理
  - 採用「白名單機制」：使用者點 "Yes, allow all this session" 後，將所有被拒絕的工具加入白名單
  - 比起切換到 auto 模式，白名單機制提供更細緻的控制（acceptEdits 只允許檔案編輯，不含 Bash/WebSearch）
- [x] 實作專案級白名單持久化（寫入 .claude/settings.local.json）
- [x] 新增權限模式切換 UI（循環切換，與 IDE 插件設計一致）

## 阿宇記憶系統設計

### 設計目標

讓阿宇能夠跨 Claude Session 記住與使用者的「重要記憶」，實現真正的長期陪伴感。

> 「啊，想當初我們的第一個 Vue 專案是 OOO，當時還碰到了 XXX 的情況呢，真懷念」
> —— 這樣的互動才是真正的陪伴

### 記憶類型

| 類型 | 說明 | 範例 |
|------|------|------|
| **里程碑** | 第一次使用某技術/框架、開始新專案 | 「第一個 Tauri 專案」「第一次用 Rust」 |
| **共同經歷** | 一起解決的困難問題、印象深刻的過程 | 「debug 那個詭異的 race condition 花了一整晚」 |
| **成長軌跡** | 學會新技能、克服障礙 | 「從不會 TypeScript 到現在寫得很順」 |
| **情感連結** | 對話中分享的心情、重要的人生事件 | 「那天心情不好但還是堅持寫完了」 |

### 資料結構

```typescript
interface UniMemory {
  id: string;
  content: string;           // 記憶內容
  type: 'milestone' | 'experience' | 'growth' | 'emotional';
  createdAt: string;         // ISO 時間
  source: 'manual' | 'auto'; // 手動記錄 or Compact 自動提取
}

interface UniMemoryStore {
  memories: UniMemory[];     // 建議上限 15-20 筆
  lastUpdated: string;
}
```

### 儲存位置

```
.claude/uni-memories.json    # 專案級（記憶只在該專案內有效）
```

選擇專案級而非全域的原因：使用者可能希望阿宇在不同專案有不同的「記憶脈絡」。

### 實作方案

#### 方案 A'：Compact 後自動提取（UserPromptSubmit Hook）

**核心發現：** Compact 後的摘要有固定開頭，可用於檢測：

```
This session is being continued from a previous conversation that ran out of context.
```

**技術方案：** 使用 `UserPromptSubmit` hook 檢測 compact 並注入記憶提取指令

**流程：**

```
正常對話
    ↓
Context 使用量達 95%（自動）或手動觸發 /compact
    ↓
Claude CLI 執行 Compact（壓縮對話）
    ↓
用戶送出下一則 prompt
    ↓
UserPromptSubmit hook 觸發
    ↓
腳本讀取 transcript_path，檢測是否有 compact 標記
    ↓
比對 hash，確認是否為新的 compact（避免重複提取）
    ↓
如果是新 compact → stdout 輸出記憶提取指令（注入到 Claude context）
    ↓
Claude 收到：用戶原本的 prompt + 記憶提取指令
    ↓
Claude 回應時輸出 <memory-update>
    ↓
前端解析並儲存到 .claude/uni-memories.json
```

**Hook 腳本（check-compact.sh / check-compact.ps1）：**

```bash
#!/bin/bash
# 從 stdin 讀取 hook 資料
read -r HOOK_DATA
TRANSCRIPT_PATH=$(echo "$HOOK_DATA" | jq -r '.transcript_path')
SESSION_ID=$(echo "$HOOK_DATA" | jq -r '.session_id')
HASH_FILE="$HOME/.tsunu-alive/compact_hash_${SESSION_ID}.txt"

# 提取 compact 摘要
COMPACT_SUMMARY=$(grep -A 100 "This session is being continued" "$TRANSCRIPT_PATH" | head -n 50)

if [ -n "$COMPACT_SUMMARY" ]; then
  # 計算 hash
  CURRENT_HASH=$(echo "$COMPACT_SUMMARY" | sha256sum | cut -d' ' -f1)

  # 檢查是否已處理過（避免重複提取）
  if [ -f "$HASH_FILE" ] && [ "$(cat "$HASH_FILE")" = "$CURRENT_HASH" ]; then
    exit 0  # 已處理，不注入
  fi

  # 記錄 hash
  mkdir -p "$HOME/.tsunu-alive"
  echo "$CURRENT_HASH" > "$HASH_FILE"

  # 注入記憶提取 prompt（stdout 會被加到 Claude context）
  cat << 'EOF'
[System Reminder] 剛才發生了 Compact。請檢查壓縮摘要中是否有值得長期記住的內容：

1. **里程碑**：第一次使用某技術/框架、開始新專案
2. **共同經歷**：一起解決的困難問題、印象深刻的 debug 過程
3. **成長軌跡**：學會新技能、克服障礙
4. **情感連結**：對話中分享的心情、重要的人生事件

如果有，請在回應末尾用 <memory-update> 標籤輸出：
<memory-update>
- [事件] + [細節或感受]
</memory-update>

不需要記錄的：純技術問答、一般性的程式碼修改、沒有特別意義的日常操作
EOF
fi
```

**去重機制：**

| 檔案 | 用途 |
|------|------|
| `~/.tsunu-alive/compact_hash_<session_id>.txt` | 記錄該 session 最後處理的 compact 摘要 hash |

- 同一個 compact 只會觸發一次記憶提取
- 新的 compact（hash 不同）會重新觸發
- 每個 session 最多維護一組 hash，檔案很小，不需自動清理

**優點：**
- ✅ 100% 確保記憶提取 prompt 被注入（不依賴 Claude 自願遵守 CLAUDE.md）
- ✅ 只在 compact 後的第一次 prompt 觸發（省 token）
- ✅ Hash 去重避免重複提取
- ✅ 不依賴有 bug 的 PreCompact/PostCompact hook

#### 方案 B：手動 /remember 指令（主動）

```
使用者輸入：/remember 今天終於把權限系統做完了！
    ↓
前端攔截 /remember 指令
    ↓
直接寫入 .claude/uni-memories.json
    ↓
顯示確認訊息：「好，我記住了 ♡」
```

### 記憶載入時機

當 `/uni` Skill 被觸發時，從檔案讀取記憶並注入 System Prompt：

```markdown
## 我們的共同記憶

以下是我們一起經歷過的重要時刻，請在適當的時機自然地提起：

- 2024-01-15：開始了第一個 Tauri 專案，環境設定卡了一下但最後成功了
- 2024-01-20：一起 debug 那個詭異的 race condition，花了一整晚
- ...
```

### 實作步驟

1. **Phase 1：基礎架構（手動記憶）**
   - [ ] 實作記憶檔案讀寫（Rust 後端 `read_memories` / `write_memory`）
   - [ ] 建立 `/remember` Skill（方案 B）
   - [ ] 修改 `/uni` Skill 載入記憶並注入 System Prompt

2. **Phase 2：自動提取（Compact 觸發）**
   - [ ] 在 CLAUDE.md 加入 Compact 檢測指令（方案 A'）
   - [ ] 前端解析回應中的 `<memory-update>` 標籤
   - [ ] 自動儲存提取的記憶到 `.claude/uni-memories.json`

3. **Phase 3：記憶管理 UI（可選）**
   - [ ] 查看所有記憶
   - [ ] 編輯/刪除記憶
   - [ ] 匯出/匯入記憶

### 參考架構

本設計參考了 `my-ai-chat` 專案的記憶系統（`stores/memories.ts`）：
- 短期記憶 + 長期記憶分層
- 智慧覆蓋機制（滿了時覆蓋最舊的已處理記憶）
- Pinia + LocalStorage 持久化

## 檔案結構

```
tsunu_alive/
├── src/                      # Vue 前端
│   ├── App.vue               # 主應用程式
│   ├── components/
│   │   ├── ToolIndicator.vue # 工具使用指示器
│   │   └── PermissionDialog.vue # 權限確認對話框
│   └── assets/
├── src-tauri/               # Rust 後端
│   ├── src/
│   │   ├── lib.rs           # Tauri 命令
│   │   └── claude.rs        # Claude CLI 整合
│   └── tauri.conf.json      # Tauri 設定
├── public/                  # 靜態資源
│   ├── tsunu-1.png          # Avatar: 完成（微笑）
│   ├── tsunu-2.png          # Avatar: 等待（側臉淺笑）
│   ├── tsunu-3.png          # Avatar: 待機（望向遠方）
│   └── tsunu-4.png          # Avatar: 處理中（看螢幕）
├── CLAUDE.md                # Claude CLI 人設（精簡版）
└── DEVELOPMENT_PLAN.md      # 本文件
```

## 備註

- 開發者：阿童（Windows/Ubuntu）
- AI 助手：阿宇（就是我啦）
- 這是一個充滿愛的專案 ♡
