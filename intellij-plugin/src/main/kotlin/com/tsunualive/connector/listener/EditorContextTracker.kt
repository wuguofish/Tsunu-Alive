package com.tsunualive.connector.listener

import com.intellij.openapi.Disposable
import com.intellij.openapi.components.Service
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.event.SelectionEvent
import com.intellij.openapi.editor.event.SelectionListener
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerEvent
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.tsunualive.connector.service.SelectionRange
import com.tsunualive.connector.service.TsunuAliveService

@Service(Service.Level.PROJECT)
class EditorContextTracker(private val project: Project) : Disposable {

    private val selectionListener = object : SelectionListener {
        override fun selectionChanged(e: SelectionEvent) {
            val editor = e.editor
            val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
            if (!belongsToProject(file)) return

            val selectedText = editor.selectionModel.selectedText ?: ""
            val selection = getSelectionRange(editor)
            TsunuAliveService.getInstance(project).sendSelectionChanged(selectedText, selection)
        }
    }

    fun start() {
        EditorFactory.getInstance().eventMulticaster.addSelectionListener(selectionListener, this)

        project.messageBus.connect(this)
            .subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
                override fun selectionChanged(event: FileEditorManagerEvent) {
                    val file = event.newFile ?: return
                    val editor = FileEditorManager.getInstance(project).selectedTextEditor ?: return
                    sendFullContext(editor, file)
                }
            })
    }

    fun sendCurrentContext() {
        val editor = FileEditorManager.getInstance(project).selectedTextEditor ?: return
        val file = FileDocumentManager.getInstance().getFile(editor.document) ?: return
        sendFullContext(editor, file)
    }

    private fun sendFullContext(editor: Editor, file: VirtualFile) {
        val document = editor.document
        val selectedText = editor.selectionModel.selectedText ?: ""
        val selection = getSelectionRange(editor)
        val fileContent = document.text
        val languageId = file.fileType.name

        TsunuAliveService.getInstance(project).sendContextUpdate(
            filePath = file.path,
            selectedText = selectedText,
            selection = selection,
            fileContent = fileContent,
            languageId = languageId
        )
    }

    private fun getSelectionRange(editor: Editor): SelectionRange {
        val selModel = editor.selectionModel
        val doc = editor.document
        val startOffset = selModel.selectionStart
        val endOffset = selModel.selectionEnd

        val startLine = doc.getLineNumber(startOffset)
        val endLine = doc.getLineNumber(endOffset)
        val startChar = startOffset - doc.getLineStartOffset(startLine)
        val endChar = endOffset - doc.getLineStartOffset(endLine)

        return SelectionRange(startLine, startChar, endLine, endChar)
    }

    private fun belongsToProject(file: VirtualFile): Boolean {
        val basePath = project.basePath ?: return true
        return file.path.startsWith(basePath)
    }

    override fun dispose() {}

    companion object {
        fun getInstance(project: Project): EditorContextTracker =
            project.getService(EditorContextTracker::class.java)
    }
}
