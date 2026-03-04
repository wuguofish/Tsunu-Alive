# IntelliJ Platform Plugin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Kotlin-based IntelliJ Platform Plugin that sends editor context (selected code, file path, language) to Tsunu Alive via WebSocket, matching the existing VS Code Extension's functionality.

**Architecture:** The plugin connects to the Tsunu Alive IDE Server (WebSocket on port 19750) using OkHttp (bundled in IntelliJ). It listens to editor events (file switch, selection change) and sends JSON-RPC 2.0 messages identical to the VS Code Extension. The Tauri backend (`setup.rs`) is extended to detect JetBrains IDEs and auto-install the plugin.

**Tech Stack:** Kotlin, Gradle + IntelliJ Platform Gradle Plugin 2.x, OkHttp WebSocket, IntelliJ Platform SDK (since-build 233 / 2023.3)

**Design doc:** `docs/plans/2026-03-04-intellij-plugin-design.md`

**Reference files:**
- VS Code Extension (feature parity target): `vscode-extension/src/extension.ts`
- IDE Server protocol: `src-tauri/src/ide_server.rs`
- Auto-install system: `src-tauri/src/setup.rs`
- Setup Wizard UI: `src/components/SetupWizard.vue`
- App.vue AddonStatus interface: `src/App.vue:446-458`
- Tauri command registration: `src-tauri/src/lib.rs:1598-1602`

---

## Task 1: Scaffold IntelliJ Plugin Project

**Files:**
- Create: `intellij-plugin/build.gradle.kts`
- Create: `intellij-plugin/settings.gradle.kts`
- Create: `intellij-plugin/gradle.properties`
- Create: `intellij-plugin/src/main/resources/META-INF/plugin.xml`

**Step 1: Create `intellij-plugin/settings.gradle.kts`**

```kotlin
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "0.9.0"
}

rootProject.name = "tsunu-alive-connector"
```

**Step 2: Create `intellij-plugin/gradle.properties`**

```properties
pluginGroup = com.tsunualive.connector
pluginName = Tsunu Alive Connector
pluginVersion = 0.1.0

platformType = IC
platformVersion = 2023.3

kotlin.stdlib.default.dependency = false
org.gradle.configuration-cache = true
```

**Step 3: Create `intellij-plugin/build.gradle.kts`**

```kotlin
plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "1.9.25"
    id("org.jetbrains.intellij.platform") version "2.2.1"
}

group = providers.gradleProperty("pluginGroup").get()
version = providers.gradleProperty("pluginVersion").get()

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        intellijIdeaCommunity(providers.gradleProperty("platformVersion"))
        instrumentationTools()
        pluginVerifier()
    }
}

kotlin {
    jvmToolchain(17)
}

intellijPlatform {
    pluginConfiguration {
        name = providers.gradleProperty("pluginName")
        version = providers.gradleProperty("pluginVersion")
        ideaVersion {
            sinceBuild = "233"
            untilBuild = provider { null }
        }
    }
    pluginVerification {
        ides {
            recommended()
        }
    }
}

tasks {
    wrapper {
        gradleVersion = "8.11.1"
    }
}
```

**Step 4: Create `intellij-plugin/src/main/resources/META-INF/plugin.xml`**

```xml
<idea-plugin>
    <id>com.tsunualive.connector</id>
    <name>Tsunu Alive Connector</name>
    <vendor>Tsunu Alive</vendor>
    <description><![CDATA[
    連接 JetBrains IDE 與 Tsunu Alive，分享編輯器 Context。
    讓阿宇能讀取你的 IDE 編輯器內容，提供更好的 pair programming 體驗。
    ]]></description>

    <depends>com.intellij.modules.platform</depends>

    <extensions defaultExtensionNs="com.intellij">
        <applicationService
            serviceImplementation="com.tsunualive.connector.settings.TsunuAliveSettings"/>

        <applicationConfigurable
            parentId="tools"
            instance="com.tsunualive.connector.settings.TsunuAliveSettingsConfigurable"
            id="com.tsunualive.connector.settings"
            displayName="Tsunu Alive"/>

        <projectService
            serviceImplementation="com.tsunualive.connector.service.TsunuAliveService"/>

        <statusBarWidgetFactory
            id="TsunuAliveStatusBar"
            implementation="com.tsunualive.connector.widget.TsunuAliveStatusBarWidgetFactory"
            order="after encodingWidget"/>

        <postStartupActivity
            implementation="com.tsunualive.connector.startup.TsunuAliveStartupActivity"/>
    </extensions>

    <actions>
        <group id="TsunuAlive.ToolsMenu" text="Tsunu Alive" popup="true">
            <add-to-group group-id="ToolsMenu" anchor="last"/>
            <action id="TsunuAlive.Connect"
                    class="com.tsunualive.connector.action.ConnectAction"
                    text="連接到 Tsunu Alive"
                    description="連接或斷開 Tsunu Alive IDE Server"/>
            <action id="TsunuAlive.SendContext"
                    class="com.tsunualive.connector.action.SendContextAction"
                    text="發送當前 Context"
                    description="發送當前編輯器的 Context"/>
            <action id="TsunuAlive.Launch"
                    class="com.tsunualive.connector.action.LaunchAction"
                    text="啟動阿宇"
                    description="啟動 Tsunu Alive 應用程式"
                    icon="/icons/tsunuAlive.svg"/>
        </group>
    </actions>
</idea-plugin>
```

**Step 5: Initialize Gradle wrapper**

Run: `cd intellij-plugin && gradle wrapper --gradle-version 8.11.1`

If `gradle` CLI is not available, manually create the wrapper files by downloading from the IntelliJ Platform Plugin Template repository.

**Step 6: Verify project compiles**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL (may have warnings about missing classes, that's OK at this stage)

**Step 7: Commit**

```bash
git add intellij-plugin/
git commit -m "feat(intellij): scaffold IntelliJ Platform Plugin project"
```

---

## Task 2: Settings (PersistentStateComponent)

**Files:**
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/settings/TsunuAliveSettings.kt`
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/settings/TsunuAliveSettingsConfigurable.kt`

**Step 1: Create `TsunuAliveSettings.kt`**

```kotlin
package com.tsunualive.connector.settings

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.util.xmlb.XmlSerializerUtil

@Service(Service.Level.APP)
@State(
    name = "com.tsunualive.connector.settings.TsunuAliveSettings",
    storages = [Storage("TsunuAliveSettings.xml")]
)
class TsunuAliveSettings : PersistentStateComponent<TsunuAliveSettings> {

    var serverUrl: String = "ws://127.0.0.1:19750"
    var autoConnect: Boolean = true
    var executablePath: String = ""

    override fun getState(): TsunuAliveSettings = this

    override fun loadState(state: TsunuAliveSettings) {
        XmlSerializerUtil.copyBean(state, this)
    }

    companion object {
        fun getInstance(): TsunuAliveSettings =
            ApplicationManager.getApplication().getService(TsunuAliveSettings::class.java)
    }
}
```

**Step 2: Create `TsunuAliveSettingsConfigurable.kt`**

```kotlin
package com.tsunualive.connector.settings

import com.intellij.openapi.options.Configurable
import javax.swing.*
import java.awt.GridBagConstraints
import java.awt.GridBagLayout
import java.awt.Insets

class TsunuAliveSettingsConfigurable : Configurable {

    private var serverUrlField: JTextField? = null
    private var autoConnectCheckbox: JCheckBox? = null
    private var executablePathField: JTextField? = null

    override fun getDisplayName(): String = "Tsunu Alive"

    override fun createComponent(): JComponent {
        val panel = JPanel(GridBagLayout())
        val gbc = GridBagConstraints().apply {
            fill = GridBagConstraints.HORIZONTAL
            insets = Insets(4, 4, 4, 4)
            anchor = GridBagConstraints.WEST
        }

        // Server URL
        gbc.gridx = 0; gbc.gridy = 0; gbc.weightx = 0.0
        panel.add(JLabel("Server URL:"), gbc)
        gbc.gridx = 1; gbc.weightx = 1.0
        serverUrlField = JTextField(30).also { panel.add(it, gbc) }

        // Auto Connect
        gbc.gridx = 0; gbc.gridy = 1; gbc.gridwidth = 2; gbc.weightx = 1.0
        autoConnectCheckbox = JCheckBox("啟動時自動連接").also { panel.add(it, gbc) }
        gbc.gridwidth = 1

        // Executable Path
        gbc.gridx = 0; gbc.gridy = 2; gbc.weightx = 0.0
        panel.add(JLabel("執行檔路徑:"), gbc)
        gbc.gridx = 1; gbc.weightx = 1.0
        executablePathField = JTextField(30).also { panel.add(it, gbc) }

        // Spacer
        gbc.gridx = 0; gbc.gridy = 3; gbc.weighty = 1.0; gbc.gridwidth = 2
        panel.add(JPanel(), gbc)

        reset()
        return panel
    }

    override fun isModified(): Boolean {
        val settings = TsunuAliveSettings.getInstance()
        return serverUrlField?.text != settings.serverUrl ||
                autoConnectCheckbox?.isSelected != settings.autoConnect ||
                executablePathField?.text != settings.executablePath
    }

    override fun apply() {
        val settings = TsunuAliveSettings.getInstance()
        settings.serverUrl = serverUrlField?.text ?: settings.serverUrl
        settings.autoConnect = autoConnectCheckbox?.isSelected ?: settings.autoConnect
        settings.executablePath = executablePathField?.text ?: settings.executablePath
    }

    override fun reset() {
        val settings = TsunuAliveSettings.getInstance()
        serverUrlField?.text = settings.serverUrl
        autoConnectCheckbox?.isSelected = settings.autoConnect
        executablePathField?.text = settings.executablePath
    }

    override fun disposeUIResources() {
        serverUrlField = null
        autoConnectCheckbox = null
        executablePathField = null
    }
}
```

**Step 3: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 4: Commit**

```bash
git add intellij-plugin/src/
git commit -m "feat(intellij): add settings with PersistentStateComponent"
```

---

## Task 3: Core WebSocket Service (TsunuAliveService)

**Files:**
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/service/TsunuAliveService.kt`

**Step 1: Create `TsunuAliveService.kt`**

This is the core service. It manages the OkHttp WebSocket connection and sends JSON-RPC messages.

Reference protocol from `src-tauri/src/ide_server.rs:80-104` (JsonRpcRequest format) and `vscode-extension/src/extension.ts:190-324` (connect/disconnect/send logic).

```kotlin
package com.tsunualive.connector.service

import com.intellij.openapi.Disposable
import com.intellij.openapi.application.ApplicationInfo
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.util.concurrency.AppExecutorUtil
import com.tsunualive.connector.settings.TsunuAliveSettings
import okhttp3.*
import org.json.JSONObject
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

enum class ConnectionState {
    DISCONNECTED, CONNECTING, CONNECTED, ERROR
}

@Service(Service.Level.PROJECT)
class TsunuAliveService(private val project: Project) : Disposable {

    private val log = Logger.getInstance(TsunuAliveService::class.java)
    private val client = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.SECONDS) // No timeout for WebSocket
        .build()
    private val messageId = AtomicInteger(0)
    private val wsRef = AtomicReference<WebSocket?>(null)
    private var reconnectFuture: ScheduledFuture<*>? = null

    var connectionState: ConnectionState = ConnectionState.DISCONNECTED
        private set

    // Listeners for state changes (used by StatusBarWidget)
    private val stateListeners = mutableListOf<() -> Unit>()

    fun addStateListener(listener: () -> Unit) {
        stateListeners.add(listener)
    }

    fun removeStateListener(listener: () -> Unit) {
        stateListeners.remove(listener)
    }

    private fun notifyStateChanged() {
        stateListeners.forEach { it() }
    }

    fun connect() {
        if (connectionState == ConnectionState.CONNECTED || connectionState == ConnectionState.CONNECTING) {
            return
        }

        val settings = TsunuAliveSettings.getInstance()
        val url = settings.serverUrl
        log.info("Connecting to Tsunu Alive at $url")

        connectionState = ConnectionState.CONNECTING
        notifyStateChanged()

        val request = Request.Builder().url(url).build()
        client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                log.info("Connected to Tsunu Alive")
                wsRef.set(webSocket)
                connectionState = ConnectionState.CONNECTED
                notifyStateChanged()
                cancelReconnect()
                sendHello()
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                try {
                    val msg = JSONObject(text)
                    if (msg.optString("method") == "server/hello") {
                        val params = msg.optJSONObject("params")
                        log.info("Server: ${params?.optString("name")} v${params?.optString("version")}")
                    }
                } catch (e: Exception) {
                    log.warn("Failed to parse message: $text", e)
                }
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(1000, null)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                log.info("Connection closed: $reason")
                wsRef.set(null)
                connectionState = ConnectionState.DISCONNECTED
                notifyStateChanged()
                scheduleReconnect()
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                log.warn("Connection failed: ${t.message}")
                wsRef.set(null)
                connectionState = ConnectionState.ERROR
                notifyStateChanged()
                scheduleReconnect()
            }
        })
    }

    fun disconnect() {
        cancelReconnect()
        wsRef.getAndSet(null)?.close(1000, "User disconnect")
        connectionState = ConnectionState.DISCONNECTED
        notifyStateChanged()
    }

    fun toggleConnection() {
        if (connectionState == ConnectionState.CONNECTED) {
            disconnect()
        } else {
            connect()
        }
    }

    private fun sendHello() {
        val appInfo = ApplicationInfo.getInstance()
        val ideName = appInfo.fullApplicationName
        val ideVersion = appInfo.fullVersion
        val workspacePath = project.basePath

        sendJsonRpc("client/hello", JSONObject().apply {
            put("name", ideName)
            put("version", ideVersion)
            if (workspacePath != null) put("workspacePath", workspacePath)
        }, withId = true)
    }

    fun sendContextUpdate(
        filePath: String?,
        selectedText: String?,
        selection: SelectionRange?,
        fileContent: String?,
        languageId: String?
    ) {
        val params = JSONObject().apply {
            filePath?.let { put("filePath", it) }
            selectedText?.let { put("selectedText", it) }
            selection?.let { put("selection", it.toJson()) }
            fileContent?.let { put("fileContent", it) }
            languageId?.let { put("languageId", it) }
        }
        sendJsonRpc("context/update", params, withId = true)
    }

    fun sendSelectionChanged(selectedText: String, selection: SelectionRange) {
        val params = JSONObject().apply {
            put("selectedText", selectedText)
            put("selection", selection.toJson())
        }
        // notification: no id (matches VS Code Extension behavior)
        sendJsonRpc("selection/changed", params, withId = false)
    }

    private fun sendJsonRpc(method: String, params: JSONObject, withId: Boolean) {
        val ws = wsRef.get() ?: return
        val msg = JSONObject().apply {
            put("jsonrpc", "2.0")
            if (withId) put("id", messageId.incrementAndGet())
            put("method", method)
            put("params", params)
        }
        ws.send(msg.toString())
        log.debug("Sent: $method")
    }

    private fun scheduleReconnect() {
        if (reconnectFuture != null) return
        val settings = TsunuAliveSettings.getInstance()
        if (!settings.autoConnect) return

        reconnectFuture = AppExecutorUtil.getAppScheduledExecutorService()
            .schedule({
                reconnectFuture = null
                if (connectionState != ConnectionState.CONNECTED) {
                    log.info("Attempting reconnect...")
                    connect()
                }
            }, 5, TimeUnit.SECONDS)
    }

    private fun cancelReconnect() {
        reconnectFuture?.cancel(false)
        reconnectFuture = null
    }

    override fun dispose() {
        disconnect()
        client.dispatcher.executorService.shutdown()
    }

    companion object {
        fun getInstance(project: Project): TsunuAliveService =
            project.getService(TsunuAliveService::class.java)
    }
}

data class SelectionRange(
    val startLine: Int,
    val startCharacter: Int,
    val endLine: Int,
    val endCharacter: Int
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("start_line", startLine)
        put("start_character", startCharacter)
        put("end_line", endLine)
        put("end_character", endCharacter)
    }
}
```

**Important note:** OkHttp is bundled with IntelliJ Platform. For `org.json.JSONObject`, IntelliJ also bundles it. No extra dependencies needed.

**Step 2: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 3: Commit**

```bash
git add intellij-plugin/src/
git commit -m "feat(intellij): add core WebSocket service with JSON-RPC"
```

---

## Task 4: Editor Context Tracker (Listener)

**Files:**
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/listener/EditorContextTracker.kt`

**Step 1: Create `EditorContextTracker.kt`**

This listens to editor events and calls `TsunuAliveService` to send context updates.

Reference: `vscode-extension/src/extension.ts:330-385` for what data to send on each event.

```kotlin
package com.tsunualive.connector.listener

import com.intellij.openapi.Disposable
import com.intellij.openapi.components.Service
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.event.SelectionEvent
import com.intellij.openapi.editor.event.SelectionListener
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerEvent
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.tsunualive.connector.service.SelectionRange
import com.tsunualive.connector.service.TsunuAliveService

@Service(Service.Level.PROJECT)
class EditorContextTracker(private val project: Project) : Disposable {

    private val selectionListener = object : SelectionListener {
        override fun selectionChanged(e: SelectionEvent) {
            val editor = e.editor
            // Only track editors belonging to this project
            val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
            if (!belongsToProject(file)) return

            val selectedText = editor.selectionModel.selectedText ?: ""
            val selection = getSelectionRange(editor)
            TsunuAliveService.getInstance(project).sendSelectionChanged(selectedText, selection)
        }
    }

    fun start() {
        // Listen for selection changes across all editors
        EditorFactory.getInstance().eventMulticaster.addSelectionListener(selectionListener, this)

        // Listen for file editor changes via message bus
        project.messageBus.connect(this)
            .subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
                override fun selectionChanged(event: FileEditorManagerEvent) {
                    val file = event.newFile ?: return
                    val editor = FileEditorManager.getInstance(project).selectedTextEditor ?: return
                    sendFullContext(editor, file)
                }
            })
    }

    fun sendCurrentContext() {
        val editor = FileEditorManager.getInstance(project).selectedTextEditor ?: return
        val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
        sendFullContext(editor, file)
    }

    private fun sendFullContext(editor: Editor, file: VirtualFile) {
        val document = editor.document
        val selectedText = editor.selectionModel.selectedText ?: ""
        val selection = getSelectionRange(editor)
        val fileContent = document.text
        val languageId = file.fileType.name

        TsunuAliveService.getInstance(project).sendContextUpdate(
            filePath = file.path,
            selectedText = selectedText,
            selection = selection,
            fileContent = fileContent,
            languageId = languageId
        )
    }

    private fun getSelectionRange(editor: Editor): SelectionRange {
        val selModel = editor.selectionModel
        val doc = editor.document
        val startOffset = selModel.selectionStart
        val endOffset = selModel.selectionEnd

        val startLine = doc.getLineNumber(startOffset)
        val endLine = doc.getLineNumber(endOffset)
        val startChar = startOffset - doc.getLineStartOffset(startLine)
        val endChar = endOffset - doc.getLineStartOffset(endLine)

        return SelectionRange(startLine, startChar, endLine, endChar)
    }

    private fun belongsToProject(file: VirtualFile): Boolean {
        val basePath = project.basePath ?: return true
        return file.path.startsWith(basePath)
    }

    override fun dispose() {
        // SelectionListener is auto-removed via Disposable parent
    }

    companion object {
        fun getInstance(project: Project): EditorContextTracker =
            project.getService(EditorContextTracker::class.java)
    }
}
```

**Step 2: Register as project service in plugin.xml**

Add to `plugin.xml` extensions section:

```xml
<projectService
    serviceImplementation="com.tsunualive.connector.listener.EditorContextTracker"/>
```

**Step 3: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 4: Commit**

```bash
git add intellij-plugin/src/
git commit -m "feat(intellij): add editor context tracker with selection listener"
```

---

## Task 5: Status Bar Widget

**Files:**
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/widget/TsunuAliveStatusBarWidgetFactory.kt`

**Step 1: Create `TsunuAliveStatusBarWidgetFactory.kt`**

Reference: `vscode-extension/src/extension.ts:391-418` for status states and display text.

```kotlin
package com.tsunualive.connector.widget

import com.intellij.openapi.project.Project
import com.intellij.openapi.util.Disposer
import com.intellij.openapi.wm.StatusBar
import com.intellij.openapi.wm.StatusBarWidget
import com.intellij.openapi.wm.StatusBarWidgetFactory
import com.tsunualive.connector.service.ConnectionState
import com.tsunualive.connector.service.TsunuAliveService
import java.awt.event.MouseEvent
import java.util.function.Consumer

class TsunuAliveStatusBarWidgetFactory : StatusBarWidgetFactory {

    override fun getId(): String = WIDGET_ID

    override fun getDisplayName(): String = "Tsunu Alive"

    override fun createWidget(project: Project): StatusBarWidget =
        TsunuAliveStatusBarWidget(project)

    override fun disposeWidget(widget: StatusBarWidget) {
        Disposer.dispose(widget)
    }

    companion object {
        const val WIDGET_ID = "TsunuAliveStatusBar"
    }
}

class TsunuAliveStatusBarWidget(private val project: Project) : StatusBarWidget,
    StatusBarWidget.TextPresentation {

    private var statusBar: StatusBar? = null

    private val stateListener: () -> Unit = {
        statusBar?.updateWidget(ID())
    }

    override fun ID(): String = TsunuAliveStatusBarWidgetFactory.WIDGET_ID

    override fun install(statusBar: StatusBar) {
        this.statusBar = statusBar
        TsunuAliveService.getInstance(project).addStateListener(stateListener)
    }

    override fun getPresentation(): StatusBarWidget.WidgetPresentation = this

    override fun getText(): String {
        return when (TsunuAliveService.getInstance(project).connectionState) {
            ConnectionState.CONNECTED -> "Tsunu Alive: 已連接"
            ConnectionState.DISCONNECTED -> "Tsunu Alive: 未連接"
            ConnectionState.CONNECTING -> "Tsunu Alive: 連接中..."
            ConnectionState.ERROR -> "Tsunu Alive: 錯誤"
        }
    }

    override fun getTooltipText(): String {
        return when (TsunuAliveService.getInstance(project).connectionState) {
            ConnectionState.CONNECTED -> "已連接到 Tsunu Alive（點擊斷開）"
            ConnectionState.DISCONNECTED -> "未連接（點擊連接）"
            ConnectionState.CONNECTING -> "連接中..."
            ConnectionState.ERROR -> "連接錯誤（點擊重試）"
        }
    }

    override fun getAlignment(): Float = 0f

    override fun getClickConsumer(): Consumer<MouseEvent> = Consumer {
        TsunuAliveService.getInstance(project).toggleConnection()
    }

    override fun dispose() {
        TsunuAliveService.getInstance(project).removeStateListener(stateListener)
        statusBar = null
    }
}
```

**Step 2: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 3: Commit**

```bash
git add intellij-plugin/src/
git commit -m "feat(intellij): add status bar widget showing connection state"
```

---

## Task 6: Startup Activity + Actions

**Files:**
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/startup/TsunuAliveStartupActivity.kt`
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/action/ConnectAction.kt`
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/action/SendContextAction.kt`
- Create: `intellij-plugin/src/main/kotlin/com/tsunualive/connector/action/LaunchAction.kt`

**Step 1: Create `TsunuAliveStartupActivity.kt`**

```kotlin
package com.tsunualive.connector.startup

import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity
import com.tsunualive.connector.listener.EditorContextTracker
import com.tsunualive.connector.service.TsunuAliveService
import com.tsunualive.connector.settings.TsunuAliveSettings

class TsunuAliveStartupActivity : ProjectActivity {

    override suspend fun execute(project: Project) {
        // Start editor context tracking
        EditorContextTracker.getInstance(project).start()

        // Auto-connect if enabled
        if (TsunuAliveSettings.getInstance().autoConnect) {
            TsunuAliveService.getInstance(project).connect()
        }
    }
}
```

**Step 2: Create `ConnectAction.kt`**

```kotlin
package com.tsunualive.connector.action

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.tsunualive.connector.service.ConnectionState
import com.tsunualive.connector.service.TsunuAliveService

class ConnectAction : AnAction() {

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        TsunuAliveService.getInstance(project).toggleConnection()
    }

    override fun update(e: AnActionEvent) {
        val project = e.project
        if (project == null) {
            e.presentation.isEnabled = false
            return
        }
        val state = TsunuAliveService.getInstance(project).connectionState
        e.presentation.text = when (state) {
            ConnectionState.CONNECTED -> "斷開 Tsunu Alive"
            else -> "連接到 Tsunu Alive"
        }
    }
}
```

**Step 3: Create `SendContextAction.kt`**

```kotlin
package com.tsunualive.connector.action

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.tsunualive.connector.listener.EditorContextTracker
import com.tsunualive.connector.service.ConnectionState
import com.tsunualive.connector.service.TsunuAliveService

class SendContextAction : AnAction() {

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        EditorContextTracker.getInstance(project).sendCurrentContext()
    }

    override fun update(e: AnActionEvent) {
        val project = e.project
        e.presentation.isEnabled = project != null &&
            TsunuAliveService.getInstance(project).connectionState == ConnectionState.CONNECTED
    }
}
```

**Step 4: Create `LaunchAction.kt`**

Reference: `vscode-extension/src/extension.ts:93-184` for executable path search logic.

```kotlin
package com.tsunualive.connector.action

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.diagnostic.Logger
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.tsunualive.connector.settings.TsunuAliveSettings
import java.io.File

class LaunchAction : AnAction() {

    private val log = Logger.getInstance(LaunchAction::class.java)

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project
        val execPath = findExecutable()
        if (execPath == null) {
            notify(project, "找不到 Tsunu Alive 執行檔。請在設定中指定路徑。", NotificationType.ERROR)
            return
        }

        try {
            val args = mutableListOf(execPath)
            project?.basePath?.let {
                args.add("--project")
                args.add(it)
            }
            ProcessBuilder(args)
                .directory(project?.basePath?.let { File(it) })
                .start()

            val projectName = project?.basePath?.let { File(it).name } ?: ""
            notify(project, "已啟動 Tsunu Alive${if (projectName.isNotEmpty()) " (專案: $projectName)" else ""}", NotificationType.INFORMATION)
        } catch (ex: Exception) {
            log.error("Failed to launch Tsunu Alive", ex)
            notify(project, "啟動失敗: ${ex.message}", NotificationType.ERROR)
        }
    }

    private fun findExecutable(): String? {
        val configPath = TsunuAliveSettings.getInstance().executablePath
        if (configPath.isNotBlank() && File(configPath).exists()) {
            return configPath
        }

        val home = System.getProperty("user.home") ?: return null
        val isWindows = System.getProperty("os.name").lowercase().contains("win")
        val isMac = System.getProperty("os.name").lowercase().contains("mac")

        val candidates = mutableListOf<String>()

        if (isWindows) {
            val localAppData = System.getenv("LOCALAPPDATA") ?: "$home\\AppData\\Local"
            candidates.add("$localAppData\\tsunu-alive\\tsunu_alive.exe")
            candidates.add("$home\\.local\\bin\\tsunu_alive.exe")
        } else if (isMac) {
            candidates.add("/Applications/Tsunu Alive.app/Contents/MacOS/tsunu_alive")
            candidates.add("$home/Applications/Tsunu Alive.app/Contents/MacOS/tsunu_alive")
            candidates.add("$home/.local/bin/tsunu_alive")
            candidates.add("/usr/local/bin/tsunu_alive")
        } else {
            candidates.add("$home/.local/bin/tsunu_alive")
            candidates.add("/usr/local/bin/tsunu_alive")
        }

        return candidates.firstOrNull { File(it).exists() }
    }

    private fun notify(project: com.intellij.openapi.project.Project?, content: String, type: NotificationType) {
        // Use balloon notification (simple, no notification group registration needed)
        NotificationGroupManager.getInstance()
            .getNotificationGroup("Tsunu Alive Notifications")
            .createNotification(content, type)
            .notify(project)
    }
}
```

**Step 5: Register notification group in plugin.xml**

Add to `plugin.xml` extensions section:

```xml
<notificationGroup id="Tsunu Alive Notifications"
                    displayType="BALLOON"/>
```

**Step 6: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 7: Commit**

```bash
git add intellij-plugin/src/
git commit -m "feat(intellij): add startup activity, actions, and launch support"
```

---

## Task 7: Add Plugin Icon

**Files:**
- Create: `intellij-plugin/src/main/resources/icons/tsunuAlive.svg`

**Step 1: Create a simple SVG icon**

Create a minimal 16x16 SVG icon. If the project already has an icon, convert it. Otherwise use a simple placeholder:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="#4a90d9" stroke="#357abd" stroke-width="1"/>
  <text x="8" y="11" text-anchor="middle" font-size="8" font-family="Arial" fill="white" font-weight="bold">宇</text>
</svg>
```

**Step 2: Verify build**

Run: `cd intellij-plugin && ./gradlew build`
Expected: BUILD SUCCESSFUL

**Step 3: Commit**

```bash
git add intellij-plugin/src/main/resources/icons/
git commit -m "feat(intellij): add plugin icon"
```

---

## Task 8: Build & Manual Test

**Step 1: Build the plugin distribution**

Run: `cd intellij-plugin && ./gradlew buildPlugin`
Expected: Creates `build/distributions/tsunu-alive-connector-0.1.0.zip`

**Step 2: Manual test in IDE sandbox**

Run: `cd intellij-plugin && ./gradlew runIde`
Expected: A sandboxed IntelliJ IDEA instance opens with the plugin installed. Verify:
- Status bar shows "Tsunu Alive: 未連接" at the bottom right
- Tools menu has "Tsunu Alive" submenu with 3 actions
- Settings > Tools > Tsunu Alive shows the settings panel
- If Tsunu Alive app is running, clicking status bar connects and shows "已連接"

**Step 3: Fix any issues found during testing**

**Step 4: Commit any fixes**

```bash
git add intellij-plugin/
git commit -m "fix(intellij): address issues found during manual testing"
```

---

## Task 9: Auto-Install — Detect JetBrains IDEs (setup.rs)

**Files:**
- Modify: `src-tauri/src/setup.rs`

**Step 1: Add JetBrainsIdeInfo struct and find_jetbrains_ides()**

Add after the existing `AddonStatus` struct in `setup.rs`:

```rust
/// JetBrains IDE 資訊
#[derive(Debug, Clone, Serialize)]
pub struct JetBrainsIdeInfo {
    /// IDE 顯示名稱（如 "PyCharm 2024.3"）
    pub name: String,
    /// 設定目錄的完整路徑
    pub config_path: String,
    /// plugins 目錄路徑
    pub plugins_path: String,
    /// Plugin 是否已安裝
    pub plugin_installed: bool,
}
```

**Step 2: Extend AddonStatus**

```rust
pub struct AddonStatus {
    pub vscode_available: bool,
    pub vscode_installed: bool,
    pub claude_available: bool,
    pub skill_installed: bool,
    // New fields
    pub jetbrains_available: bool,
    pub jetbrains_installed: bool,
    pub jetbrains_ides: Vec<JetBrainsIdeInfo>,
}
```

**Step 3: Implement `find_jetbrains_ides()`**

```rust
/// 偵測已安裝的 JetBrains IDE
fn find_jetbrains_ides() -> Vec<JetBrainsIdeInfo> {
    let mut ides = Vec::new();

    // 取得 JetBrains 設定根目錄
    let config_roots = get_jetbrains_config_roots();

    for root in config_roots {
        if !root.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_jetbrains_ide_dir(&name) && entry.path().is_dir() {
                    let config_path = entry.path();
                    let plugins_path = config_path.join("plugins");
                    let plugin_installed = plugins_path
                        .join("tsunu-alive-connector")
                        .join("lib")
                        .exists();

                    ides.push(JetBrainsIdeInfo {
                        name: humanize_ide_name(&name),
                        config_path: config_path.to_string_lossy().to_string(),
                        plugins_path: plugins_path.to_string_lossy().to_string(),
                        plugin_installed,
                    });
                }
            }
        }
    }

    ides
}

fn get_jetbrains_config_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            roots.push(PathBuf::from(appdata).join("JetBrains"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home_dir() {
            roots.push(home.join("Library").join("Application Support").join("JetBrains"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = home_dir() {
            roots.push(home.join(".config").join("JetBrains"));
        }
    }

    roots
}

/// 判斷目錄名稱是否為 JetBrains IDE 設定目錄
fn is_jetbrains_ide_dir(name: &str) -> bool {
    let prefixes = [
        "PyCharm", "PyCharmCE",
        "IntelliJIdea", "IdeaIC",
        "AndroidStudio", "AndroidStudioPreview",
        "WebStorm", "GoLand", "CLion", "RubyMine", "Rider",
        "PhpStorm", "DataGrip", "RustRover",
    ];
    prefixes.iter().any(|p| name.starts_with(p))
}

/// 將目錄名稱轉換為可讀的 IDE 名稱
fn humanize_ide_name(dir_name: &str) -> String {
    let mappings = [
        ("PyCharmCE", "PyCharm Community"),
        ("PyCharm", "PyCharm Professional"),
        ("IntelliJIdea", "IntelliJ IDEA Ultimate"),
        ("IdeaIC", "IntelliJ IDEA Community"),
        ("AndroidStudio", "Android Studio"),
        ("WebStorm", "WebStorm"),
        ("GoLand", "GoLand"),
        ("CLion", "CLion"),
        ("RubyMine", "RubyMine"),
        ("Rider", "Rider"),
        ("PhpStorm", "PhpStorm"),
        ("DataGrip", "DataGrip"),
        ("RustRover", "RustRover"),
    ];

    for (prefix, display_name) in &mappings {
        if dir_name.starts_with(prefix) {
            let version = &dir_name[prefix.len()..];
            return format!("{} {}", display_name, version);
        }
    }

    dir_name.to_string()
}
```

**Step 4: Update `check_addon_status()` to include JetBrains info**

```rust
pub fn check_addon_status() -> AddonStatus {
    // ... existing code ...
    let jetbrains_ides = find_jetbrains_ides();
    let jetbrains_available = !jetbrains_ides.is_empty();
    let jetbrains_installed = jetbrains_ides.iter().any(|ide| ide.plugin_installed);

    AddonStatus {
        vscode_available,
        vscode_installed,
        claude_available,
        skill_installed,
        jetbrains_available,
        jetbrains_installed,
        jetbrains_ides,
    }
}
```

**Step 5: Verify build**

Run: `cd src-tauri && cargo check`
Expected: No errors

**Step 6: Commit**

```bash
git add src-tauri/src/setup.rs
git commit -m "feat(setup): add JetBrains IDE detection"
```

---

## Task 10: Auto-Install — Install Plugin Command (setup.rs)

**Files:**
- Modify: `src-tauri/src/setup.rs`
- Modify: `src-tauri/src/lib.rs` (register new command)

**Step 1: Add `install_jetbrains_plugin` command to `setup.rs`**

```rust
/// 安裝 JetBrains Plugin（從 bundled resource 解壓到指定 IDE 的 plugins 目錄）
#[tauri::command]
pub fn install_jetbrains_plugin(app: tauri::AppHandle, plugins_path: String) -> Result<String, String> {
    let bundled_dir = get_bundled_dir(&app)?;
    let zip_path = bundled_dir.join("tsunu-alive-connector.zip");

    if !zip_path.exists() {
        return Err(format!("Plugin zip not found: {:?}", zip_path));
    }

    let dest = PathBuf::from(&plugins_path).join("tsunu-alive-connector");

    // 若已存在，先刪除舊版
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove old plugin: {}", e))?;
    }

    // 解壓 zip
    let file = fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let out_path = PathBuf::from(&plugins_path).join(entry.mangled_name());

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent dir: {}", e))?;
            }
            let mut outfile = fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    Ok("JetBrains Plugin installed successfully. Please restart your IDE.".to_string())
}
```

**Step 2: Add `zip` dependency to `Cargo.toml`**

Add to `[dependencies]`:
```toml
zip = "2"
```

**Step 3: Register new command in `lib.rs`**

Add `setup::install_jetbrains_plugin` to the `invoke_handler` list at `src-tauri/src/lib.rs:1598-1602`.

**Step 4: Verify build**

Run: `cd src-tauri && cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/src/setup.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat(setup): add JetBrains plugin installation command"
```

---

## Task 11: Update Frontend — SetupWizard & AddonStatus

**Files:**
- Modify: `src/components/SetupWizard.vue`
- Modify: `src/App.vue` (AddonStatus interface)

**Step 1: Update `AddonStatus` interface in `App.vue:446-458`**

```typescript
interface AddonStatus {
  vscode_available: boolean;
  vscode_installed: boolean;
  claude_available: boolean;
  skill_installed: boolean;
  // New
  jetbrains_available: boolean;
  jetbrains_installed: boolean;
  jetbrains_ides: Array<{
    name: string;
    config_path: string;
    plugins_path: string;
    plugin_installed: boolean;
  }>;
}
```

**Step 2: Update `hasUninstalledAddons` computed in `App.vue:453-458`**

```typescript
const hasUninstalledAddons = computed(() => {
  if (!addonStatus.value) return false;
  const s = addonStatus.value;
  return (s.vscode_available && !s.vscode_installed) ||
         (s.jetbrains_available && !s.jetbrains_installed) ||
         !s.skill_installed;
});
```

**Step 3: Update `SetupWizard.vue` to add JetBrains section**

Add `jetbrains_available`, `jetbrains_installed`, `jetbrains_ides` to the `AddonStatus` interface in SetupWizard.vue.

Add a new checkbox item in the template (after the VS Code section, before Claude Code Skill) for JetBrains IDE Plugin installation with:
- Show detected IDEs list
- Checkbox to select/deselect
- Install button calls `invoke('install_jetbrains_plugin', { pluginsPath: ide.plugins_path })` for each selected IDE
- Show "需重啟 IDE" message after install

**Step 4: Verify frontend builds**

Run: `npm run build`
Expected: No errors

**Step 5: Commit**

```bash
git add src/App.vue src/components/SetupWizard.vue
git commit -m "feat(ui): add JetBrains IDE support to SetupWizard"
```

---

## Task 12: Bundle Plugin ZIP into Tauri Resources

**Files:**
- Create: `src-tauri/resources/bundled/` directory (if not exists)
- Modify: build process to copy plugin zip

**Step 1: Create bundled resources directory**

```bash
mkdir -p src-tauri/resources/bundled
```

**Step 2: Build the IntelliJ plugin and copy to bundled**

```bash
cd intellij-plugin && ./gradlew buildPlugin
cp build/distributions/tsunu-alive-connector-0.1.0.zip ../src-tauri/resources/bundled/tsunu-alive-connector.zip
```

**Step 3: Verify Tauri can access the resource**

The `tauri.conf.json` already has `"resources": ["resources/bundled/**/*"]`, so the zip will be included in the bundle automatically.

**Step 4: Commit**

```bash
git add src-tauri/resources/bundled/tsunu-alive-connector.zip
git commit -m "build: bundle IntelliJ plugin zip for auto-install"
```

---

## Task 13: Final Integration Test

**Step 1: Build everything**

```bash
# Build IntelliJ plugin
cd intellij-plugin && ./gradlew buildPlugin

# Build Tauri app
cd .. && npm run tauri build
```

**Step 2: Test auto-install flow**

1. Launch Tsunu Alive
2. Setup Wizard should detect JetBrains IDEs
3. Install plugin to one of the detected IDEs
4. Restart that IDE
5. Verify plugin appears in IDE's plugin list
6. Verify status bar shows "Tsunu Alive: 未連接"

**Step 3: Test WebSocket connection**

1. Start Tsunu Alive app (IDE Server runs on port 19750)
2. Open a project in PyCharm/IntelliJ with the plugin installed
3. Verify status bar changes to "Tsunu Alive: 已連接"
4. Select some code → verify selection appears in Tsunu Alive
5. Switch files → verify context updates in Tsunu Alive

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete IntelliJ Platform Plugin with auto-install support"
```
