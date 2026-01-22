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
- [ ] 測試人設切換機制

### Phase 4：介面優化與權限確認 🔄 部分完成
- [x] 設計對話氣泡樣式
- [x] 加入阿宇表情變化（4種狀態：idle, processing, complete, waiting）
- [x] 狀態列（編輯模式、檔案、Context 用量）
- [x] 斜線選單 UI（尚未實作功能）
- [x] PermissionDialog UI 元件
- [x] 研究 IDE 插件權限確認機制（完成！採用「後處理」模式）
- [ ] 實作「確認後重新執行」邏輯
- [ ] 實作 session 白名單管理
- [ ] 新增權限模式切換 UI
- [ ] 優化使用者體驗

### Phase 5：打包與發布
- [ ] 打包 macOS 版本
- [ ] 測試（請收禮者協助）
- [ ] 修正問題並完成

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
- [ ] 實作「確認後重新執行」邏輯
- [ ] 實作 session 白名單管理
- [ ] 實作專案級白名單持久化
- [ ] 新增權限模式切換 UI

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
