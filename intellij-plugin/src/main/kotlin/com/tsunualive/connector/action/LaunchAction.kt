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
        // 1. 優先使用設定中的路徑
        val configPath = TsunuAliveSettings.getInstance().executablePath
        if (configPath.isNotBlank() && File(configPath).exists()) {
            return configPath
        }

        val home = System.getProperty("user.home") ?: return null
        val isWindows = System.getProperty("os.name").lowercase().contains("win")
        val isMac = System.getProperty("os.name").lowercase().contains("mac")

        // 2. Windows: 查詢 Registry 取得安裝路徑（最可靠）
        if (isWindows) {
            val registryPath = findFromWindowsRegistry()
            if (registryPath != null) return registryPath
        }

        // 3. 嘗試常見路徑候選
        val candidates = mutableListOf<String>()

        if (isWindows) {
            val localAppData = System.getenv("LOCALAPPDATA") ?: "$home\\AppData\\Local"
            val programFiles = System.getenv("ProgramFiles") ?: "C:\\Program Files"

            // Tauri NSIS 安裝器的預設路徑（productName 帶空格）
            candidates.add("$localAppData\\Tsunu Alive\\Tsunu Alive.exe")
            candidates.add("$localAppData\\Tsunu Alive\\tsunu_alive.exe")
            candidates.add("$programFiles\\Tsunu Alive\\Tsunu Alive.exe")
            candidates.add("$programFiles\\Tsunu Alive\\tsunu_alive.exe")
            // 不帶空格的變體
            candidates.add("$localAppData\\tsunu-alive\\tsunu_alive.exe")
            candidates.add("$localAppData\\tsunu-alive\\Tsunu Alive.exe")

            // 掃描安裝目錄下所有 exe（處理 _temp 等命名變體）
            val installDirs = listOf(
                "$localAppData\\Tsunu Alive",
                "$localAppData\\tsunu-alive",
                "$programFiles\\Tsunu Alive",
            )
            for (dir in installDirs) {
                val dirFile = File(dir)
                if (dirFile.isDirectory) {
                    dirFile.listFiles()?.filter {
                        it.name.lowercase().contains("tsunu") && it.name.endsWith(".exe") && it.name != "uninstall.exe"
                    }?.firstOrNull()?.let {
                        candidates.add(0, it.absolutePath) // 放到最前面
                    }
                }
            }
        } else if (isMac) {
            candidates.add("/Applications/Tsunu Alive.app/Contents/MacOS/Tsunu Alive")
            candidates.add("/Applications/Tsunu Alive.app/Contents/MacOS/tsunu_alive")
            candidates.add("$home/Applications/Tsunu Alive.app/Contents/MacOS/Tsunu Alive")
            candidates.add("$home/Applications/Tsunu Alive.app/Contents/MacOS/tsunu_alive")
            candidates.add("$home/.local/bin/tsunu_alive")
            candidates.add("/usr/local/bin/tsunu_alive")
        } else {
            candidates.add("$home/.local/bin/tsunu_alive")
            candidates.add("/usr/local/bin/tsunu_alive")
            candidates.add("/usr/bin/tsunu_alive")
        }

        return candidates.firstOrNull { File(it).exists() }
    }

    /**
     * 從 Windows Registry 查詢 Tauri NSIS 安裝器註冊的安裝路徑
     */
    private fun findFromWindowsRegistry(): String? {
        try {
            val regKeys = listOf(
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Tsunu Alive",
                "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Tsunu Alive",
            )

            for (regKey in regKeys) {
                val process = ProcessBuilder("reg", "query", regKey, "/v", "InstallLocation")
                    .redirectErrorStream(true)
                    .start()
                val output = process.inputStream.bufferedReader().readText()
                process.waitFor()

                if (process.exitValue() == 0 && output.contains("InstallLocation")) {
                    // 解析 "InstallLocation    REG_SZ    "C:\path\to\app""
                    val match = Regex("""InstallLocation\s+REG_SZ\s+("?)(.+?)\1\s*$""", RegexOption.MULTILINE)
                        .find(output)
                    val installDir = match?.groupValues?.get(2)?.trim()
                    if (installDir != null) {
                        val dir = File(installDir)
                        if (dir.isDirectory) {
                            // 找到安裝目錄後，掃描其中的主程式 exe
                            dir.listFiles()?.filter {
                                it.name.lowercase().contains("tsunu") && it.name.endsWith(".exe") && it.name != "uninstall.exe"
                            }?.firstOrNull()?.let {
                                return it.absolutePath
                            }
                        }
                    }
                }
            }
        } catch (e: Exception) {
            log.warn("Failed to query Windows registry", e)
        }
        return null
    }

    private fun notify(project: com.intellij.openapi.project.Project?, content: String, type: NotificationType) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup("Tsunu Alive Notifications")
            .createNotification(content, type)
            .notify(project)
    }
}
