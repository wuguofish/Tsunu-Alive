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
