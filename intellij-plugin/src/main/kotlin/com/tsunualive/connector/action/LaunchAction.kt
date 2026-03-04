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
        NotificationGroupManager.getInstance()
            .getNotificationGroup("Tsunu Alive Notifications")
            .createNotification(content, type)
            .notify(project)
    }
}
