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
