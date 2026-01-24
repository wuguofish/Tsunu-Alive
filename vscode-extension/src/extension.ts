/**
 * Tsunu Alive VS Code Connector
 * 連接 VS Code 與 Tsunu Alive，分享編輯器 Context
 */

import * as vscode from 'vscode';
import WebSocket from 'ws';

// ============================================================================
// 類型定義
// ============================================================================

interface SelectionRange {
    start_line: number;
    start_character: number;
    end_line: number;
    end_character: number;
}

interface ContextUpdate {
    filePath?: string;
    selectedText?: string;
    selection?: SelectionRange;
    fileContent?: string;
    languageId?: string;
}

interface JsonRpcMessage {
    jsonrpc: string;
    id?: number;
    method: string;
    params?: Record<string, unknown>;
}

// ============================================================================
// 全域狀態
// ============================================================================

let ws: WebSocket | null = null;
let statusBarItem: vscode.StatusBarItem;
let reconnectTimer: NodeJS.Timeout | null = null;
let messageId = 0;

// ============================================================================
// 主要功能
// ============================================================================

export function activate(context: vscode.ExtensionContext) {
    console.log('Tsunu Alive Connector 啟動中...');

    // 建立狀態列項目
    statusBarItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100
    );
    statusBarItem.command = 'tsunu-alive.connect';
    updateStatusBar('disconnected');
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    // 註冊命令
    context.subscriptions.push(
        vscode.commands.registerCommand('tsunu-alive.connect', connect),
        vscode.commands.registerCommand('tsunu-alive.disconnect', disconnect),
        vscode.commands.registerCommand('tsunu-alive.sendContext', sendCurrentContext)
    );

    // 監聽編輯器事件
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(onEditorChange),
        vscode.window.onDidChangeTextEditorSelection(onSelectionChange)
    );

    // 自動連接
    const config = vscode.workspace.getConfiguration('tsunuAlive');
    if (config.get('autoConnect', true)) {
        connect();
    }
}

export function deactivate() {
    disconnect();
}

// ============================================================================
// WebSocket 連接管理
// ============================================================================

function connect() {
    if (ws && ws.readyState === WebSocket.OPEN) {
        vscode.window.showInformationMessage('已經連接到 Tsunu Alive');
        return;
    }

    const config = vscode.workspace.getConfiguration('tsunuAlive');
    const serverUrl = config.get('serverUrl', 'ws://127.0.0.1:19750');

    console.log(`連接到 ${serverUrl}...`);
    updateStatusBar('connecting');

    try {
        ws = new WebSocket(serverUrl);

        ws.on('open', () => {
            console.log('已連接到 Tsunu Alive');
            updateStatusBar('connected');
            vscode.window.showInformationMessage('🔗 已連接到 Tsunu Alive！');

            // 發送 hello（包含 workspace 路徑，用於過濾同專案的 IDE）
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            sendMessage({
                jsonrpc: '2.0',
                id: ++messageId,
                method: 'client/hello',
                params: {
                    name: 'VS Code',
                    version: vscode.version,
                    workspacePath: workspaceFolder?.uri.fsPath
                }
            });

            // 發送當前 context
            sendCurrentContext();

            // 清除重連計時器
            if (reconnectTimer) {
                clearTimeout(reconnectTimer);
                reconnectTimer = null;
            }
        });

        ws.on('message', (data: WebSocket.Data) => {
            try {
                const message = JSON.parse(data.toString());
                console.log('收到訊息:', message);

                // 處理 server hello
                if (message.method === 'server/hello') {
                    console.log(`Server: ${message.params?.name} v${message.params?.version}`);
                }
            } catch (e) {
                console.error('解析訊息失敗:', e);
            }
        });

        ws.on('close', () => {
            console.log('與 Tsunu Alive 的連接已關閉');
            updateStatusBar('disconnected');
            ws = null;

            // 自動重連
            scheduleReconnect();
        });

        ws.on('error', (error: Error) => {
            console.error('WebSocket 錯誤:', error.message);
            updateStatusBar('error');
        });

    } catch (error) {
        console.error('連接失敗:', error);
        updateStatusBar('error');
        scheduleReconnect();
    }
}

function disconnect() {
    if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
    }

    if (ws) {
        ws.close();
        ws = null;
        updateStatusBar('disconnected');
        vscode.window.showInformationMessage('已斷開與 Tsunu Alive 的連接');
    }
}

function scheduleReconnect() {
    if (reconnectTimer) return;

    reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        const config = vscode.workspace.getConfiguration('tsunuAlive');
        if (config.get('autoConnect', true)) {
            console.log('嘗試重新連接...');
            connect();
        }
    }, 5000); // 5 秒後重連
}

// ============================================================================
// 訊息發送
// ============================================================================

function sendMessage(message: JsonRpcMessage) {
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(message));
        console.log('發送訊息:', message.method);
    }
}

function sendContextUpdate(context: ContextUpdate) {
    sendMessage({
        jsonrpc: '2.0',
        id: ++messageId,
        method: 'context/update',
        params: context as Record<string, unknown>
    });
}

function sendSelectionChange(selectedText: string, selection: SelectionRange) {
    sendMessage({
        jsonrpc: '2.0',
        method: 'selection/changed',
        params: {
            selectedText,
            selection
        }
    });
}

// ============================================================================
// 編輯器事件處理
// ============================================================================

function sendCurrentContext() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        console.log('沒有活動的編輯器');
        return;
    }

    const document = editor.document;
    const selection = editor.selection;

    const context: ContextUpdate = {
        filePath: document.uri.fsPath,
        languageId: document.languageId,
        fileContent: document.getText(),
        selectedText: document.getText(selection),
        selection: {
            start_line: selection.start.line,
            start_character: selection.start.character,
            end_line: selection.end.line,
            end_character: selection.end.character
        }
    };

    sendContextUpdate(context);
    console.log('已發送 Context:', context.filePath);
}

function onEditorChange(editor: vscode.TextEditor | undefined) {
    if (!editor) return;

    const document = editor.document;

    // 發送完整 context
    sendContextUpdate({
        filePath: document.uri.fsPath,
        languageId: document.languageId,
        fileContent: document.getText()
    });
}

function onSelectionChange(event: vscode.TextEditorSelectionChangeEvent) {
    const editor = event.textEditor;
    const selection = editor.selection;
    const document = editor.document;

    // 只發送選取範圍（輕量）
    sendSelectionChange(
        document.getText(selection),
        {
            start_line: selection.start.line,
            start_character: selection.start.character,
            end_line: selection.end.line,
            end_character: selection.end.character
        }
    );
}

// ============================================================================
// 狀態列更新
// ============================================================================

function updateStatusBar(status: 'connected' | 'disconnected' | 'connecting' | 'error') {
    switch (status) {
        case 'connected':
            statusBarItem.text = '$(link) Tsunu Alive';
            statusBarItem.tooltip = '已連接到 Tsunu Alive（點擊斷開）';
            statusBarItem.command = 'tsunu-alive.disconnect';
            statusBarItem.backgroundColor = undefined;
            break;
        case 'disconnected':
            statusBarItem.text = '$(debug-disconnect) Tsunu Alive';
            statusBarItem.tooltip = '未連接（點擊連接）';
            statusBarItem.command = 'tsunu-alive.connect';
            statusBarItem.backgroundColor = undefined;
            break;
        case 'connecting':
            statusBarItem.text = '$(sync~spin) Tsunu Alive';
            statusBarItem.tooltip = '連接中...';
            statusBarItem.command = undefined;
            statusBarItem.backgroundColor = undefined;
            break;
        case 'error':
            statusBarItem.text = '$(error) Tsunu Alive';
            statusBarItem.tooltip = '連接錯誤（點擊重試）';
            statusBarItem.command = 'tsunu-alive.connect';
            statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
            break;
    }
}
