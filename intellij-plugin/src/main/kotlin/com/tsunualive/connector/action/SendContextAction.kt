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
