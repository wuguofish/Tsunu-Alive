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
  // Context 相關資訊（Complete 事件）
  total_tokens_in_conversation?: number;
  context_window_max?: number;
  context_window_used_percent?: number;
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

// 對話項目（文字或工具，按時間順序）
type ChatItem =
  | { type: 'text'; content: string }
  | { type: 'tool'; tool: ToolUseItem };

// 訊息類型（包含按時間順序的項目）
interface Message {
  role: 'user' | 'assistant';
  items: ChatItem[];  // 按時間順序的項目（文字和工具穿插）
}

// 目前的 session ID
const sessionId = ref<string | null>(null);

// 目前使用的 model
const currentModel = ref('');

// Session 白名單（這個 session 內允許的工具）
const sessionAllowedTools = ref<Set<string>>(new Set());

// 記住上一次的 prompt（用於權限確認後重新執行）
const lastPrompt = ref<string>('');

// 累積的回應文字
const streamingText = ref('');

// 目前的工具使用
const currentToolUses = ref<ToolUseItem[]>([]);

// 這次請求中所有被拒絕的工具（用於 "yes-all" 時一次加入白名單）
const deniedToolsThisRequest = ref<Set<string>>(new Set());

// 事件監聯取消函數
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
    items: [{ type: 'text', content: '欸，你來啦～有什麼需要幫忙的嗎？ *推眼鏡*' }]
  }
]);

// 輸入框內容
const userInput = ref('');

// @-Mention 相關狀態
interface FileItem {
  name: string;
  path: string;
  is_dir: boolean;
}
const showMentionMenu = ref(false);
const mentionFilter = ref('');
const mentionFiles = ref<FileItem[]>([]);
const mentionCursorPosition = ref(0);  // @ 符號的位置
const mentionSelectedIndex = ref(0);   // 選單中選中的項目索引

// 是否正在等待回應
const isLoading = ref(false);

// 阿宇風格忙碌狀態文字
const uniThinkingTexts = [
  // 經典動作
  "推眼鏡中",
  "敲鍵盤中",
  "輕敲桌面",
  "翻閱文件中",
  "盯著螢幕",
  "若有所思",
  "撐著下巴",

  // 工程師風格
  "Debug 中",
  "Compiling...",
  "npm thinking",
  "git thinking",
  "重構思緒中",
  "載入記憶體",
  "優化路徑中",

  // 阿宇口頭禪
  "讓我想想",
  "這個嘛...",
  "嗯......",
  "欸等等",
  "從另一個角度想的話...",

  // 生活化
  "泡咖啡中",
  "喝一口茶",
  "整理思緒",
  "翻找資料",

  // 可愛一點的
  "腦袋轉轉",
  "認真思考",
  "專注模式",
  "沉思中",
];

// 忙碌狀態符號（包含狗狗元素 🐕）
const thinkingSymbols = ['·', '◦', '○', '◉', '●', '🐾', '🐕', '🐶', '🦴', '🐩', '🐕‍🦺'];

// 忙碌狀態文字
const busyStatus = ref('');
let busyTextInterval: ReturnType<typeof setInterval> | null = null;
let symbolIndex = 0;

// 隨機取得忙碌文字
function getRandomThinkingText(): string {
  const text = uniThinkingTexts[Math.floor(Math.random() * uniThinkingTexts.length)];
  const symbol = thinkingSymbols[symbolIndex % thinkingSymbols.length];
  symbolIndex++;
  return `${text} ${symbol}`;
}

// 開始忙碌文字動畫
function startBusyTextAnimation() {
  busyStatus.value = getRandomThinkingText();
  busyTextInterval = setInterval(() => {
    busyStatus.value = getRandomThinkingText();
  }, 2000); // 每 2 秒換一次
}

// 停止忙碌文字動畫
function stopBusyTextAnimation() {
  if (busyTextInterval) {
    clearInterval(busyTextInterval);
    busyTextInterval = null;
  }
  busyStatus.value = '';
}

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

// Extended Thinking 模式
const extendedThinking = ref(false);

// 切換 Extended Thinking
function toggleExtendedThinking() {
  extendedThinking.value = !extendedThinking.value;
  console.log('💭 Extended Thinking:', extendedThinking.value ? 'ON' : 'OFF');
}

// 工作目錄
const workingDir = ref<string | null>(null);

// 工作目錄資料夾名稱（用於 UI 顯示）
const workingDirName = computed(() => {
  if (!workingDir.value) return '—';
  // 取得最後一個資料夾名稱（處理 Windows 和 Unix 路徑）
  const parts = workingDir.value.split(/[\\/]/);
  return parts[parts.length - 1] || parts[parts.length - 2] || '—';
});

// Context 用量（0-100，null 表示尚未取得）
const contextUsage = ref<number | null>(null);

// Context 詳細資訊
const contextInfo = ref<{
  totalTokens?: number;
  maxTokens?: number;
} | null>(null);

// 斜線選單顯示狀態
const showSlashMenu = ref(false);

// 歷史對話相關
interface SessionItem {
  session_id: string;
  created_at: string;
  last_modified: string;
  summary: string | null;
}
const showHistoryMenu = ref(false);
const historySessions = ref<SessionItem[]>([]);
const historyLoading = ref(false);

// IDE 連接狀態
interface IdeClient {
  id: string;
  name: string;
  connected_at: string;
}

interface IdeContext {
  file_path: string | null;
  selected_text: string | null;
  selection: {
    start_line: number;
    start_character: number;
    end_line: number;
    end_character: number;
  } | null;
  language_id: string | null;
  last_updated: string | null;
}

interface IdeServerStatus {
  running: boolean;
  port: number;
  connected_clients: IdeClient[];
  current_context: IdeContext | null;
}

const ideStatus = ref<IdeServerStatus | null>(null);
let ideStatusInterval: ReturnType<typeof setInterval> | null = null;

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
// 取得或建立當前的 assistant 訊息
function getOrCreateAssistantMessage(): Message {
  const lastMsg = messages.value[messages.value.length - 1];
  if (lastMsg && lastMsg.role === 'assistant') {
    return lastMsg;
  }
  // 建立新的 assistant 訊息（items 陣列用於按時間順序存放文字和工具）
  const newMsg: Message = {
    role: 'assistant',
    items: []
  };
  messages.value.push(newMsg);
  return newMsg;
}

function addToolUse(toolName: string, toolId: string, input: Record<string, unknown>) {
  // 取得當前的 assistant 訊息
  const assistantMsg = getOrCreateAssistantMessage();

  // 用 tool_id 檢查是否已存在於 items 中
  const existingById = assistantMsg.items.find(
    (item): item is { type: 'tool'; tool: ToolUseItem } =>
      item.type === 'tool' && item.tool.id === toolId
  );
  if (existingById) return; // 已存在，直接返回

  // 額外檢查：如果是同類型工具且 input 完全相同，也視為重複（stream 重複發送的情況）
  const inputJson = JSON.stringify(input);
  const existingByInput = assistantMsg.items.find(
    (item): item is { type: 'tool'; tool: ToolUseItem } =>
      item.type === 'tool' &&
      item.tool.type === toolName &&
      JSON.stringify(item.tool.input) === inputJson
  );

  if (!existingByInput) {
    // 在 items 中按時間順序添加工具項目
    assistantMsg.items.push({
      type: 'tool',
      tool: {
        id: toolId,
        type: toolName,
        name: toolName,
        input: input
      }
    });
  }

  // 同時更新 currentToolUses（用於權限確認時的引用）
  if (!currentToolUses.value.find(t => t.id === toolId)) {
    currentToolUses.value.push({
      id: toolId,
      type: toolName,
      name: toolName,
      input: input
    });
  }
}

// 處理 Claude 事件
function handleClaudeEvent(event: ClaudeEvent) {
  console.log('Claude event:', event.event_type, event);

  // Debug: 特別追蹤權限相關事件
  if (event.event_type === 'PermissionDenied') {
    console.log('🔴 PermissionDenied received:', {
      tool_name: event.tool_name,
      tool_id: event.tool_id,
      editMode: editMode.value,
      currentPendingPermission: pendingPermission.value
    });
  }

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
        // 取得或建立 assistant 訊息
        const assistantMsg = getOrCreateAssistantMessage();

        // 找到最後一個文字項目，或創建新的
        const lastItem = assistantMsg.items[assistantMsg.items.length - 1];
        if (lastItem && lastItem.type === 'text') {
          // 更新現有的文字項目
          lastItem.content = streamingText.value;
        } else {
          // 在工具後面添加新的文字項目
          assistantMsg.items.push({ type: 'text', content: streamingText.value });
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
      // 在 'ask' 模式下顯示確認對話框
      console.log('🔴 PermissionDenied conditions:', {
        has_tool_name: !!event.tool_name,
        has_tool_id: !!event.tool_id,
        editMode: editMode.value,
        will_show_dialog: !!(event.tool_name && event.tool_id && editMode.value === 'ask')
      });

      if (event.tool_name && event.tool_id && editMode.value === 'ask') {
        // 先加入工具使用記錄（顯示為待確認狀態）
        addToolUse(event.tool_name, event.tool_id, event.input || {});

        // 累積這次請求中被拒絕的工具（用於 "yes-all" 時一次加入白名單）
        deniedToolsThisRequest.value.add(event.tool_name);
        console.log('🔴 Accumulated denied tools:', [...deniedToolsThisRequest.value]);

        // 只有當還沒有待確認的對話框時才顯示（避免覆蓋）
        if (!pendingPermission.value) {
          pendingPermission.value = {
            toolName: event.tool_name,
            toolId: event.tool_id,
            input: event.input || {}
          };

          console.log('🔴 Setting pendingPermission:', pendingPermission.value);

          // 切換到等待狀態
          avatarState.value = 'waiting';
          stopBusyTextAnimation();
          busyStatus.value = '等待確認...';
        }
      } else {
        console.log('🔴 PermissionDenied ignored due to conditions not met');
      }
      break;

    case 'Connected':
      // Claude CLI 已連線
      busyStatus.value = 'Connected';
      break;

    case 'ToolResult':
      // 工具結果
      if (event.tool_id) {
        // 更新 currentToolUses
        const tool = currentToolUses.value.find(t => t.id === event.tool_id);
        if (tool) {
          tool.result = event.result;
          if (event.is_error) {
            tool.isCancelled = true;
          }
        }

        // 同時更新 assistant 訊息中的工具（在 items 陣列中查找）
        const lastMsg = messages.value[messages.value.length - 1];
        if (lastMsg && lastMsg.role === 'assistant') {
          const toolItem = lastMsg.items.find(
            (item): item is { type: 'tool'; tool: ToolUseItem } =>
              item.type === 'tool' && item.tool.id === event.tool_id
          );
          if (toolItem) {
            toolItem.tool.result = event.result;
            if (event.is_error) {
              toolItem.tool.isCancelled = true;
            }
          }
        }
      }
      break;

    case 'Complete':
      // 完成
      isLoading.value = false;
      stopBusyTextAnimation();
      avatarState.value = 'complete';
      streamingText.value = '';
      // 注意：不清空 currentToolUses，讓工具結果保留在對話框內
      // 會在下一次 sendMessageCore 開始時清空

      // 更新 context 使用量資訊
      if (event.context_window_used_percent !== undefined) {
        contextUsage.value = Math.round(event.context_window_used_percent);
        console.log('📊 Context usage updated:', contextUsage.value + '%');
      }
      if (event.total_tokens_in_conversation !== undefined || event.context_window_max !== undefined) {
        contextInfo.value = {
          totalTokens: event.total_tokens_in_conversation,
          maxTokens: event.context_window_max,
        };
        console.log('📊 Context info:', contextInfo.value);
      }

      // 3 秒後回到待機表情
      setTimeout(() => {
        avatarState.value = 'idle';
      }, 3000);
      break;

    case 'Error':
      // 錯誤
      isLoading.value = false;
      stopBusyTextAnimation();
      avatarState.value = 'idle';
      if (event.message) {
        messages.value.push({
          role: 'assistant',
          items: [{ type: 'text', content: `*皺眉* 抱歉，出了點問題：${event.message}` }]
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

// 全域快捷鍵處理
function handleGlobalKeydown(e: KeyboardEvent) {
  const isMac = navigator.userAgent.includes('Mac');
  const ctrlOrCmd = isMac ? e.metaKey : e.ctrlKey;

  // Ctrl/Cmd + N: 開始新對話
  if (ctrlOrCmd && e.key === 'n') {
    e.preventDefault();
    startNewConversation();
    return;
  }

  // Ctrl/Cmd + L: 清除輸入框
  if (ctrlOrCmd && e.key === 'l') {
    e.preventDefault();
    userInput.value = '';
    return;
  }

  // Escape: 中斷請求（如果正在載入）或關閉選單
  if (e.key === 'Escape') {
    if (showMentionMenu.value) {
      closeMentionMenu();
      return;
    }
    if (showSlashMenu.value) {
      showSlashMenu.value = false;
      return;
    }
    if (showHistoryMenu.value) {
      showHistoryMenu.value = false;
      return;
    }
    if (isLoading.value) {
      e.preventDefault();
      interruptRequest();
      return;
    }
  }

  // Ctrl/Cmd + Shift + C: 執行 /compact
  if (ctrlOrCmd && e.shiftKey && e.key === 'C') {
    e.preventDefault();
    executeSlashCommand('/compact');
    return;
  }
}

// 開始新對話
function startNewConversation() {
  // 清除對話（與 /clear 相同）
  messages.value = [
    {
      role: 'assistant',
      items: [{ type: 'text', content: '好，我們開始新的對話吧！有什麼需要幫忙的嗎？ *推眼鏡*' }]
    }
  ];
  sessionId.value = null;
  contextUsage.value = null;
  contextInfo.value = null;

  // 聚焦到輸入框
  const textarea = document.querySelector('textarea');
  if (textarea) {
    textarea.focus();
  }
}

// 元件掛載時設定監聯並獲取工作目錄
onMounted(async () => {
  setupEventListeners();

  // 註冊全域快捷鍵
  window.addEventListener('keydown', handleGlobalKeydown);

  // 獲取當前工作目錄
  try {
    const dir = await invoke<string>('get_working_directory');
    workingDir.value = dir;
    console.log('📁 Working directory:', dir);
  } catch (error) {
    console.error('Failed to get working directory:', error);
  }

  // 開始輪詢 IDE 狀態
  startIdeStatusPolling();
});

// 元件卸載時清理
onUnmounted(() => {
  if (unlistenClaude) {
    unlistenClaude();
  }
  stopBusyTextAnimation();
  stopIdeStatusPolling();

  // 移除全域快捷鍵
  window.removeEventListener('keydown', handleGlobalKeydown);
});

// 送出訊息（核心函數，支援 allowedTools）
async function sendMessageCore(content: string, extraAllowedTools: string[] = []) {
  // 開始載入狀態
  isLoading.value = true;
  avatarState.value = 'processing';
  startBusyTextAnimation();
  streamingText.value = '';
  currentToolUses.value = [];  // 清空當前請求的工具追蹤（舊的已保存在 messages 中）
  deniedToolsThisRequest.value.clear();  // 清空這次請求累積的被拒絕工具

  // 合併 session 白名單和額外的工具
  const allAllowedTools = [
    ...sessionAllowedTools.value,
    ...extraAllowedTools
  ];

  // 根據編輯模式決定 permissionMode
  let permissionMode: string | null = null;
  if (editMode.value === 'auto') {
    permissionMode = 'bypassPermissions';
  } else if (editMode.value === 'plan') {
    permissionMode = 'plan';
  }
  // 'ask' 模式使用 default，不傳參數

  try {
    // 呼叫 Rust 端送出訊息給 Claude CLI
    await invoke('send_to_claude', {
      prompt: content,
      workingDir: null,  // 使用預設目錄
      allowedTools: allAllowedTools.length > 0 ? allAllowedTools : null,
      permissionMode: permissionMode,
      extendedThinking: extendedThinking.value || null
    });
  } catch (error) {
    console.error('Failed to send to Claude:', error);
    isLoading.value = false;
    stopBusyTextAnimation();
    avatarState.value = 'idle';

    // 加入錯誤訊息
    messages.value.push({
      role: 'assistant',
      items: [{ type: 'text', content: `*皺眉* 連接 Claude 時發生錯誤：${error}` }]
    });
  }
}

// 送出訊息（從輸入框）
async function sendMessage() {
  const content = userInput.value.trim();
  if (!content || isLoading.value) return;

  // 記住這次的 prompt（用於權限確認後重新執行）
  lastPrompt.value = content;

  // 加入使用者訊息
  messages.value.push({
    role: 'user',
    items: [{ type: 'text', content: content }]
  });
  userInput.value = '';
  await scrollToBottom();

  await sendMessageCore(content);
}

// 按 Enter 送出（Shift+Enter 換行）
function handleKeydown(e: KeyboardEvent) {
  // 如果 @-mention 選單開啟，處理選單導航
  if (showMentionMenu.value) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      mentionSelectedIndex.value = Math.min(
        mentionSelectedIndex.value + 1,
        mentionFiles.value.length - 1
      );
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      mentionSelectedIndex.value = Math.max(mentionSelectedIndex.value - 1, 0);
      return;
    }
    if (e.key === 'Enter' || e.key === 'Tab') {
      e.preventDefault();
      if (mentionFiles.value.length > 0) {
        selectMentionFile(mentionFiles.value[mentionSelectedIndex.value]);
      }
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      closeMentionMenu();
      return;
    }
  }

  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

// @-Mention: 處理輸入變化
async function handleInput(e: Event) {
  const target = e.target as HTMLTextAreaElement;
  const value = target.value;
  const cursorPos = target.selectionStart || 0;

  // 尋找游標前最近的 @ 符號
  const textBeforeCursor = value.substring(0, cursorPos);
  const lastAtIndex = textBeforeCursor.lastIndexOf('@');

  if (lastAtIndex !== -1) {
    // 檢查 @ 是否在單字開頭（前面是空白或開頭）
    const charBefore = lastAtIndex > 0 ? textBeforeCursor[lastAtIndex - 1] : ' ';
    if (charBefore === ' ' || charBefore === '\n' || lastAtIndex === 0) {
      // 取得 @ 後面的過濾文字
      const filterText = textBeforeCursor.substring(lastAtIndex + 1);

      // 如果過濾文字包含空白，關閉選單
      if (filterText.includes(' ')) {
        closeMentionMenu();
        return;
      }

      mentionCursorPosition.value = lastAtIndex;
      mentionFilter.value = filterText;
      mentionSelectedIndex.value = 0;

      // 載入檔案列表
      await loadMentionFiles(filterText);
      showMentionMenu.value = true;
      return;
    }
  }

  closeMentionMenu();
}

// @-Mention: 載入檔案列表
async function loadMentionFiles(filter: string) {
  if (!workingDir.value) return;

  try {
    // 解析路徑：如果 filter 包含 /，取得子目錄
    const parts = filter.split('/');
    const subPath = parts.length > 1 ? parts.slice(0, -1).join('/') : null;
    const nameFilter = parts[parts.length - 1];

    const files = await invoke<FileItem[]>('list_files', {
      workingDir: workingDir.value,
      subPath: subPath,
      filter: nameFilter || null,
    });

    mentionFiles.value = files;
  } catch (error) {
    console.error('Failed to load files:', error);
    mentionFiles.value = [];
  }
}

// @-Mention: 選擇檔案
function selectMentionFile(file: FileItem) {
  const value = userInput.value;
  const beforeAt = value.substring(0, mentionCursorPosition.value);
  const afterFilter = value.substring(
    mentionCursorPosition.value + 1 + mentionFilter.value.length
  );

  // 如果是資料夾，加上 / 並繼續顯示選單
  if (file.is_dir) {
    userInput.value = beforeAt + '@' + file.path + '/' + afterFilter;
    mentionFilter.value = file.path + '/';
    mentionSelectedIndex.value = 0;
    loadMentionFiles(file.path + '/');
  } else {
    // 如果是檔案，完成選擇
    userInput.value = beforeAt + '@' + file.path + ' ' + afterFilter;
    closeMentionMenu();
  }
}

// @-Mention: 關閉選單
function closeMentionMenu() {
  showMentionMenu.value = false;
  mentionFilter.value = '';
  mentionFiles.value = [];
  mentionSelectedIndex.value = 0;
}

// 歷史對話：載入 session 列表
async function loadHistorySessions() {
  if (!workingDir.value) return;

  historyLoading.value = true;
  try {
    const sessions = await invoke<SessionItem[]>('list_sessions', {
      workingDir: workingDir.value,
    });
    historySessions.value = sessions;
  } catch (error) {
    console.error('Failed to load sessions:', error);
    historySessions.value = [];
  } finally {
    historyLoading.value = false;
  }
}

// IDE Server：取得狀態
async function refreshIdeStatus() {
  try {
    const status = await invoke<IdeServerStatus>('get_ide_status');
    ideStatus.value = status;
  } catch (error) {
    console.error('Failed to get IDE status:', error);
  }
}

// IDE Server：開始輪詢狀態
function startIdeStatusPolling() {
  refreshIdeStatus(); // 立即執行一次
  ideStatusInterval = setInterval(refreshIdeStatus, 2000); // 每 2 秒更新
}

// IDE Server：停止輪詢
function stopIdeStatusPolling() {
  if (ideStatusInterval) {
    clearInterval(ideStatusInterval);
    ideStatusInterval = null;
  }
}

// 計算 IDE 連接狀態文字
const ideConnectionText = computed(() => {
  if (!ideStatus.value) return 'IDE: —';
  if (!ideStatus.value.running) return 'IDE: Off';
  const clientCount = ideStatus.value.connected_clients.length;
  if (clientCount === 0) return 'IDE: Waiting';
  if (clientCount === 1) return `IDE: ${ideStatus.value.connected_clients[0].name}`;
  return `IDE: ${clientCount} connected`;
});

// 計算 IDE 當前 context 顯示
const ideContextDisplay = computed(() => {
  if (!ideStatus.value?.current_context?.file_path) return null;
  const ctx = ideStatus.value.current_context;
  const fileName = ctx.file_path?.split(/[\\/]/).pop() || '';
  if (ctx.selection) {
    return `${fileName}:${ctx.selection.start_line + 1}`;
  }
  return fileName;
});

// 插入 IDE context 參考到輸入框
function insertIdeContextReference() {
  const ctx = ideStatus.value?.current_context;
  if (!ctx?.file_path) return;

  // 生成 @file#L1-10 格式的參考
  let reference = `@${ctx.file_path}`;
  if (ctx.selection) {
    const startLine = ctx.selection.start_line + 1;
    const endLine = ctx.selection.end_line + 1;
    if (startLine === endLine) {
      reference += `#L${startLine}`;
    } else {
      reference += `#L${startLine}-${endLine}`;
    }
  }

  // 插入到輸入框（在游標位置或末尾）
  const textarea = document.querySelector('textarea') as HTMLTextAreaElement | null;
  if (textarea) {
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const before = userInput.value.substring(0, start);
    const after = userInput.value.substring(end);

    // 確保前後有空格
    const needSpaceBefore = before.length > 0 && !before.endsWith(' ') && !before.endsWith('\n');
    const needSpaceAfter = after.length > 0 && !after.startsWith(' ') && !after.startsWith('\n');

    userInput.value = before + (needSpaceBefore ? ' ' : '') + reference + (needSpaceAfter ? ' ' : '') + after;

    // 聚焦並移動游標到插入點後
    textarea.focus();
    const newPosition = start + (needSpaceBefore ? 1 : 0) + reference.length + (needSpaceAfter ? 1 : 0);
    textarea.setSelectionRange(newPosition, newPosition);
  } else {
    // 沒有 textarea，直接附加到末尾
    userInput.value = (userInput.value ? userInput.value + ' ' : '') + reference + ' ';
  }

  console.log('📎 插入 IDE context 參考:', reference);
}

// 歷史對話：切換選單顯示
async function toggleHistoryMenu() {
  showSlashMenu.value = false; // 關閉斜線選單
  showHistoryMenu.value = !showHistoryMenu.value;

  if (showHistoryMenu.value) {
    await loadHistorySessions();
  }
}

// 歷史對話：恢復指定 session
async function resumeSession(session: SessionItem) {
  showHistoryMenu.value = false;

  // 設定要恢復的 session ID
  sessionId.value = session.session_id;

  // 清除當前對話，準備恢復
  messages.value = [
    {
      role: 'assistant',
      items: [{ type: 'text', content: `*翻閱之前的筆記* 嗯，讓我看看我們上次聊到哪裡...` }]
    }
  ];
  contextUsage.value = null;
  contextInfo.value = null;

  // 發送一個簡單的繼續訊息來恢復對話
  await scrollToBottom();
  lastPrompt.value = '/continue';
  await sendMessageCore('/continue');
}

// 歷史對話：格式化時間顯示
function formatSessionTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return 'Today ' + date.toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit' });
  } else if (diffDays === 1) {
    return 'Yesterday';
  } else if (diffDays < 7) {
    return `${diffDays} days ago`;
  } else {
    return date.toLocaleDateString('zh-TW', { month: 'short', day: 'numeric' });
  }
}

// 執行斜線命令
async function executeSlashCommand(command: string) {
  showSlashMenu.value = false;

  // 將斜線命令作為訊息發送
  messages.value.push({
    role: 'user',
    items: [{ type: 'text', content: command }]
  });
  await scrollToBottom();

  // 特殊處理某些命令
  if (command === '/clear') {
    // 清除對話（開始新 session）
    messages.value = [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '好，我們開始新的對話吧！有什麼需要幫忙的嗎？ *推眼鏡*' }]
      }
    ];
    sessionId.value = null;
    contextUsage.value = null;
    contextInfo.value = null;
    return;
  }

  // 其他命令直接發送給 Claude CLI
  lastPrompt.value = command;
  await sendMessageCore(command);
}

// 處理權限確認回應
// 使用「後處理」模式：確認後用 --allowedTools 重新執行
async function handlePermissionResponse(response: 'yes' | 'yes-all' | 'yes-always' | 'no' | 'custom', customMessage?: string) {
  console.log('🟢 Permission response:', response, customMessage);
  console.log('🟢 Current state:', {
    pendingPermission: pendingPermission.value,
    lastPrompt: lastPrompt.value,
    sessionAllowedTools: [...sessionAllowedTools.value]
  });

  if (!pendingPermission.value) return;

  const toolName = pendingPermission.value.toolName;
  const toolId = pendingPermission.value.toolId;

  // 清除待確認的權限
  pendingPermission.value = null;

  switch (response) {
    case 'yes':
      // 單次允許：用 --allowedTools 重新執行同一個請求
      if (lastPrompt.value) {
        console.log(`Re-executing with allowedTools: ${toolName}`);
        await sendMessageCore(lastPrompt.value, [toolName]);
      }
      break;

    case 'yes-all':
      // 本次 session 都允許：將所有這次被拒絕的工具都加入白名單後重新執行
      // 這樣可以一次處理多個被拒絕的工具，避免反覆確認
      for (const deniedTool of deniedToolsThisRequest.value) {
        sessionAllowedTools.value.add(deniedTool);
      }
      console.log(`Added all denied tools to session whitelist:`, [...sessionAllowedTools.value]);
      if (lastPrompt.value) {
        await sendMessageCore(lastPrompt.value);
      }
      break;

    case 'yes-always':
      // 專案內永久允許：將所有這次被拒絕的工具寫入專案設定
      // 同時也加入 session 白名單
      if (!workingDir.value) {
        console.error('Working directory not set, cannot save project permission');
        // Fallback to session-only permission
        for (const deniedTool of deniedToolsThisRequest.value) {
          sessionAllowedTools.value.add(deniedTool);
        }
      } else {
        for (const deniedTool of deniedToolsThisRequest.value) {
          sessionAllowedTools.value.add(deniedTool);
          // 寫入專案設定
          try {
            await invoke('add_project_permission', {
              workingDir: workingDir.value,
              toolName: deniedTool
            });
            console.log(`Added project permission: ${deniedTool}`);
          } catch (error) {
            console.error(`Failed to add project permission for ${deniedTool}:`, error);
          }
        }
      }
      if (lastPrompt.value) {
        await sendMessageCore(lastPrompt.value);
      }
      break;

    case 'no':
      // 拒絕：更新工具使用記錄，不重新執行
      {
        // 更新 currentToolUses
        const tool = currentToolUses.value.find(t => t.id === toolId);
        if (tool) {
          tool.isCancelled = true;
          tool.userResponse = 'Denied by user';
        }
        // 同時更新 assistant 訊息中的工具（在 items 陣列中查找）
        const lastMsg = messages.value[messages.value.length - 1];
        if (lastMsg && lastMsg.role === 'assistant') {
          const toolItem = lastMsg.items.find(
            (item): item is { type: 'tool'; tool: ToolUseItem } =>
              item.type === 'tool' && item.tool.id === toolId
          );
          if (toolItem) {
            toolItem.tool.isCancelled = true;
            toolItem.tool.userResponse = 'Denied by user';
          }
        }
      }
      avatarState.value = 'idle';
      break;

    case 'custom':
      // 自訂回應：發送使用者的訊息作為新的請求
      if (customMessage) {
        // 更新 currentToolUses
        const tool = currentToolUses.value.find(t => t.id === toolId);
        if (tool) {
          tool.isCancelled = true;
          tool.userResponse = customMessage;
        }
        // 同時更新 assistant 訊息中的工具（在 items 陣列中查找）
        const lastMsg = messages.value[messages.value.length - 1];
        if (lastMsg && lastMsg.role === 'assistant') {
          const toolItem = lastMsg.items.find(
            (item): item is { type: 'tool'; tool: ToolUseItem } =>
              item.type === 'tool' && item.tool.id === toolId
          );
          if (toolItem) {
            toolItem.tool.isCancelled = true;
            toolItem.tool.userResponse = customMessage;
          }
        }
        // 將自訂訊息作為新的使用者訊息發送
        messages.value.push({
          role: 'user',
          items: [{ type: 'text', content: customMessage }]
        });
        await scrollToBottom();
        lastPrompt.value = customMessage;
        await sendMessageCore(customMessage);
      }
      break;
  }
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
  stopBusyTextAnimation();
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
            v-for="(msg, msgIndex) in messages"
            :key="msgIndex"
            :class="['message', msg.role]"
          >
            <!-- 按時間順序渲染 items（文字和工具交錯顯示）-->
            <template v-for="(item, itemIndex) in msg.items" :key="itemIndex">
              <!-- 工具項目 -->
              <div v-if="item.type === 'tool'" class="tool-item">
                <ToolIndicator
                  :type="item.tool.type as any"
                  :path="(item.tool.input?.file_path as string) || (item.tool.input?.path as string)"
                  :description="item.tool.input?.description as string"
                  :input="item.tool.input?.command as string"
                  :output="item.tool.result"
                  :isRunning="!item.tool.result && !item.tool.isCancelled"
                  :isCancelled="item.tool.isCancelled"
                  :userResponse="item.tool.userResponse"
                />
              </div>

              <!-- 文字項目 -->
              <div
                v-else-if="item.type === 'text' && item.content"
                class="message-content markdown-body"
                v-html="renderMarkdown(item.content)"
              ></div>
            </template>
          </div>

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
          @input="handleInput"
          placeholder="Type @ to mention files..."
          :disabled="isLoading"
          rows="2"
        ></textarea>

        <!-- @-Mention 選單 -->
        <div v-if="showMentionMenu && mentionFiles.length > 0" class="mention-menu">
          <div class="mention-header">
            <span class="mention-icon">📄</span>
            <span>Files</span>
            <span class="mention-filter" v-if="mentionFilter">{{ mentionFilter }}</span>
          </div>
          <div
            v-for="(file, index) in mentionFiles"
            :key="file.path"
            :class="['mention-item', { selected: index === mentionSelectedIndex }]"
            @click="selectMentionFile(file)"
            @mouseenter="mentionSelectedIndex = index"
          >
            <span class="mention-item-icon">{{ file.is_dir ? '📁' : '📄' }}</span>
            <span class="mention-item-name">{{ file.name }}</span>
            <span class="mention-item-path" v-if="mentionFilter.includes('/')">{{ file.path }}</span>
          </div>
        </div>
      </div>

      <!-- 狀態列 -->
      <div class="status-bar">
        <!-- 左側按鈕 -->
        <div class="status-left">
          <button class="status-btn edit-mode" @click="cycleEditMode" :title="editModeLabels[editMode]">
            <span class="mode-icon">▶</span>
            <span class="mode-label">{{ editModeLabels[editMode] }}</span>
          </button>
          <button class="status-btn working-dir" :title="workingDir || '未知'">
            <span class="dir-icon">📁</span>
            <span class="dir-name">{{ workingDirName }}</span>
          </button>
          <button
            class="status-btn thinking-toggle"
            :class="{ active: extendedThinking }"
            @click="toggleExtendedThinking"
            title="Extended Thinking"
          >
            <span class="thinking-icon">💭</span>
            <span class="thinking-label">{{ extendedThinking ? 'Thinking ON' : 'Thinking' }}</span>
          </button>
          <button
            class="status-btn context-usage"
            :class="{ warning: contextUsage !== null && contextUsage >= 80, danger: contextUsage !== null && contextUsage >= 95 }"
            :title="contextInfo ? `Tokens: ${contextInfo.totalTokens?.toLocaleString() || '?'} / ${contextInfo.maxTokens?.toLocaleString() || '?'}` : 'Context usage'"
          >
            <span class="usage-icon">◐</span>
            <span class="usage-text">{{ contextUsage !== null ? contextUsage + '% used' : '—' }}</span>
          </button>
          <button
            class="status-btn ide-status"
            :class="{
              connected: (ideStatus?.connected_clients?.length ?? 0) > 0,
              waiting: ideStatus?.running && (ideStatus?.connected_clients?.length ?? 0) === 0,
              off: !ideStatus?.running
            }"
            :title="ideStatus?.running ? `WebSocket port ${ideStatus?.port}` : 'IDE Server not running'"
          >
            <span class="ide-icon">🔗</span>
            <span class="ide-text">{{ ideConnectionText }}</span>
            <span
              v-if="ideContextDisplay"
              class="ide-context clickable"
              @click.stop="insertIdeContextReference"
              title="點擊插入檔案參考"
            >{{ ideContextDisplay }}</span>
          </button>
        </div>

        <!-- 右側按鈕 -->
        <div class="status-right">
          <button class="status-btn history-btn" @click="toggleHistoryMenu" title="對話歷史">
            <span class="history-icon">🕐</span>
          </button>
          <button class="status-btn attach-btn" title="Attach files">
            <span class="attach-icon">📎</span>
          </button>
          <button class="status-btn slash-btn" @click="showSlashMenu = !showSlashMenu; showHistoryMenu = false" title="Commands">
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
          <div class="slash-item" @click="executeSlashCommand('/model')">
            Switch model... <span class="slash-hint">{{ currentModel || 'default' }}</span>
          </div>
        </div>
        <div class="slash-section">
          <div class="slash-section-title">Slash Commands</div>
          <div class="slash-item" @click="executeSlashCommand('/compact')">
            /compact <span class="slash-hint">壓縮對話歷史</span>
          </div>
          <div class="slash-item" @click="executeSlashCommand('/cost')">
            /cost <span class="slash-hint">查看使用費用</span>
          </div>
          <div class="slash-item" @click="executeSlashCommand('/clear')">
            /clear <span class="slash-hint">開始新對話</span>
          </div>
        </div>
        <div class="slash-section">
          <div class="slash-section-title">Context</div>
          <div class="slash-item context-info">
            <span>使用量</span>
            <span class="slash-hint">{{ contextUsage !== null ? contextUsage + '%' : '—' }}</span>
          </div>
          <div v-if="contextInfo" class="slash-item context-info">
            <span>Tokens</span>
            <span class="slash-hint">{{ contextInfo.totalTokens?.toLocaleString() || '?' }} / {{ contextInfo.maxTokens?.toLocaleString() || '?' }}</span>
          </div>
        </div>
      </div>

      <!-- 歷史對話選單 -->
      <div v-if="showHistoryMenu" class="history-menu">
        <div class="history-header">
          <span class="history-title">🕐 對話歷史</span>
          <button class="history-close" @click="showHistoryMenu = false">✕</button>
        </div>
        <div v-if="historyLoading" class="history-loading">
          載入中...
        </div>
        <div v-else-if="historySessions.length === 0" class="history-empty">
          還沒有對話歷史
        </div>
        <div v-else class="history-list">
          <div
            v-for="session in historySessions"
            :key="session.session_id"
            class="history-item"
            @click="resumeSession(session)"
          >
            <div class="history-item-time">{{ formatSessionTime(session.last_modified) }}</div>
            <div class="history-item-summary">{{ session.summary || '(無摘要)' }}</div>
          </div>
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
  flex-direction: column;  /* 垂直排列：工具在上，訊息在下 */
  max-width: 85%;
}

.message.user {
  align-self: flex-start;
}

.message.assistant {
  align-self: flex-end;
  max-width: 85%;  /* 讓 assistant 訊息可以更寬以容納工具 */
}

/* 工具項目 - 與文字交錯顯示 */
.tool-item {
  width: 100%;
  margin-bottom: 8px;
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
  position: relative;
}

/* @-Mention 選單 */
.mention-menu {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  max-height: 300px;
  overflow-y: auto;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  margin-bottom: 4px;
  z-index: 100;
}

.mention-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  font-size: 0.8rem;
  color: var(--text-muted);
}

.mention-icon {
  font-size: 1rem;
}

.mention-filter {
  margin-left: auto;
  font-family: monospace;
  background-color: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
  transition: background-color 0.15s;
}

.mention-item:hover,
.mention-item.selected {
  background-color: var(--primary-color);
}

.mention-item-icon {
  font-size: 1rem;
  flex-shrink: 0;
}

.mention-item-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mention-item-path {
  font-size: 0.75rem;
  color: var(--text-muted);
  font-family: monospace;
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
.dir-icon,
.usage-icon {
  font-size: 0.75rem;
}

/* Extended Thinking 按鈕 */
.thinking-toggle {
  transition: all 0.2s;
}

.thinking-toggle.active {
  color: #9b59b6;
  background-color: rgba(155, 89, 182, 0.15);
}

.thinking-toggle.active .thinking-icon {
  animation: thinking-pulse 1.5s infinite;
}

@keyframes thinking-pulse {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.1); }
}

/* Context usage 警告樣式 */
.context-usage.warning {
  color: #f39c12;
}

.context-usage.warning .usage-icon {
  animation: pulse-warning 1.5s infinite;
}

.context-usage.danger {
  color: #e74c3c;
  font-weight: 600;
}

.context-usage.danger .usage-icon {
  animation: pulse-danger 0.8s infinite;
}

@keyframes pulse-warning {
  0%, 100% { opacity: 0.7; }
  50% { opacity: 1; }
}

@keyframes pulse-danger {
  0%, 100% { opacity: 0.5; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.1); }
}

/* IDE 連接狀態 */
.ide-status {
  transition: all 0.2s;
}

.ide-status.connected {
  color: #2ecc71;
}

.ide-status.connected .ide-icon {
  animation: ide-connected 2s infinite;
}

.ide-status.waiting {
  color: #f39c12;
}

.ide-status.waiting .ide-icon {
  animation: ide-waiting 1.5s infinite;
}

.ide-status.off {
  color: var(--text-muted);
  opacity: 0.6;
}

.ide-context {
  font-family: monospace;
  font-size: 0.75rem;
  background-color: rgba(46, 204, 113, 0.2);
  padding: 1px 6px;
  border-radius: 3px;
  margin-left: 4px;
}

.ide-context.clickable {
  cursor: pointer;
  transition: all 0.15s;
}

.ide-context.clickable:hover {
  background-color: rgba(46, 204, 113, 0.4);
  transform: scale(1.05);
}

@keyframes ide-connected {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}

@keyframes ide-waiting {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
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

.slash-item:hover:not(.context-info) {
  background-color: var(--primary-color);
}

.slash-item.context-info {
  cursor: default;
  opacity: 0.8;
}

.slash-hint {
  color: var(--text-muted);
  font-size: 0.8rem;
}

.slash-toggle {
  font-size: 1.2rem;
}

/* 歷史對話選單 */
.history-menu {
  position: absolute;
  bottom: 100%;
  right: 20px;
  width: 360px;
  max-height: 400px;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 100;
  display: flex;
  flex-direction: column;
}

.history-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  border-bottom: 1px solid var(--border-color);
}

.history-title {
  font-weight: 500;
}

.history-close {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1rem;
  padding: 4px;
}

.history-close:hover {
  color: var(--text-color);
}

.history-loading,
.history-empty {
  padding: 24px;
  text-align: center;
  color: var(--text-muted);
}

.history-list {
  overflow-y: auto;
  max-height: 340px;
}

.history-item {
  padding: 12px;
  border-bottom: 1px solid var(--border-color);
  cursor: pointer;
  transition: background-color 0.15s;
}

.history-item:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.history-item:last-child {
  border-bottom: none;
}

.history-item-time {
  font-size: 0.75rem;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.history-item-summary {
  font-size: 0.9rem;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
