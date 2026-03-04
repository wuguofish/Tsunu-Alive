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
