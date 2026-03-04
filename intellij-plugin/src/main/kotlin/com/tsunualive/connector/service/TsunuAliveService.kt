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
        .readTimeout(0, TimeUnit.SECONDS)
        .build()
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
