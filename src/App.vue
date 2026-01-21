<script setup lang="ts">
import { ref, nextTick, computed, onMounted, onUnmounted } from "vue";
import { marked, Renderer } from "marked";
import hljs from "highlight.js";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import ToolIndicator from "./components/ToolIndicator.vue";
import PermissionDialog from "./components/PermissionDialog.vue";

// Claude 事件類型定義
interface ClaudeEvent {
  event_type: 'Init' | 'Text' | 'ToolUse' | 'PermissionDenied' | 'ToolResult' | 'Complete' | 'Error' | 'Connected';
  session_id?: string;
  model?: string;
  text?: string;
  is_complete?: boolean;
  tool_name?: string;
  tool_id?: string;
  input?: Record<string, unknown>;
  result?: string;
  is_error?: boolean;
  cost_usd?: number;
  message?: string;
}

// 待確認的權限請求
interface PendingPermission {
  toolName: string;
  toolId: string;
  input: Record<string, unknown>;
}

// 工具使用項目
interface ToolUseItem {
  id: string;
  type: string;
  name: string;
  input: Record<string, unknown>;
  result?: string;
  isCancelled?: boolean;
  userResponse?: string;  // 使用者拒絕或自訂的回應
}

// 目前的 session ID
const sessionId = ref<string | null>(null);

// 目前使用的 model
const currentModel = ref('');

// 累積的回應文字
const streamingText = ref('');

// 目前的工具使用
const currentToolUses = ref<ToolUseItem[]>([]);

// 事件監聽取消函數
let unlistenClaude: UnlistenFn | null = null;

// 自訂 renderer 來處理程式碼高亮
const renderer = new Renderer();
renderer.code = function ({ text, lang }: { text: string; lang?: string }) {
  const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
  const highlighted = hljs.highlight(text, { language }).value;
  return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
};

// 設定 marked
marked.setOptions({
  breaks: true, // 支援換行
});
marked.use({ renderer });

// 將 Markdown 轉換為 HTML
function renderMarkdown(content: string): string {
  return marked.parse(content) as string;
}

// 訊息類型
interface Message {
  role: 'user' | 'assistant';
  content: string;
}

// 阿宇的表情狀態
type AvatarState = 'idle' | 'processing' | 'complete' | 'waiting';

// Avatar 圖片對應
const avatarImages: Record<AvatarState, string> = {
  idle: '/tsunu-3.png',       // 待機：側臉望向遠方，靜靜陪伴
  processing: '/tsunu-4.png', // 處理中：視線向下，專注盯螢幕
  complete: '/tsunu-1.png',   // 完成：溫柔微笑，開心看著你
  waiting: '/tsunu-2.png',    // 等待選擇：側臉淺笑，「你覺得呢？」
};

// 當前表情狀態
const avatarState = ref<AvatarState>('idle');

// 計算當前應顯示的 Avatar
const currentAvatar = computed(() => avatarImages[avatarState.value]);

// 對話歷史
const messages = ref<Message[]>([
  {
    role: 'assistant',
    content: '欸，你來啦～有什麼需要幫忙的嗎？ *推眼鏡*'
  }
]);

// 輸入框內容
const userInput = ref('');

// 是否正在等待回應
const isLoading = ref(false);

// 忙碌狀態文字
const busyStatus = ref('Perusing...');

// 編輯模式
type EditMode = 'ask' | 'auto' | 'plan';
const editMode = ref<EditMode>('ask');
const editModeLabels: Record<EditMode, string> = {
  ask: 'Ask before edits',
  auto: 'Edit automatically',
  plan: 'Plan Mode'
};

// 切換編輯模式
function cycleEditMode() {
  const modes: EditMode[] = ['ask', 'auto', 'plan'];
  const currentIndex = modes.indexOf(editMode.value);
  editMode.value = modes[(currentIndex + 1) % modes.length];
}

// 目前檔案（測試用）
const currentFile = ref('App.vue');

// Context 用量（測試用，0-100）
const contextUsage = ref(87);

// 斜線選單顯示狀態
const showSlashMenu = ref(false);

// 待確認的權限請求
const pendingPermission = ref<PendingPermission | null>(null);

// 對話區域的參考，用於自動滾動
const chatContainer = ref<HTMLElement | null>(null);

// 滾動到最底部
async function scrollToBottom() {
  await nextTick();
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
  }
}

// 加入工具使用記錄（避免重複）
function addToolUse(toolName: string, toolId: string, input: Record<string, unknown>) {
  // 用 tool_id 檢查是否已存在
  const existingById = currentToolUses.value.find(t => t.id === toolId);
  if (!existingById) {
    // 額外檢查：如果是同類型工具且描述相同，也視為重複（stream 重複發送的情況）
    const desc = (input?.description as string) || '';
    const existingByContent = currentToolUses.value.find(t =>
      t.type === toolName &&
      (t.input?.description as string || '') === desc
    );
    if (!existingByContent) {
      currentToolUses.value.push({
        id: toolId,
        type: toolName,
        name: toolName,
        input: input
      });
    }
  }
}

// 處理 Claude 事件
function handleClaudeEvent(event: ClaudeEvent) {
  console.log('Claude event:', event);

  switch (event.event_type) {
    case 'Init':
      // 初始化完成
      if (event.session_id) {
        sessionId.value = event.session_id;
      }
      if (event.model) {
        currentModel.value = event.model;
      }
      busyStatus.value = 'Thinking...';
      break;

    case 'Text':
      // 收到文字串流
      if (event.text) {
        streamingText.value += event.text;
        // 檢查是否需要加入新的助手訊息
        const lastMsg = messages.value[messages.value.length - 1];
        if (lastMsg && lastMsg.role === 'assistant') {
          // 更新現有的助手訊息
          lastMsg.content = streamingText.value;
        } else {
          // 第一次收到文字，加入新的助手訊息
          messages.value.push({
            role: 'assistant',
            content: streamingText.value
          });
        }
        scrollToBottom();
      }
      break;

    case 'ToolUse':
      // 工具使用（不需要權限確認的工具）
      if (event.tool_name && event.tool_id) {
        busyStatus.value = `Using ${event.tool_name}...`;
        addToolUse(event.tool_name, event.tool_id, event.input || {});
      }
      break;

    case 'PermissionDenied':
      // 權限被拒絕（由 Claude CLI 的 default 權限模式回報）
      if (event.tool_name && event.tool_id) {
        // 更新工具使用記錄為已取消
        const tool = currentToolUses.value.find(t => t.id === event.tool_id);
        if (tool) {
          tool.isCancelled = true;
          tool.userResponse = 'Permission denied by CLI';
        }
      }
      break;

    case 'Connected':
      // Claude CLI 已連線
      busyStatus.value = 'Connected';
      break;

    case 'ToolResult':
      // 工具結果
      if (event.tool_id) {
        const tool = currentToolUses.value.find(t => t.id === event.tool_id);
        if (tool) {
          tool.result = event.result;
          // 如果是錯誤結果，標記為取消
          if (event.is_error) {
            tool.isCancelled = true;
          }
        }
      }
      break;

    case 'Complete':
      // 完成
      isLoading.value = false;
      avatarState.value = 'complete';
      streamingText.value = '';
      currentToolUses.value = [];

      // 3 秒後回到待機表情
      setTimeout(() => {
        avatarState.value = 'idle';
      }, 3000);
      break;

    case 'Error':
      // 錯誤
      isLoading.value = false;
      avatarState.value = 'idle';
      if (event.message) {
        messages.value.push({
          role: 'assistant',
          content: `*皺眉* 抱歉，出了點問題：${event.message}`
        });
      }
      break;
  }
}

// 設定事件監聽
async function setupEventListeners() {
  unlistenClaude = await listen<ClaudeEvent>('claude-event', (event) => {
    handleClaudeEvent(event.payload);
  });
}

// 元件掛載時設定監聽
onMounted(() => {
  setupEventListeners();
});

// 元件卸載時清理
onUnmounted(() => {
  if (unlistenClaude) {
    unlistenClaude();
  }
});

// 送出訊息
async function sendMessage() {
  const content = userInput.value.trim();
  if (!content || isLoading.value) return;

  // 加入使用者訊息
  messages.value.push({
    role: 'user',
    content: content
  });
  userInput.value = '';
  await scrollToBottom();

  // 開始載入狀態
  isLoading.value = true;
  avatarState.value = 'processing';
  busyStatus.value = 'Connecting...';
  streamingText.value = '';
  // 注意：不要在這裡加入空的助手訊息，等收到第一個文字時再加入

  try {
    // 呼叫 Rust 端送出訊息給 Claude CLI
    await invoke('send_to_claude', {
      prompt: content,
      workingDir: null  // 使用預設目錄
    });
  } catch (error) {
    console.error('Failed to send to Claude:', error);
    isLoading.value = false;
    avatarState.value = 'idle';

    // 加入錯誤訊息
    messages.value.push({
      role: 'assistant',
      content: `*皺眉* 連接 Claude 時發生錯誤：${error}`
    });
  }
}

// 按 Enter 送出（Shift+Enter 換行）
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

// 處理權限確認回應
// 注意：目前使用 -p 模式時無法進行互動式權限確認
// 此功能保留供未來實作互動模式使用
async function handlePermissionResponse(response: 'yes' | 'yes-all' | 'no' | 'custom', customMessage?: string) {
  console.log('Permission response:', response, customMessage);

  if (!pendingPermission.value) return;

  const toolId = pendingPermission.value.toolId;
  let userResponse: string | undefined;

  switch (response) {
    case 'no':
      userResponse = 'Denied by user';
      break;
    case 'custom':
      userResponse = customMessage;
      break;
  }

  // 如果是拒絕或自訂回應，更新工具使用記錄
  if (response === 'no' || response === 'custom') {
    const tool = currentToolUses.value.find(t => t.id === toolId);
    if (tool) {
      tool.isCancelled = true;
      tool.userResponse = userResponse;
    }
  }

  // 清除待確認的權限
  pendingPermission.value = null;

  // 恢復處理狀態
  avatarState.value = 'processing';
  busyStatus.value = 'Processing...';

  // TODO: 當實作互動模式時，這裡需要發送回應給 Claude CLI
  // 目前 -p 模式下權限確認由 CLI 自動處理
  console.log('Interactive permission response not yet implemented');
}

// 中斷請求
async function interruptRequest() {
  console.log('Interrupt request');

  // 清除待確認的權限
  pendingPermission.value = null;

  // 呼叫 Rust 端中斷 Claude
  try {
    await invoke('interrupt_claude');
  } catch (error) {
    console.error('Failed to interrupt Claude:', error);
  }

  isLoading.value = false;
  avatarState.value = 'idle';
  streamingText.value = '';
  currentToolUses.value = [];
}
</script>

<template>
  <div class="app-container">
    <!-- 標題列 -->
    <header class="app-header">
      <h1>Tsunu Alive</h1>
      <span class="subtitle">阿宇陪你寫程式</span>
    </header>

    <!-- 主要內容區 -->
    <div class="main-content">
      <!-- 左側：對話區域 -->
      <div class="chat-section">
        <!-- 對話訊息 -->
        <div class="chat-container" ref="chatContainer">
          <div
            v-for="(msg, index) in messages"
            :key="index"
            :class="['message', msg.role]"
          >
            <div
              class="message-content markdown-body"
              v-html="renderMarkdown(msg.content)"
            ></div>
          </div>

          <!-- 工具使用提示會在這裡動態顯示 -->
          <template v-for="tool in currentToolUses" :key="tool.id">
            <ToolIndicator
              :type="tool.type as any"
              :path="(tool.input?.file_path as string) || (tool.input?.path as string)"
              :description="tool.input?.description as string"
              :input="tool.input?.command as string"
              :output="tool.result"
              :isRunning="!tool.result && !tool.isCancelled"
              :isCancelled="tool.isCancelled"
              :userResponse="tool.userResponse"
            />
          </template>

          <!-- 權限確認對話框 -->
          <PermissionDialog
            v-if="pendingPermission"
            :action="pendingPermission.toolName"
            :target="(pendingPermission.input?.file_path as string) || (pendingPermission.input?.path as string) || (pendingPermission.input?.description as string) || ''"
            :summary="(pendingPermission.input?.description as string)"
            :preview="(pendingPermission.input?.command as string)"
            @respond="handlePermissionResponse"
          />

          <!-- 載入中指示器 -->
          <div v-if="isLoading && !pendingPermission" class="message assistant loading">
            <div class="message-content">
              <span class="typing-indicator">
                <span></span>
                <span></span>
                <span></span>
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- 右側：阿宇 Avatar -->
      <div class="avatar-panel">
        <div class="avatar-container">
          <img :src="currentAvatar" alt="阿宇" class="avatar" />
        </div>
        <div class="avatar-name">阿宇</div>
      </div>
    </div>

    <!-- 輸入區域（底部 100% 寬度）-->
    <div class="input-area">
      <!-- 忙碌狀態指示器 -->
      <div v-if="isLoading" class="busy-indicator">
        <span class="busy-dot"></span>
        <span class="busy-text">{{ busyStatus }}</span>
      </div>

      <!-- 輸入框 -->
      <div class="input-wrapper">
        <textarea
          v-model="userInput"
          @keydown="handleKeydown"
          placeholder="Queue another message..."
          :disabled="isLoading"
          rows="2"
        ></textarea>
      </div>

      <!-- 狀態列 -->
      <div class="status-bar">
        <!-- 左側按鈕 -->
        <div class="status-left">
          <button class="status-btn edit-mode" @click="cycleEditMode" :title="editModeLabels[editMode]">
            <span class="mode-icon">▶</span>
            <span class="mode-label">{{ editModeLabels[editMode] }}</span>
          </button>
          <button class="status-btn current-file" :title="currentFile">
            <span class="file-icon">&lt;/&gt;</span>
            <span class="file-name">{{ currentFile }}</span>
          </button>
          <button class="status-btn context-usage" :title="`Context: ${contextUsage}% used`">
            <span class="usage-icon">◐</span>
            <span class="usage-text">{{ contextUsage }}% used</span>
          </button>
        </div>

        <!-- 右側按鈕 -->
        <div class="status-right">
          <button class="status-btn attach-btn" title="Attach files">
            <span class="attach-icon">📎</span>
          </button>
          <button class="status-btn slash-btn" @click="showSlashMenu = !showSlashMenu" title="Commands">
            <span class="slash-icon">/</span>
          </button>
          <button
            v-if="!isLoading"
            class="status-btn send-btn"
            @click="sendMessage"
            :disabled="!userInput.trim()"
            title="Send (Enter)"
          >
            <span class="send-icon">⏎</span>
          </button>
          <button
            v-else
            class="status-btn interrupt-btn"
            @click="interruptRequest"
            title="Interrupt (Esc)"
          >
            <span class="interrupt-icon">■</span>
          </button>
        </div>
      </div>

      <!-- 斜線選單 -->
      <div v-if="showSlashMenu" class="slash-menu">
        <input type="text" class="slash-search" placeholder="Filter actions..." />
        <div class="slash-section">
          <div class="slash-section-title">Model</div>
          <div class="slash-item">Switch model... <span class="slash-hint">claude-opus-4-5-20251101</span></div>
          <div class="slash-item">Thinking <span class="slash-toggle">○</span></div>
          <div class="slash-item">Account & usage...</div>
        </div>
        <div class="slash-section">
          <div class="slash-section-title">Slash Commands</div>
          <div class="slash-item">/compact</div>
          <div class="slash-item">/context</div>
          <div class="slash-item">/cost</div>
        </div>
        <div class="slash-section">
          <div class="slash-section-title">Settings</div>
          <div class="slash-item">Switch account</div>
          <div class="slash-item">General config...</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
/* 全域樣式重設 */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  /* 藍色系配色 */
  --primary-color: #4a90d9;
  --primary-dark: #357abd;
  --primary-light: #6ba3e0;
  --bg-color: #1a1a2e;
  --bg-secondary: #16213e;
  --text-color: #e8e8e8;
  --text-muted: #a0a0a0;
  --user-bubble: #4a90d9;
  --assistant-bubble: #2d3748;
  --border-color: #3a3a5c;

  font-family: 'Segoe UI', 'Microsoft JhengHei', sans-serif;
  font-size: 15px;
  line-height: 1.6;
}

body {
  background-color: var(--bg-color);
  color: var(--text-color);
  overflow: hidden;
}

#app {
  height: 100vh;
  width: 100vw;
}

/* Markdown 內容樣式 */
.markdown-body {
  line-height: 1.6;
}

.markdown-body h1,
.markdown-body h2,
.markdown-body h3,
.markdown-body h4 {
  margin-top: 1em;
  margin-bottom: 0.5em;
  font-weight: 600;
}

.markdown-body h3 {
  font-size: 1.1rem;
}

.markdown-body p {
  margin: 0;
}

.markdown-body p + p {
  margin-top: 0.5em;
}

.markdown-body ul,
.markdown-body ol {
  margin: 0.5em 0;
  padding-left: 1.5em;
}

.markdown-body li {
  margin: 0.25em 0;
}

.markdown-body code {
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 0.9em;
}

.markdown-body code:not(.hljs) {
  background-color: rgba(255, 255, 255, 0.1);
  padding: 0.15em 0.4em;
  border-radius: 4px;
}

.markdown-body pre {
  margin: 0.75em 0;
  border-radius: 8px;
  overflow-x: auto;
}

.markdown-body pre code {
  display: block;
  padding: 12px 16px;
  font-size: 0.85rem;
  line-height: 1.5;
}

.markdown-body blockquote {
  margin: 0.75em 0;
  padding: 0.5em 1em;
  border-left: 3px solid var(--primary-color);
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 0 4px 4px 0;
}

.markdown-body blockquote p {
  margin: 0;
}

.markdown-body strong {
  font-weight: 600;
}

.markdown-body em {
  font-style: italic;
}

.markdown-body a {
  color: var(--primary-light);
  text-decoration: underline;
}

.markdown-body a:hover {
  color: var(--primary-color);
}
</style>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  max-width: 100%;
  background-color: var(--bg-color);
}

/* 標題列 */
.app-header {
  padding: 12px 20px;
  background-color: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.app-header h1 {
  font-size: 1.3rem;
  font-weight: 600;
  color: var(--primary-light);
}

.subtitle {
  font-size: 0.85rem;
  color: var(--text-muted);
}

/* 主要內容區：左右分割 */
.main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 左側：對話區域 */
.chat-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* 對話訊息容器 */
.chat-container {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.message {
  display: flex;
  max-width: 85%;
}

.message.user {
  align-self: flex-start;
}

.message.assistant {
  align-self: flex-end;
}

.message-content {
  padding: 12px 16px;
  border-radius: 16px;
  word-break: break-word;
}

.message.user .message-content {
  background-color: var(--user-bubble);
  color: white;
  border-bottom-left-radius: 4px;
}

.message.assistant .message-content {
  background-color: var(--assistant-bubble);
  color: var(--text-color);
  border-bottom-right-radius: 4px;
}

/* 載入中動畫 */
.typing-indicator {
  display: flex;
  gap: 4px;
  padding: 4px 0;
}

.typing-indicator span {
  width: 8px;
  height: 8px;
  background-color: var(--text-muted);
  border-radius: 50%;
  animation: typing 1.4s infinite ease-in-out;
}

.typing-indicator span:nth-child(2) {
  animation-delay: 0.2s;
}

.typing-indicator span:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes typing {
  0%, 60%, 100% {
    transform: translateY(0);
    opacity: 0.4;
  }
  30% {
    transform: translateY(-8px);
    opacity: 1;
  }
}

/* 輸入區域 */
.input-area {
  padding: 12px 20px;
  background-color: var(--bg-secondary);
  border-top: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  gap: 8px;
  position: relative;
}

/* 忙碌狀態指示器 */
.busy-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.busy-dot {
  width: 8px;
  height: 8px;
  background-color: #f39c12;
  border-radius: 50%;
  animation: busy-pulse 1.5s infinite;
}

@keyframes busy-pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.busy-text {
  color: var(--text-muted);
  font-size: 0.85rem;
  font-style: italic;
}

/* 輸入框包裝 */
.input-wrapper {
  width: 100%;
}

textarea {
  width: 100%;
  padding: 12px 16px;
  border: 1px solid var(--border-color);
  border-radius: 12px;
  background-color: var(--bg-color);
  color: var(--text-color);
  font-family: inherit;
  font-size: 0.95rem;
  resize: none;
  outline: none;
  transition: border-color 0.2s;
}

textarea:focus {
  border-color: var(--primary-color);
}

textarea::placeholder {
  color: var(--text-muted);
}

textarea:disabled {
  opacity: 0.6;
}

/* 狀態列 */
.status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 0;
}

.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 4px;
}

.status-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--text-muted);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.status-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: var(--text-color);
}

.status-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.mode-icon,
.file-icon,
.usage-icon {
  font-size: 0.75rem;
}

.slash-icon {
  font-size: 1rem;
  font-weight: 600;
}

.send-btn {
  background-color: var(--primary-color);
  color: white;
  padding: 6px 10px;
  border-radius: 6px;
}

.send-btn:hover:not(:disabled) {
  background-color: var(--primary-dark);
}

.interrupt-btn {
  background-color: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 10px;
  border-radius: 6px;
}

.interrupt-btn:hover {
  background-color: rgba(231, 76, 60, 0.2);
  border-color: #e74c3c;
}

.interrupt-icon {
  font-size: 0.9rem;
}

/* 斜線選單 */
.slash-menu {
  position: absolute;
  bottom: 100%;
  right: 20px;
  width: 320px;
  max-height: 400px;
  overflow-y: auto;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 100;
}

.slash-search {
  width: 100%;
  padding: 12px;
  background: transparent;
  border: none;
  border-bottom: 1px solid var(--border-color);
  color: var(--text-color);
  font-size: 0.9rem;
  outline: none;
}

.slash-search::placeholder {
  color: var(--text-muted);
}

.slash-section {
  padding: 8px 0;
}

.slash-section-title {
  padding: 4px 12px;
  font-size: 0.75rem;
  color: var(--text-muted);
  text-transform: uppercase;
}

.slash-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  color: var(--text-color);
  font-size: 0.9rem;
  cursor: pointer;
  transition: background-color 0.2s;
}

.slash-item:hover {
  background-color: var(--primary-color);
}

.slash-hint {
  color: var(--text-muted);
  font-size: 0.8rem;
}

.slash-toggle {
  font-size: 1.2rem;
}

/* 右側：Avatar 面板 */
.avatar-panel {
  width: 384px;
  background-color: var(--bg-secondary);
  border-left: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  padding: 0px 0px 20px 0px;
}

.avatar-container {
  width: 100%;
  overflow: hidden;
  border-radius: 0px;
  border: 0px solid var(--primary-color);
  background-color: var(--bg-color);  
}

.avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top center;
  transition: opacity 0.3s ease;
}

.avatar-name {
  margin-top: 12px;
  font-size: 1rem;
  font-weight: 500;
  color: var(--primary-light);
}

/* 滾動條樣式 */
.chat-container::-webkit-scrollbar {
  width: 8px;
}

.chat-container::-webkit-scrollbar-track {
  background: var(--bg-secondary);
}

.chat-container::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 4px;
}

.chat-container::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}
</style>
