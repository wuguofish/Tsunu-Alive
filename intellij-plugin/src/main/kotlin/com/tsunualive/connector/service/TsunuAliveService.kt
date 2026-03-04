package com.tsunualive.connector.service

import com.google.gson.JsonObject
import com.intellij.openapi.Disposable
import com.intellij.openapi.application.ApplicationInfo
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.util.concurrency.AppExecutorUtil
import com.tsunualive.connector.settings.TsunuAliveSettings
import java.net.URI
import java.net.http.HttpClient
import java.net.http.WebSocket
import java.util.concurrent.CompletionStage
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
    private val httpClient = HttpClient.newHttpClient()
    private val messageId = AtomicInteger(0)
    private val wsRef = AtomicReference<WebSocket?>(null)
    private var reconnectFuture: ScheduledFuture<*>? = null

    var connectionState: ConnectionState = ConnectionState.DISCONNECTED
        private set

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

        val listener = object : WebSocket.Listener {
            private val buffer = StringBuilder()

            override fun onOpen(webSocket: WebSocket) {
                log.info("Connected to Tsunu Alive")
                wsRef.set(webSocket)
                connectionState = ConnectionState.CONNECTED
                notifyStateChanged()
                cancelReconnect()
                sendHello()
                webSocket.request(1)
            }

            override fun onText(webSocket: WebSocket, data: CharSequence, last: Boolean): CompletionStage<*>? {
                buffer.append(data)
                if (last) {
                    val text = buffer.toString()
                    buffer.setLength(0)
                    try {
                        val msg = com.google.gson.JsonParser.parseString(text).asJsonObject
                        if (msg.has("method") && msg.get("method").asString == "server/hello") {
                            val params = msg.getAsJsonObject("params")
                            log.info("Server: ${params?.get("name")?.asString} v${params?.get("version")?.asString}")
                        }
                    } catch (e: Exception) {
                        log.warn("Failed to parse message: $text", e)
                    }
                }
                webSocket.request(1)
                return null
            }

            override fun onClose(webSocket: WebSocket, statusCode: Int, reason: String): CompletionStage<*>? {
                log.info("Connection closed: $reason")
                wsRef.set(null)
                connectionState = ConnectionState.DISCONNECTED
                notifyStateChanged()
                scheduleReconnect()
                return null
            }

            override fun onError(webSocket: WebSocket, error: Throwable) {
                log.warn("Connection failed: ${error.message}")
                wsRef.set(null)
                connectionState = ConnectionState.ERROR
                notifyStateChanged()
                scheduleReconnect()
            }
        }

        try {
            httpClient.newWebSocketBuilder()
                .buildAsync(URI.create(url), listener)
        } catch (e: Exception) {
            log.error("Failed to initiate WebSocket connection", e)
            connectionState = ConnectionState.ERROR
            notifyStateChanged()
            scheduleReconnect()
        }
    }

    fun disconnect() {
        cancelReconnect()
        wsRef.getAndSet(null)?.sendClose(WebSocket.NORMAL_CLOSURE, "User disconnect")
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

        val params = JsonObject().apply {
            addProperty("name", ideName)
            addProperty("version", ideVersion)
            if (workspacePath != null) addProperty("workspacePath", workspacePath)
        }
        sendJsonRpc("client/hello", params, withId = true)
    }

    fun sendContextUpdate(
        filePath: String?,
        selectedText: String?,
        selection: SelectionRange?,
        fileContent: String?,
        languageId: String?
    ) {
        val params = JsonObject().apply {
            filePath?.let { addProperty("filePath", it) }
            selectedText?.let { addProperty("selectedText", it) }
            selection?.let { add("selection", it.toJson()) }
            fileContent?.let { addProperty("fileContent", it) }
            languageId?.let { addProperty("languageId", it) }
        }
        sendJsonRpc("context/update", params, withId = true)
    }

    fun sendSelectionChanged(selectedText: String, selection: SelectionRange) {
        val params = JsonObject().apply {
            addProperty("selectedText", selectedText)
            add("selection", selection.toJson())
        }
        sendJsonRpc("selection/changed", params, withId = false)
    }

    private fun sendJsonRpc(method: String, params: JsonObject, withId: Boolean) {
        val ws = wsRef.get() ?: return
        val msg = JsonObject().apply {
            addProperty("jsonrpc", "2.0")
            if (withId) addProperty("id", messageId.incrementAndGet())
            addProperty("method", method)
            add("params", params)
        }
        ws.sendText(msg.toString(), true)
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
    fun toJson(): JsonObject = JsonObject().apply {
        addProperty("start_line", startLine)
        addProperty("start_character", startCharacter)
        addProperty("end_line", endLine)
        addProperty("end_character", endCharacter)
    }
}
