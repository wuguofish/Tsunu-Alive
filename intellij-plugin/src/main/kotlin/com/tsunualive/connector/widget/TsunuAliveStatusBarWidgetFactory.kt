package com.tsunualive.connector.widget

import com.intellij.openapi.project.Project
import com.intellij.openapi.util.Disposer
import com.intellij.openapi.wm.StatusBar
import com.intellij.openapi.wm.StatusBarWidget
import com.intellij.openapi.wm.StatusBarWidgetFactory
import com.tsunualive.connector.service.ConnectionState
import com.tsunualive.connector.service.TsunuAliveService
import com.intellij.util.Consumer
import java.awt.event.MouseEvent

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
