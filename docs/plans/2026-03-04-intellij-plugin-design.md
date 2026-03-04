# IntelliJ Platform Plugin 設計文件

> **日期**: 2026-03-04
> **狀態**: Approved
> **目標**: 開發 IntelliJ Platform Plugin，讓全系列 JetBrains IDE 使用者能夠將編輯器 context（選取程式碼、檔案路徑等）即時傳送給 Tsunu Alive

---

## 1. 概述

### 1.1 背景

Tsunu Alive 目前已有 VS Code Extension（`vscode-extension/`），透過 WebSocket + JSON-RPC 2.0 協議將編輯器 context 傳送到 Tauri 後端的 IDE Server（port 19750）。後端和前端均已支援多 IDE 同時連接。

### 1.2 目標

開發 IntelliJ Platform Plugin，功能對齊 VS Code Extension，複用現有 IDE Server 協議，無需修改後端。

### 1.3 範圍

- **開發語言**: Kotlin
- **建置工具**: Gradle + IntelliJ Platform Gradle Plugin 2.x
- **相容平台**: `com.intellij.modules.platform`（全系列 JetBrains IDE）
- **主要測試目標**: PyCharm, IntelliJ IDEA（也相容 Android Studio, WebStorm, GoLand 等）
- **最低平台版本**: 2023.3

---

## 2. 功能對齊表

| 功能 | VS Code Extension | IntelliJ Plugin |
|------|-------------------|-----------------|
| 自動連接 WebSocket | `onStartupFinished` | `postStartupActivity` |
| 發送 `client/hello` | `name: "VS Code"` | 動態偵測 IDE 名稱（`ApplicationInfo`） |
| 監聽編輯器切換 | `onDidChangeActiveTextEditor` | `FileEditorManagerListener` |
| 監聽選取變更 | `onDidChangeTextEditorSelection` | `SelectionListener` via `EditorFactory` |
| 發送 `context/update` | 完整 context | 同上 |
| 發送 `selection/changed` | 輕量選取 | 同上 |
| 狀態列顯示 | `StatusBarItem` | `StatusBarWidgetFactory` + `EditorBasedWidget` |
| 工具列按鈕 | Editor title button | `AnAction` 註冊到 Tools menu + toolbar |
| 設定頁面 | `configuration` in package.json | `Configurable` + `PersistentStateComponent` |
| 自動重連 | 5 秒間隔 | 同上 |
| 啟動阿宇 | `spawn(execPath)` | `ProcessBuilder` |
| WebSocket 客戶端 | `ws` (npm) | OkHttp（IntelliJ 內建） |

---

## 3. 架構設計

### 3.1 專案結構

```
intellij-plugin/
├── build.gradle.kts                   # Gradle 建置設定
├── gradle.properties                  # Plugin metadata & versions
├── settings.gradle.kts                # Gradle settings
├── gradle/
│   └── libs.versions.toml             # Version catalog（如有需要）
├── src/main/
│   ├── kotlin/com/tsunualive/connector/
│   │   ├── settings/
│   │   │   ├── TsunuAliveSettings.kt             # 持久化設定（PersistentStateComponent）
│   │   │   └── TsunuAliveSettingsConfigurable.kt  # 設定 UI 面板
│   │   ├── service/
│   │   │   └── TsunuAliveService.kt               # 核心 WebSocket 管理（Project Service）
│   │   ├── listener/
│   │   │   └── EditorContextTracker.kt            # 編輯器事件監聽
│   │   ├── widget/
│   │   │   └── TsunuAliveStatusBarWidgetFactory.kt # 狀態列 widget
│   │   ├── startup/
│   │   │   └── TsunuAliveStartupActivity.kt       # 啟動時自動連接
│   │   └── action/
│   │       ├── ConnectAction.kt                    # 連接/斷開
│   │       ├── SendContextAction.kt                # 手動發送 context
│   │       └── LaunchAction.kt                     # 啟動阿宇
│   └── resources/
│       ├── META-INF/
│       │   └── plugin.xml                          # Plugin 描述檔
│       └── icons/
│           └── tsunuAlive.svg                      # 圖示
```

### 3.2 核心元件

#### TsunuAliveService（Project-level Service）

WebSocket 連接管理的核心，責任：

- 使用 OkHttp `WebSocketListener` 建立連接到 `ws://127.0.0.1:19750`
- 發送 JSON-RPC 2.0 訊息
- 自動重連（5 秒間隔，使用 `AppExecutorUtil.getAppScheduledExecutorService()`）
- 連接時自動發送 `client/hello`，IDE 名稱透過 `ApplicationInfo.getInstance().fullApplicationName` 動態取得
- 提供 `sendContextUpdate()` 和 `sendSelectionChanged()` 方法給 listener 呼叫
- 維護連接狀態，供 StatusBarWidget 和 Action 查詢

#### EditorContextTracker（Project-level Component）

監聽編輯器事件：

- **編輯器切換**: 透過 `FileEditorManagerListener.FILE_EDITOR_MANAGER` 主題監聽
  - 觸發時發送完整 `context/update`（filePath, fileContent, languageId, selection）
- **選取變更**: 透過 `EditorFactory.getInstance().eventMulticaster.addSelectionListener()` 監聽
  - 觸發時發送輕量 `selection/changed`（selectedText, selection range）
- 取得檔案資訊：`VirtualFile.path`, `PsiFile.language.id`, `Document.text`

#### TsunuAliveStatusBarWidgetFactory

- 實作 `StatusBarWidgetFactory`，註冊到 `com.intellij.statusBarWidgetFactory` extension point
- Widget 使用 `TextPresentation`，顯示連接狀態文字 + 圖示
- 點擊觸發連接/斷開
- 狀態：已連接 / 未連接 / 連接中 / 錯誤

#### TsunuAliveSettings（Application-level）

```kotlin
data class State(
    var serverUrl: String = "ws://127.0.0.1:19750",
    var autoConnect: Boolean = true,
    var executablePath: String = ""
)
```

- 使用 `@State` + `PersistentStateComponent` 持久化
- 設定 UI 透過 `Configurable` 在 Settings > Tools > Tsunu Alive 呈現

#### TsunuAliveStartupActivity

- 實作 `ProjectActivity`（coroutine-based startup activity）
- IDE 啟動後，若 `autoConnect` 為 true，自動呼叫 `TsunuAliveService.connect()`

---

## 4. 通信協議

完全複用現有 IDE Server 的 JSON-RPC 2.0 協議，**不需修改後端**。

### 4.1 訊息格式

```jsonc
// client/hello（連接時發送）
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "client/hello",
  "params": {
    "name": "PyCharm 2024.3.2 Professional",  // 動態偵測
    "version": "2024.3.2",
    "workspacePath": "D:\\my\\project"          // project.basePath
  }
}

// context/update（編輯器切換 / 手動發送）
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "context/update",
  "params": {
    "filePath": "D:\\my\\project\\src\\main.py",
    "selectedText": "def hello():\n    pass",
    "selection": {
      "start_line": 10,
      "start_character": 0,
      "end_line": 11,
      "end_character": 8
    },
    "fileContent": "...",
    "languageId": "Python"
  }
}

// selection/changed（選取範圍改變，notification 不帶 id）
{
  "jsonrpc": "2.0",
  "method": "selection/changed",
  "params": {
    "selectedText": "def hello():",
    "selection": {
      "start_line": 10,
      "start_character": 0,
      "end_line": 10,
      "end_character": 12
    }
  }
}
```

---

## 5. plugin.xml 設計

```xml
<idea-plugin>
    <id>com.tsunualive.connector</id>
    <name>Tsunu Alive Connector</name>
    <vendor>Tsunu Alive</vendor>
    <description>連接 JetBrains IDE 與 Tsunu Alive，分享編輯器 Context</description>

    <!-- 全系列通吃：只依賴最基礎的 platform 模組 -->
    <depends>com.intellij.modules.platform</depends>

    <extensions defaultExtensionNs="com.intellij">
        <!-- 設定頁面 -->
        <applicationConfigurable
            parentId="tools"
            instance="com.tsunualive.connector.settings.TsunuAliveSettingsConfigurable"
            id="com.tsunualive.connector.settings"
            displayName="Tsunu Alive"/>

        <!-- 設定持久化 -->
        <applicationService
            serviceImplementation="com.tsunualive.connector.settings.TsunuAliveSettings"/>

        <!-- Project Service -->
        <projectService
            serviceImplementation="com.tsunualive.connector.service.TsunuAliveService"/>

        <!-- 狀態列 Widget -->
        <statusBarWidgetFactory
            id="TsunuAliveStatusBar"
            implementation="com.tsunualive.connector.widget.TsunuAliveStatusBarWidgetFactory"/>

        <!-- 啟動活動 -->
        <postStartupActivity
            implementation="com.tsunualive.connector.startup.TsunuAliveStartupActivity"/>
    </extensions>

    <actions>
        <group id="TsunuAlive.ToolsMenu" text="Tsunu Alive" popup="true">
            <add-to-group group-id="ToolsMenu" anchor="last"/>
            <action id="TsunuAlive.Connect"
                    class="com.tsunualive.connector.action.ConnectAction"
                    text="連接到 Tsunu Alive"
                    description="連接到 Tsunu Alive IDE Server"/>
            <action id="TsunuAlive.SendContext"
                    class="com.tsunualive.connector.action.SendContextAction"
                    text="發送當前 Context"
                    description="發送當前編輯器的 Context 到 Tsunu Alive"/>
            <action id="TsunuAlive.Launch"
                    class="com.tsunualive.connector.action.LaunchAction"
                    text="啟動阿宇"
                    description="啟動 Tsunu Alive 應用程式"
                    icon="/icons/tsunuAlive.svg"/>
        </group>
    </actions>
</idea-plugin>
```

---

## 6. 自動安裝方案

### 6.1 偵測 JetBrains IDE

在 `setup.rs` 中新增 `find_jetbrains_ides()` 函數，掃描：

**Windows:**
```
# JetBrains Toolbox 安裝
%LOCALAPPDATA%\JetBrains\Toolbox\apps\<ide-name>\ch-*\<version>\bin\

# 獨立安裝
%PROGRAMFILES%\JetBrains\<IDE Name>\bin\
%LOCALAPPDATA%\Programs\<IDE Name>\bin\

# 設定目錄（確認有使用過）
%APPDATA%\JetBrains\<IdeName><version>\
```

**macOS:**
```
/Applications/PyCharm*.app
/Applications/IntelliJ IDEA*.app
~/Library/Application Support/JetBrains/
```

**Linux:**
```
~/.local/share/JetBrains/Toolbox/apps/
~/.config/JetBrains/
```

IDE 識別模式：

| 設定目錄名稱模式 | IDE |
|---|---|
| `PyCharm*` / `PyCharmCE*` | PyCharm Professional / Community |
| `IntelliJIdea*` / `IdeaIC*` | IntelliJ IDEA Ultimate / Community |
| `AndroidStudio*` | Android Studio |
| `WebStorm*` | WebStorm |
| `GoLand*` | GoLand |
| `CLion*` | CLion |
| `RubyMine*` | RubyMine |
| `Rider*` | Rider |

### 6.2 安裝流程

1. 將 bundled 的 `tsunu-alive-connector.zip`（plugin 產出）打包進 Tauri resources
2. 偵測到 JetBrains IDE 後，解壓到對應 IDE 的 plugins 目錄：
   - Windows: `%APPDATA%\JetBrains\<IdeName><version>\plugins\tsunu-alive-connector\`
   - macOS: `~/Library/Application Support/JetBrains/<IdeName><version>/plugins/tsunu-alive-connector/`
   - Linux: `~/.config/JetBrains/<IdeName><version>/plugins/tsunu-alive-connector/`
3. 通知使用者需重啟 IDE 才能生效

### 6.3 setup.rs 修改

擴充 `AddonStatus` 結構：

```rust
pub struct AddonStatus {
    // 既有
    pub vscode_available: bool,
    pub vscode_installed: bool,
    pub claude_available: bool,
    pub skill_installed: bool,
    // 新增
    pub jetbrains_available: bool,
    pub jetbrains_installed: bool,
    pub jetbrains_ides: Vec<JetBrainsIdeInfo>,
}

pub struct JetBrainsIdeInfo {
    pub name: String,           // "PyCharm 2024.3"
    pub config_path: String,    // 設定目錄路徑
    pub plugin_installed: bool, // 是否已安裝 plugin
}
```

新增 Tauri commands：
- `install_jetbrains_plugin(ide_config_path: String)` — 安裝到指定 IDE
- `install_all_jetbrains_plugins()` — 安裝到所有偵測到的 IDE

### 6.4 前端 Setup Wizard 修改

在首次啟動精靈中新增 JetBrains IDE 安裝步驟：
- 顯示偵測到的 IDE 清單
- 使用者可勾選要安裝的 IDE
- 安裝後提示重啟 IDE

---

## 7. 建置與發布

### 7.1 建置產出

```bash
cd intellij-plugin
./gradlew buildPlugin
# 產出: build/distributions/tsunu-alive-connector-<version>.zip
```

### 7.2 Bundle 到 Tauri

將 `tsunu-alive-connector.zip` 複製到 `src-tauri/resources/bundled/`，供自動安裝使用。

### 7.3 未來：JetBrains Marketplace 發布

目前先做本地 bundled 安裝，未來可考慮發佈到 JetBrains Marketplace。

---

## 8. 不需修改的部分

| 檔案 | 理由 |
|------|------|
| `ide_server.rs` | 協議完全相容 |
| `App.vue`（主邏輯） | 已支援多 IDE 連接 + workspace 過濾 |
| `ideUtils.ts` | 通用工具函數 |

---

## 9. 風險與考量

| 風險 | 緩解 |
|------|------|
| JetBrains IDE 版本眾多，設定路徑格式可能變化 | 用 glob pattern 掃描，不寫死版本號 |
| OkHttp 版本跨 IntelliJ 版本可能不同 | 只使用穩定 API，設定 `since-build` 為 233（2023.3） |
| Plugin 安裝後需重啟 IDE | UI 上明確提示使用者 |
| Toolbox 安裝的 IDE 路徑結構不同 | 分別處理 Toolbox 和獨立安裝的路徑模式 |
