package com.tsunualive.connector.startup

import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity
import com.tsunualive.connector.listener.EditorContextTracker
import com.tsunualive.connector.service.TsunuAliveService
import com.tsunualive.connector.settings.TsunuAliveSettings

class TsunuAliveStartupActivity : ProjectActivity {

    override suspend fun execute(project: Project) {
        EditorContextTracker.getInstance(project).start()

        if (TsunuAliveSettings.getInstance().autoConnect) {
            TsunuAliveService.getInstance(project).connect()
        }
    }
}
