<script setup lang="ts">
import { ref, nextTick, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { File, Folder, Paperclip, Copy, Check, Square, Slash, FolderOpen } from "lucide-vue-next";
import ToolIndicator from "./components/ToolIndicator.vue";
import PermissionDialog from "./components/PermissionDialog.vue";
import PlanApprovalDialog from "./components/PlanApprovalDialog.vue";
import AskUserQuestionDialog from "./components/AskUserQuestionDialog.vue";
import SessionSelector from "./components/SessionSelector.vue";
import ImagePreview, { type AttachedImage } from "./components/ImagePreview.vue";
import SetupWizard from "./components/SetupWizard.vue";
import { useTabManager } from "./composables/useTabManager";
import { renderMarkdown } from "./utils/markdown";
import {
  handleClaudeEvent as processClaudeEvent,
  type ClaudeEvent,
  type PendingPermission,
  type ToolUseItem,
  type Message,
  type AppState,
  type EventAction,
  type EditMode,
  type AvatarState,
  type DiffHunk,
} from "./utils/claudeEventHandler";

// Tab Manager
const tabManager = useTabManager();

// 目前的 session ID
const sessionId = ref<string | null>(null);

// 目前使用的 model
const currentModel = ref('');

// Session 白名單（這個 session 內允許的工具）
const sessionAllowedTools = ref<Set<string>>(new Set());

// 記住上一次的 prompt（用於權限確認後重新執行）
const lastPrompt = ref<string>('');

// 待送出的訊息（Claude 工作中時使用者輸入的下一則訊息）
const pendingMessage = ref<string | null>(null);

// 累積的回應文字
const streamingText = ref('');

// 目前的工具使用
const currentToolUses = ref<ToolUseItem[]>([]);

// 這次請求中所有被拒絕的工具（用於 "yes-all" 時一次加入白名單）
const deniedToolsThisRequest = ref<Set<string>>(new Set());

// 事件監聯取消函數
let unlistenClaude: UnlistenFn | null = null;
let unlistenPermissionRequest: UnlistenFn | null = null;
let unlistenPlanApproval: UnlistenFn | null = null;

// Avatar 圖片對應 - 靜態狀態（無眨眼動畫的狀態）
const staticAvatarImages: Record<string, string> = {
  thinking: '/character/thinking.png',      // 思考中：手托下巴
  complete: '/character/complete-1.png',    // 完成：比讚
  planApproved: '/character/complete-2.png', // 計畫批准：OK 手勢
  // error 和 waiting 已改為動畫（有眨眼）
};

// Idle 眨眼動畫幀（約 5 秒一次眨眼，10fps = 50 幀，有半閉）
const idleFrames = [
  ...Array(23).fill('/character/idle.png'),    // 睜眼
  '/character/idle-half.png',                   // 半閉
  '/character/idle-blink.png',                  // 閉眼
  '/character/idle-blink.png',                  // 閉眼
  '/character/idle-half.png',                   // 半閉
  ...Array(23).fill('/character/idle.png'),    // 睜眼
];

// Working 動畫幀（8 張循環）
const workingFrames = [
  '/character/working-1.png',
  '/character/working-2.png',
  '/character/working-1.png',
  '/character/working-3.png',
  '/character/working-1.png',
  '/character/working-4.png',
  '/character/working-1.png',
  '/character/working-5.png',
  '/character/working-1.png',
  '/character/working-6.png',
  '/character/working-7.png',
  '/character/working-8.png',
];

// Error 眨眼動畫幀（約 5 秒一次眨眼，5fps = 25 幀，無半閉）
const errorFrames = [
  ...Array(24).fill('/character/error.png'),      // 睜眼（大部分時間）
  '/character/error-blink.png',                    // 閉眼
];

// Waiting 眨眼動畫幀（約 5 秒一次眨眼，5fps = 25 幀，無半閉）
const waitingFrames = [
  ...Array(24).fill('/character/waiting.png'),    // 睜眼（大部分時間）
  '/character/waiting-blink.png',                  // 閉眼
];

// 當前表情狀態
const avatarState = ref<AvatarState>('idle');

// 動畫相關
const currentFrame = ref(0);
let avatarAnimationTimer: ReturnType<typeof setInterval> | null = null;

// 計算當前應顯示的 Avatar
const currentAvatar = computed(() => {
  const state = avatarState.value;
  if (state === 'idle') {
    return idleFrames[currentFrame.value % idleFrames.length];
  } else if (state === 'working') {
    return workingFrames[currentFrame.value % workingFrames.length];
  } else if (state === 'error') {
    return errorFrames[currentFrame.value % errorFrames.length];
  } else if (state === 'waiting') {
    return waitingFrames[currentFrame.value % waitingFrames.length];
  } else {
    return staticAvatarImages[state] || '/character/idle.png';
  }
});

// 開始動畫
function startAvatarAnimation(fps: number) {
  stopAvatarAnimation();
  currentFrame.value = 0;
  avatarAnimationTimer = setInterval(() => {
    currentFrame.value++;
  }, 1000 / fps);
}

// 停止動畫
function stopAvatarAnimation() {
  if (avatarAnimationTimer) {
    clearInterval(avatarAnimationTimer);
    avatarAnimationTimer = null;
  }
  currentFrame.value = 0;
}

// 監控 avatarState 變化，控制動畫
watch(avatarState, (newState) => {
  if (newState === 'idle') {
    // 待機：慢速眨眼（約 5 秒一個循環，52 幀 @ 10fps）
    startAvatarAnimation(10);
  } else if (newState === 'working') {
    // 工作中：快速動畫（12 幀，約 2 秒循環 = 6fps）
    startAvatarAnimation(6);
  } else if (newState === 'error' || newState === 'waiting') {
    // error/waiting：慢速眨眼（約 5 秒一次，25 幀 @ 5fps）
    startAvatarAnimation(5);
  } else {
    // 其他狀態（thinking, complete, planApproved）：停止動畫，顯示靜態圖
    stopAvatarAnimation();
  }
}, { immediate: true });

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

// Slash Commands (Skills) 相關
interface SkillItem {
  name: string;
  description: string;
  source: 'builtin' | 'user' | 'project';
}
const allSkills = ref<SkillItem[]>([]);
const availableSkills = ref<string[]>([]);  // 從 init 事件取得的可用 Skills
const slashFilter = ref('');
const skillsLoaded = ref(false);
const mentionFilter = ref('');
const mentionFiles = ref<FileItem[]>([]);
const mentionCursorPosition = ref(0);  // @ 符號的位置
const mentionSelectedIndex = ref(0);   // 選單中選中的項目索引

// 附加的圖片列表
const attachedImages = ref<AttachedImage[]>([]);
let imageIdCounter = 0;
const fileInputRef = ref<HTMLInputElement | null>(null);

// 是否正在等待回應
const isLoading = ref(false);
const isProcessAlive = ref(false);  // 互動模式：CLI process 是否存活

// Toast 通知狀態
const toastMessage = ref('');
const toastVariant = ref<'info' | 'warning' | 'error'>('info');
const toastVisible = ref(false);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

function showToast(message: string, variant: 'info' | 'warning' | 'error' = 'info') {
  // 清除之前的計時器
  if (toastTimer) {
    clearTimeout(toastTimer);
  }

  toastMessage.value = message;
  toastVariant.value = variant;
  toastVisible.value = true;

  // 4 秒後自動隱藏
  toastTimer = setTimeout(() => {
    toastVisible.value = false;
  }, 4000);
}

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
  "Compiling",
  "npm thinking",
  "git thinking",
  "重構思緒中",
  "載入記憶體",
  "優化路徑中",

  // 阿宇口頭禪
  "讓我想想",
  "這個嘛",
  "嗯",
  "欸等等",
  "從另一個角度想的話",

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
const thinkingSymbols = ['👓', '💭', '🍵','🥤', '🍓', '☕', '💻', '⌨️', '🐾', '🐕', '🐶', '🦴', '🐩', '🐕‍🦺'];

// 忙碌狀態文字
const busyStatus = ref('');
let busyTextInterval: ReturnType<typeof setInterval> | null = null;
let symbolIndex = 0;

// 隨機取得忙碌文字
function getRandomThinkingText(): string {
  const text = uniThinkingTexts[Math.floor(Math.random() * uniThinkingTexts.length)];
  const symbol = thinkingSymbols[symbolIndex % thinkingSymbols.length];
  symbolIndex++;
  return `${text}... ${symbol}`;
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

// 編輯模式（變數名稱對齊 Claude CLI，Label 對齊 VS Code）
const editMode = ref<EditMode>('default');
const editModeLabels: Record<EditMode, string> = {
  default: 'Ask before edits',
  acceptEdits: 'Edit automatically',   // Edit/Write 免確認，Bash 仍需確認
  bypassPermissions: 'Trust all',      // 全部跳過確認
  plan: 'Plan Mode'
};

const editModeIcons: Record<EditMode, string> = {
  default: '▶',
  acceptEdits: '⏩︎',
  bypassPermissions: '❉',
  plan: '⏸'  // pause
};

// 切換編輯模式
function cycleEditMode() {
  const modes: EditMode[] = ['default', 'acceptEdits', 'bypassPermissions', 'plan'];
  const currentIndex = modes.indexOf(editMode.value);
  editMode.value = modes[(currentIndex + 1) % modes.length];
}

// editMode 變更時同步到後端 permission_server + 標記需要 respawn
// 這樣 server 才能根據模式自動允許對應的工具
watch(editMode, async (newMode) => {
  try {
    await invoke('set_edit_mode', { mode: newMode });
  } catch (error) {
    console.error('Failed to sync edit mode to backend:', error);
  }
  // 互動模式：標記 process 需要 respawn（下次 ensureProcess 時重新啟動）
  markProcessForRespawn();
}, { immediate: true });

// Extended Thinking 模式
const extendedThinking = ref(false);

// 切換 Extended Thinking
function toggleExtendedThinking() {
  extendedThinking.value = !extendedThinking.value;
  console.log('💭 Extended Thinking:', extendedThinking.value ? 'ON' : 'OFF');
  // 互動模式：標記 process 需要 respawn
  markProcessForRespawn();
}

// 互動模式：設定變更時標記 process 需要 respawn
// 不立即中斷，等下次送訊息時 ensureProcess() 自然重新啟動
async function markProcessForRespawn() {
  if (!isProcessAlive.value) return;
  try {
    await invoke('interrupt_claude');
  } catch (error) {
    console.error('Failed to interrupt Claude for respawn:', error);
  }
  isProcessAlive.value = false;
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

// 選擇工作目錄
async function selectWorkingDir() {
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '選擇專案資料夾',
      defaultPath: workingDir.value || undefined,
    });

    if (selected && typeof selected === 'string') {
      // 開始切換：顯示 loading 狀態
      isLoading.value = true;
      busyStatus.value = '切換專案中...';
      avatarState.value = 'working';

      // 更新工作目錄
      workingDir.value = selected;

      // 互動模式：中斷舊 process（新目錄需要新 process）
      await markProcessForRespawn();

      // 清除當前 session 狀態
      sessionId.value = null;
      messages.value = [{
        role: 'assistant',
        items: [{ type: 'text', content: `好，我們來看看 ${workingDirName.value} 這個專案～ *推眼鏡*` }]
      }];
      streamingText.value = '';
      currentToolUses.value = [];

      // 清空舊的歷史 sessions（避免看到舊專案的資料）
      historySessions.value = [];  

      // 重新載入 skills
      skillsLoaded.value = false;
      await loadSkills();

      // 重新載入歷史 sessions
      await loadHistorySessions();

      // 重新載入標籤頁（每個專案有自己的標籤頁）
      await tabManager.initialize(workingDir.value);

      // 恢復目前標籤頁的狀態
      restoreTabState();

      // 如果目前標籤頁有 sessionId，載入歷史訊息
      const activeTab = tabManager.activeTab.value;
      if (activeTab?.sessionId) {
        sessionId.value = activeTab.sessionId;
        await loadTabHistory(activeTab.sessionId);
      }

      // 切換完成：恢復 idle 狀態
      isLoading.value = false;
      busyStatus.value = '';
      avatarState.value = 'idle';
    }
  } catch (error) {
    console.error('Failed to select directory:', error);
    // 錯誤時也要恢復狀態
    isLoading.value = false;
    busyStatus.value = '';
    avatarState.value = 'error';
  }
}

// Context 用量（0-100，null 表示尚未取得）
const contextUsage = ref<number | null>(null);

// Context 詳細資訊
const contextInfo = ref<{
  totalTokens?: number;
  maxTokens?: number;
  inputTokensThisTurn?: number;
  inputTokensDelta?: number;
} | null>(null);

// 上一次 turn 的 input tokens（用於計算增量）
const prevInputTokens = ref<number | null>(null);

// Context 用量圖示（根據用量百分比顯示不同的圓圈）
const contextUsageIcon = computed(() => {
  const usage = contextUsage.value;
  if (usage === null || usage < 50) return '◯';  // 空心
  if (usage < 75) return '◑';  // 半滿
  if (usage < 95) return '◕';  // 快滿
  return '◉';  // 全滿
});

// 斜線選單顯示狀態
const showSlashMenu = ref(false);

// 首次啟動安裝精靈
const showSetupWizard = ref(false);

// 附加組件安裝狀態
interface AddonStatus {
  vscode_available: boolean;
  vscode_installed: boolean;
  claude_available: boolean;
  skill_installed: boolean;
  jetbrains_available: boolean;
  jetbrains_installed: boolean;
  jetbrains_ides: Array<{
    name: string;
    config_path: string;
    plugins_path: string;
    plugin_installed: boolean;
  }>;
}
const addonStatus = ref<AddonStatus | null>(null);
const hasUninstalledAddons = computed(() => {
  if (!addonStatus.value) return false;
  const s = addonStatus.value;
  return (s.vscode_available && !s.vscode_installed) ||
         (s.jetbrains_available && !s.jetbrains_installed) ||
         !s.skill_installed;
});

async function refreshAddonStatus() {
  try {
    addonStatus.value = await invoke<AddonStatus>('check_addon_status');
  } catch (e) {
    console.error('Failed to check addon status:', e);
  }
}

// 歷史對話相關
interface SessionItem {
  session_id: string;
  created_at: string;
  last_modified: string;
  summary: string | null;
}
const historySessions = ref<SessionItem[]>([]);
const historyLoading = ref(false);

// IDE 連接狀態
interface IdeClient {
  id: string;
  name: string;
  connected_at: string;
  workspace_path: string | null;  // 工作區路徑（用於過濾同專案 IDE）
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
  client_id: string | null;  // 來源客戶端 ID（用於判斷是哪個 IDE 發送的）
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

// AskUserQuestion 的問題類型
interface QuestionOption {
  label: string;
  description?: string;
}

interface Question {
  question: string;
  header: string;
  options: QuestionOption[];
  multiSelect: boolean;
}

interface PendingQuestion {
  toolId: string;
  questions: Question[];
}

// 待回答的問題（AskUserQuestion 工具）
const pendingQuestion = ref<PendingQuestion | null>(null);

// 對話區域的參考，用於自動滾動
const chatContainer = ref<HTMLElement | null>(null);

// 滾動到最底部
async function scrollToBottom() {
  await nextTick();
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
  }
}

// 複製文字到剪貼簿
const copiedIndex = ref<string | null>(null);

async function copyToClipboard(content: string, itemKey: string) {
  try {
    await navigator.clipboard.writeText(content);
    copiedIndex.value = itemKey;
    // 2 秒後重置
    setTimeout(() => {
      if (copiedIndex.value === itemKey) {
        copiedIndex.value = null;
      }
    }, 2000);
  } catch (err) {
    console.error('Failed to copy:', err);
  }
}

// 攔截外部超連結，改在系統瀏覽器開啟
function handleExternalLinkClick(e: MouseEvent) {
  const target = (e.target as HTMLElement).closest('a');
  if (!target) return;
  const href = target.getAttribute('href');
  if (href && /^https?:\/\//.test(href)) {
    e.preventDefault();
    openUrl(href).catch(err => console.error('Failed to open URL:', err));
  }
}

// 取得當前 App 狀態（用於事件處理）
function getCurrentAppState(): AppState {
  return {
    sessionId: sessionId.value,
    currentModel: currentModel.value,
    streamingText: streamingText.value,
    messages: messages.value,
    currentToolUses: currentToolUses.value,
    deniedToolsThisRequest: deniedToolsThisRequest.value,
    pendingPermission: pendingPermission.value,
    avatarState: avatarState.value,
    busyStatus: busyStatus.value,
    isLoading: isLoading.value,
    editMode: editMode.value,
    contextUsage: contextUsage.value,
    contextInfo: contextInfo.value,
    prevInputTokens: prevInputTokens.value,
    lastPrompt: lastPrompt.value,
    availableSkills: availableSkills.value,
    isProcessAlive: isProcessAlive.value,
  };
}

// 應用事件處理結果到 Vue refs
function applyEventResult(result: { stateUpdates: Partial<AppState>; actions: EventAction[] }) {
  const { stateUpdates, actions } = result;

  // 應用狀態更新
  if (stateUpdates.sessionId !== undefined) sessionId.value = stateUpdates.sessionId;
  if (stateUpdates.currentModel !== undefined) currentModel.value = stateUpdates.currentModel;
  if (stateUpdates.streamingText !== undefined) streamingText.value = stateUpdates.streamingText;
  if (stateUpdates.messages !== undefined) messages.value = stateUpdates.messages;
  if (stateUpdates.currentToolUses !== undefined) currentToolUses.value = stateUpdates.currentToolUses;
  if (stateUpdates.deniedToolsThisRequest !== undefined) deniedToolsThisRequest.value = stateUpdates.deniedToolsThisRequest;
  if (stateUpdates.pendingPermission !== undefined) pendingPermission.value = stateUpdates.pendingPermission;
  if (stateUpdates.avatarState !== undefined) avatarState.value = stateUpdates.avatarState;
  if (stateUpdates.busyStatus !== undefined) busyStatus.value = stateUpdates.busyStatus;
  if (stateUpdates.isLoading !== undefined) {
    isLoading.value = stateUpdates.isLoading;
    // 互動模式：Complete 事件時處理排隊訊息
    if (!stateUpdates.isLoading && pendingMessage.value) {
      const queued = pendingMessage.value;
      pendingMessage.value = null;
      nextTick().then(() => {
        userInput.value = queued;
        sendMessage();
      });
    }
  }
  if (stateUpdates.isProcessAlive !== undefined) isProcessAlive.value = stateUpdates.isProcessAlive;
  if (stateUpdates.contextUsage !== undefined) contextUsage.value = stateUpdates.contextUsage;
  if (stateUpdates.contextInfo !== undefined) contextInfo.value = stateUpdates.contextInfo;
  if (stateUpdates.prevInputTokens !== undefined) prevInputTokens.value = stateUpdates.prevInputTokens;
  if (stateUpdates.availableSkills !== undefined) {
    availableSkills.value = stateUpdates.availableSkills;
    // 重新載入 skills（合併 init 事件的 skills 和自訂 skills）
    skillsLoaded.value = false;
    loadSkills();
  }

  // 執行副作用動作
  for (const action of actions) {
    switch (action.type) {
      case 'scrollToBottom':
        scrollToBottom();
        break;
      case 'stopBusyTextAnimation':
        stopBusyTextAnimation();
        break;
      case 'startCompleteTimer':
        // 3 秒後回到待機表情
        setTimeout(() => {
          avatarState.value = 'idle';
        }, 3000);
        break;
      case 'addErrorMessage':
        messages.value.push({
          role: 'assistant',
          items: [{ type: 'text', content: `*皺眉* 抱歉，出了點問題：${action.message}` }]
        });
        break;
      case 'showToast':
        showToast(action.message, action.variant || 'info');
        break;
    }
  }
}

// 處理 Claude 事件（使用 claudeEventHandler.ts 的統一邏輯）
function handleClaudeEvent(event: ClaudeEvent) {
  console.log('Claude event:', event.event_type, event);

  // 特殊處理 AskUserQuestion 工具
  if (event.event_type === 'ToolUse' && event.tool_name === 'AskUserQuestion') {
    console.log('🤔 AskUserQuestion detected:', event.input);

    // 從 input 中提取 questions
    const input = event.input as { questions?: Question[] } | undefined;
    if (input?.questions && Array.isArray(input.questions)) {
      pendingQuestion.value = {
        toolId: event.tool_id || '',
        questions: input.questions,
      };
      avatarState.value = 'waiting';
      busyStatus.value = '等待回答...';
      stopBusyTextAnimation();
    }
  }

  // 特殊處理 ExitPlanMode 的 ToolResult：從結果中解析計畫檔案路徑
  if (event.event_type === 'ToolResult' && event.result) {
    // 從 ToolResult 中解析 "Your plan has been saved to: <path>" 格式
    const planPathMatch = event.result.match(/Your plan has been saved to:\s*(.+\.md)/);
    if (planPathMatch) {
      const filePath = planPathMatch[1].trim();
      console.log('📋 Plan file path from ToolResult:', filePath);

      // 找到對應的工具並附加路徑資訊
      const tool = currentToolUses.value.find(t => t.id === event.tool_id && t.name === 'ExitPlanMode');
      if (tool) {
        tool.input = {
          ...(tool.input || {}),
          _planFilePath: filePath,
        };
      }

      // 同時更新 messages 中的工具
      const lastMsg = messages.value[messages.value.length - 1];
      if (lastMsg && lastMsg.role === 'assistant') {
        const toolItem = lastMsg.items.find(
          (item): item is { type: 'tool'; tool: ToolUseItem } =>
            item.type === 'tool' && item.tool.id === event.tool_id && item.tool.name === 'ExitPlanMode'
        );
        if (toolItem) {
          toolItem.tool.input = {
            ...(toolItem.tool.input || {}),
            _planFilePath: filePath,
          };
        }
      }
    }
  }

  // 特殊處理 AskUserQuestion 的 ToolResult (is_error)
  // 這種情況發生在 Hook 機制自動允許 AskUserQuestion 後，Claude CLI 因為沒有收到用戶輸入而返回錯誤
  // 注意：我們不應該在這裡清除 pendingQuestion，因為用戶可能還在回答問題
  // 正確的做法是：讓用戶回答問題，答案會作為 user message 發送
  // 這裡只記錄日誌，不清除 pendingQuestion
  if (event.event_type === 'ToolResult' && event.is_error) {
    const tool = currentToolUses.value.find(t => t.id === event.tool_id);
    if (tool?.name === 'AskUserQuestion') {
      console.log('ℹ️ AskUserQuestion ToolResult with error (expected in fallback mode), keeping pendingQuestion for user to answer');
      // 不清除 pendingQuestion，讓用戶可以繼續回答
      // 用戶回答後，答案會作為 user message 發送給 Claude
    }
  }

  // 取得當前狀態並處理事件
  const state = getCurrentAppState();
  const result = processClaudeEvent(event, state);

  // 應用結果
  applyEventResult(result);

  // Complete 事件時檢查是否有 <memory-update> 標籤（自動記憶提取）
  if (event.event_type === 'Complete') {
    extractAndSaveMemories();
  }
}

/**
 * 從最後一個 assistant 訊息中提取並儲存記憶
 * 格式：<memory-update>
 * - [type:experience] 記憶內容
 * - [type:milestone] 另一個記憶
 * </memory-update>
 */
async function extractAndSaveMemories() {
  // 取得最後一個 assistant 訊息的文字內容
  const lastMsg = messages.value[messages.value.length - 1];
  if (!lastMsg || lastMsg.role !== 'assistant') return;

  const textContent = lastMsg.items
    .filter((item): item is { type: 'text'; content: string } => item.type === 'text')
    .map(item => item.content)
    .join('\n');

  // 檢查是否有 <memory-update> 標籤
  const memoryMatch = textContent.match(/<memory-update>([\s\S]*?)<\/memory-update>/);
  if (!memoryMatch) return;

  const memoryBlock = memoryMatch[1];
  console.log('📝 Found memory-update block:', memoryBlock);

  // 解析每一行記憶
  // 格式：- [type:experience] 記憶內容
  const memoryLines = memoryBlock.split('\n').filter(line => line.trim().startsWith('-'));

  for (const line of memoryLines) {
    // 解析 [type:xxx] 和內容
    const typeMatch = line.match(/\[type:(\w+)\]\s*(.+)/);
    if (typeMatch) {
      const memoryType = typeMatch[1]; // milestone, experience, growth, emotional
      const content = typeMatch[2].trim();

      try {
        await invoke('write_memory', {
          content,
          memoryType,
          source: 'auto',  // 自動提取
        });
        console.log(`💾 Auto-saved memory [${memoryType}]:`, content);
      } catch (error) {
        console.error('Failed to save auto memory:', error);
      }
    } else {
      // 沒有 type 標記，使用預設 experience
      const content = line.replace(/^-\s*/, '').trim();
      if (content) {
        try {
          await invoke('write_memory', {
            content,
            memoryType: 'experience',
            source: 'auto',
          });
          console.log('💾 Auto-saved memory [experience]:', content);
        } catch (error) {
          console.error('Failed to save auto memory:', error);
        }
      }
    }
  }
}

// Hook 權限請求事件類型
interface PermissionRequestEvent {
  tool_name: string;
  tool_input: Record<string, unknown>;
  tool_use_id: string;
  session_id?: string;
}

// ExitPlanMode 專用事件類型
interface PlanApprovalEvent {
  tool_use_id: string;
  plan_content?: string;
  plan_file_path?: string;
}

// 設定事件監聯
async function setupEventListeners() {
  unlistenClaude = await listen<ClaudeEvent>('claude-event', (event) => {
    handleClaudeEvent(event.payload);
  });

  // 監聽 Permission HTTP Server 發送的權限請求（Hook 機制）
  unlistenPermissionRequest = await listen<PermissionRequestEvent>('permission-request', (event) => {
    console.log('🔔 Permission request from Hook:', event.payload);
    const { tool_name, tool_input, tool_use_id, session_id } = event.payload;

    // 設定待確認的權限，標記為來自 Hook
    pendingPermission.value = {
      toolName: tool_name,
      toolId: tool_use_id,
      input: tool_input,
      isFromHook: true,
      sessionId: session_id,
    };
    avatarState.value = 'waiting';
    busyStatus.value = '等待確認...';
  });

  // 監聯 ExitPlanMode 專用事件
  unlistenPlanApproval = await listen<PlanApprovalEvent>('plan-approval-request', (event) => {
    console.log('📋 Plan approval request:', event.payload);
    const { tool_use_id, plan_content, plan_file_path } = event.payload;

    // 設定待確認的權限（ExitPlanMode），標記為來自 Hook
    pendingPermission.value = {
      toolName: 'ExitPlanMode',
      toolId: tool_use_id,
      input: {
        plan: plan_content,
        _planFilePath: plan_file_path,
      },
      isFromHook: true,
    };
    avatarState.value = 'waiting';
    busyStatus.value = '等待確認計畫...';
  });
}

// 全域快捷鍵處理
function handleGlobalKeydown(e: KeyboardEvent) {
  const isMac = navigator.userAgent.includes('Mac');
  const ctrlOrCmd = isMac ? e.metaKey : e.ctrlKey;

  // Ctrl/Cmd + N: 新增標籤頁
  if (ctrlOrCmd && !e.shiftKey && e.key === 'n') {
    e.preventDefault();
    // 先保存當前標籤頁狀態
    saveCurrentTabState();
    // 建立新標籤頁
    tabManager.createTab();
    // 恢復新標籤頁狀態（空的新對話）
    restoreTabState();
    return;
  }

  // Ctrl/Cmd + Shift + N: 當前標籤頁開始新對話
  if (ctrlOrCmd && e.shiftKey && e.key === 'N') {
    e.preventDefault();
    handleNewConversation();
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
      // 如果輸入框只有 "/" 開頭的內容，清空
      if (userInput.value.startsWith('/') && !userInput.value.includes(' ')) {
        userInput.value = '';
      }
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

// === SessionSelector 事件處理器 ===

// 切換標籤頁
function handleSwitchTab(tabId: string) {
  // 先保存當前標籤頁的狀態
  saveCurrentTabState();

  // 切換到新標籤頁
  tabManager.switchTab(tabId);

  // 恢復新標籤頁的狀態
  restoreTabState();
}

// 關閉標籤頁
function handleCloseTab(tabId: string) {
  // 如果關閉的是當前標籤頁，先保存狀態
  if (tabId === tabManager.activeTabId.value) {
    // tabManager.closeTab 會自動切換到其他標籤頁
    tabManager.closeTab(tabId);
    // 恢復新的活躍標籤頁狀態
    restoreTabState();
  } else {
    // 關閉其他標籤頁，不需要切換狀態
    tabManager.closeTab(tabId);
  }
}

// 開始新對話（清除當前標籤頁）
function handleNewConversation() {
  tabManager.clearCurrentTab();

  // 互動模式：中斷舊 process（新對話需要新 session）
  markProcessForRespawn();

  // 同步到 App.vue 狀態
  messages.value = [
    {
      role: 'assistant',
      items: [{ type: 'text', content: '好，我們開始新的對話吧！有什麼需要幫忙的嗎？ *推眼鏡*' }]
    }
  ];
  sessionId.value = null;
  streamingText.value = '';
  currentToolUses.value = [];
  deniedToolsThisRequest.value.clear();
  contextUsage.value = null;
  contextInfo.value = null;
  lastPrompt.value = '';
  isLoading.value = false;
  pendingPermission.value = null;
  avatarState.value = 'idle';

  // 聚焦到輸入框
  const textarea = document.querySelector('textarea');
  if (textarea) {
    textarea.focus();
  }
}

// 歷史訊息類型（對應 Rust 端的 HistoryMessage）
// 注意：Rust 使用 #[serde(rename = "type")]，所以 JSON 中的字段名是 "type" 不是 "tool_type"
interface HistoryMessage {
  role: string;
  items: Array<
    | { type: 'text'; content: string }
    | { type: 'tool'; tool: {
        id: string;
        name: string;
        type: string;
        input: unknown;
        result?: string;
        structured_patch?: DiffHunk[];
        is_error?: boolean;
        image_base64?: string;
        image_media_type?: string;
      }
    }
  >;
}

// 從歷史對話開啟
async function handleOpenHistory(sessionId_: string, summary: string | null) {
  // 先保存當前標籤頁狀態
  saveCurrentTabState();

  // 在新標籤頁中開啟歷史對話
  const newTab = tabManager.openFromHistory(sessionId_, summary);

  // 設定 App.vue 狀態
  sessionId.value = sessionId_;
  streamingText.value = '';
  currentToolUses.value = [];
  deniedToolsThisRequest.value.clear();
  contextUsage.value = null;
  contextInfo.value = null;

  // 嘗試載入歷史訊息
  try {
    if (workingDir.value) {
      const historyMessages = await invoke<HistoryMessage[]>('load_session_history', {
        workingDir: workingDir.value,
        sessionId: sessionId_,
      });

      if (historyMessages && historyMessages.length > 0) {
        // 將歷史訊息轉換為 App.vue 的 Message 格式
        messages.value = historyMessages.map(msg => ({
          role: msg.role as 'user' | 'assistant',
          items: msg.items.map(item => {
            if (item.type === 'text') {
              // 檢測是否為 skill 內容
              const skillMatch = item.content.match(/^Base directory for this skill: (.+?)\n\n# (.+?)\r?\n/);
              if (skillMatch) {
                return {
                  type: 'text' as const,
                  content: item.content,
                  isSkill: true,
                  skillDir: skillMatch[1],
                  skillName: skillMatch[2],
                };
              }
              return { type: 'text' as const, content: item.content };
            } else {
              return {
                type: 'tool' as const,
                tool: {
                  id: item.tool.id,
                  type: item.tool.type,
                  name: item.tool.name,
                  input: item.tool.input as Record<string, unknown>,
                  result: item.tool.result,
                  // VS Code 風格 Diff View（歷史訊息）
                  structuredPatch: item.tool.structured_patch,
                  // 工具執行是否失敗（歷史訊息）
                  isCancelled: item.tool.is_error,
                  // 圖片結果（歷史訊息）
                  imageBase64: item.tool.image_base64,
                  imageMediaType: item.tool.image_media_type,
                }
              };
            }
          })
        }));
      } else {
        // 沒有歷史訊息，顯示預設訊息
        messages.value = [...newTab.messages];
      }
    } else {
      messages.value = [...newTab.messages];
    }
  } catch (error) {
    console.error('Failed to load session history:', error);
    // 載入失敗，使用預設訊息
    messages.value = [...newTab.messages];
  }

  await scrollToBottom();
}

// 保存當前標籤頁狀態到 tabManager
function saveCurrentTabState() {
  const tab = tabManager.activeTab.value;
  if (!tab) return;

  tab.sessionId = sessionId.value;
  tab.messages = [...messages.value];
  tab.streamingText = streamingText.value;
  tab.currentToolUses = [...currentToolUses.value];
  tab.deniedToolsThisRequest = new Set(deniedToolsThisRequest.value);
  tab.contextUsage = contextUsage.value;
  tab.contextInfo = contextInfo.value;
  tab.lastPrompt = lastPrompt.value;
  tab.isLoading = isLoading.value;
  tab.pendingPermission = pendingPermission.value;
  tab.avatarState = avatarState.value;
}

// 從 tabManager 恢復標籤頁狀態
function restoreTabState() {
  const tab = tabManager.activeTab.value;
  if (!tab) return;

  sessionId.value = tab.sessionId;
  messages.value = [...tab.messages];
  streamingText.value = tab.streamingText;
  currentToolUses.value = [...tab.currentToolUses];
  deniedToolsThisRequest.value = new Set(tab.deniedToolsThisRequest);
  contextUsage.value = tab.contextUsage;
  contextInfo.value = tab.contextInfo;
  lastPrompt.value = tab.lastPrompt;
  isLoading.value = tab.isLoading;
  pendingPermission.value = tab.pendingPermission;
  avatarState.value = tab.avatarState;

  // 滾動到底部
  scrollToBottom();
}

// 載入標籤頁的歷史訊息（如果有 sessionId 的話）
async function loadTabHistory(tabSessionId: string) {
  if (!workingDir.value || !tabSessionId) return;

  try {
    const historyMessages = await invoke<HistoryMessage[]>('load_session_history', {
      workingDir: workingDir.value,
      sessionId: tabSessionId,
    });

    if (historyMessages && historyMessages.length > 0) {
      // 將歷史訊息轉換為 App.vue 的 Message 格式
      messages.value = historyMessages.map(msg => ({
        role: msg.role as 'user' | 'assistant',
        items: msg.items.map(item => {
          if (item.type === 'text') {
            // 檢測是否為 skill 內容
            const skillMatch = item.content.match(/^Base directory for this skill: (.+?)\n\n# (.+?)\r?\n/);
            if (skillMatch) {
              return {
                type: 'text' as const,
                content: item.content,
                isSkill: true,
                skillDir: skillMatch[1],
                skillName: skillMatch[2],
              };
            }
            return { type: 'text' as const, content: item.content };
          } else {
            return {
              type: 'tool' as const,
              tool: {
                id: item.tool.id,
                type: item.tool.type,
                name: item.tool.name,
                input: item.tool.input as Record<string, unknown>,
                result: item.tool.result,
                // VS Code 風格 Diff View（歷史訊息）
                structuredPatch: item.tool.structured_patch,
                // 工具執行是否失敗（歷史訊息）
                isCancelled: item.tool.is_error,
                // 圖片結果（歷史訊息）
                imageBase64: item.tool.image_base64,
                imageMediaType: item.tool.image_media_type,
              }
            };
          }
        })
      }));
      await scrollToBottom();
    }
  } catch (error) {
    console.error('Failed to load tab history:', error);
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

    // 初始化 Tab Manager
    await tabManager.initialize(dir);

    // 恢復當前標籤頁的狀態
    restoreTabState();

    // 如果當前標籤頁有 sessionId，載入歷史訊息
    const activeTab = tabManager.activeTab.value;
    if (activeTab?.sessionId) {
      sessionId.value = activeTab.sessionId;
      await loadTabHistory(activeTab.sessionId);
    }
  } catch (error) {
    console.error('Failed to get working directory:', error);
  }

  // 開始輪詢 IDE 狀態
  startIdeStatusPolling();

  // 附加組件狀態偵測（同時用於首次啟動判斷和狀態列顯示）
  await refreshAddonStatus();
  try {
    const setupDone = await invoke<boolean>('check_setup_done');
    if (!setupDone) {
      showSetupWizard.value = true;
    }
  } catch (e) {
    console.error('Failed to check setup status:', e);
  }
});

// 元件卸載時清理
onUnmounted(() => {
  if (unlistenClaude) {
    unlistenClaude();
  }
  if (unlistenPermissionRequest) {
    unlistenPermissionRequest();
  }
  if (unlistenPlanApproval) {
    unlistenPlanApproval();
  }
  stopBusyTextAnimation();
  stopIdeStatusPolling();

  // 移除全域快捷鍵
  window.removeEventListener('keydown', handleGlobalKeydown);
});

// 確保 CLI process 已啟動（互動模式）
// 如果 process 不存在或已退出，啟動新的 process
async function ensureProcess(): Promise<void> {
  if (isProcessAlive.value) return;

  const permissionMode = editMode.value === 'default' ? null : editMode.value;

  await invoke('start_claude', {
    workingDir: workingDir.value,
    sessionId: sessionId.value || null,
    permissionMode: permissionMode,
    extendedThinking: extendedThinking.value || null,
  });

  isProcessAlive.value = true;
}

// 送出訊息（核心函數，支援 allowedTools）
// 互動模式：先確保 process 存在，再透過 stdin 送訊息（非阻塞）
async function sendMessageCore(content: string, _extraAllowedTools: string[] = []) {
  // 開始載入狀態
  isLoading.value = true;
  avatarState.value = 'thinking';
  startBusyTextAnimation();
  streamingText.value = '';
  currentToolUses.value = [];  // 清空當前請求的工具追蹤（舊的已保存在 messages 中）
  deniedToolsThisRequest.value.clear();  // 清空這次請求累積的被拒絕工具

  try {
    // 確保 CLI process 已啟動
    await ensureProcess();

    // 透過 stdin 送訊息給長駐的 CLI process（非阻塞，立即返回）
    await invoke('send_message', {
      message: content,
    });
    // 注意：不再 await CLI 完成，回應透過 claude-event 事件串流回來
    // 排隊訊息的處理已移到 applyEventResult 的 Complete 事件中
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

// /remember 指令處理：儲存記憶到後端
async function handleRememberCommand(content: string) {
  try {
    // 顯示用戶訊息
    messages.value.push({
      role: 'user',
      items: [{ type: 'text', content: `/remember ${content}` }]
    });

    // 呼叫後端儲存記憶
    await invoke('write_memory', {
      content: content,
      memoryType: 'experience',  // 預設為共同經歷
      source: 'manual',
    });

    // 顯示確認訊息
    messages.value.push({
      role: 'assistant',
      items: [{ type: 'text', content: `好，我記住了 ♡\n\n*把「${content}」記在心裡*` }]
    });

    await scrollToBottom();
  } catch (error) {
    console.error('Failed to save memory:', error);
    messages.value.push({
      role: 'assistant',
      items: [{ type: 'text', content: `*皺眉* 記憶儲存失敗：${error}` }]
    });
  }
}

// 送出訊息（從輸入框）
async function sendMessage() {
  const content = userInput.value.trim();
  if (!content) return;

  // Claude 正在工作時，將訊息排隊等待
  if (isLoading.value) {
    pendingMessage.value = content;
    userInput.value = '';
    return;
  }

  // /remember 指令攔截：直接儲存記憶，不送給 Claude
  if (content.startsWith('/remember ')) {
    const memoryContent = content.substring('/remember '.length).trim();
    if (memoryContent) {
      await handleRememberCommand(memoryContent);
    } else {
      messages.value.push({
        role: 'assistant',
        items: [{ type: 'text', content: '欸，要記什麼呢？請在 /remember 後面加上想讓我記住的內容。' }]
      });
    }
    userInput.value = '';
    return;
  }

  // 組合最終 prompt：IDE 選取 + 文字 + 圖片路徑
  let finalContent = content;

  // 自動附加 IDE 選取內容（如果有的話）
  // 只有當來源 IDE 的 workspace_path 與目前工作目錄一致時才附加
  const ideCtx = ideStatus.value?.current_context;
  let selectedText = ideCtx?.selected_text;

  // 檢查來源 IDE 是否為同專案
  if (selectedText && selectedText.trim() && ideCtx?.client_id) {
    const sourceClient = ideStatus.value?.connected_clients?.find(
      c => c.id === ideCtx.client_id
    );

    // 如果找到來源 client 且有 workspace_path，檢查是否為同專案
    if (sourceClient?.workspace_path && workingDir.value) {
      const clientWorkspace = sourceClient.workspace_path.toLowerCase().replace(/\\/g, '/');
      const currentDir = workingDir.value.toLowerCase().replace(/\\/g, '/');

      // 不是同專案則不附加
      const isSameProject = currentDir === clientWorkspace ||
                           currentDir.startsWith(clientWorkspace + '/') ||
                           clientWorkspace.startsWith(currentDir + '/');
      if (!isSameProject) {
        selectedText = null;
      }
    }
  }

  if (selectedText && selectedText.trim()) {
    const filePath = ideCtx?.file_path || 'unknown';
    const selection = ideCtx?.selection;
    let lineInfo = '';
    if (selection) {
      const startLine = selection.start_line + 1;
      const endLine = selection.end_line + 1;
      lineInfo = startLine === endLine ? `line ${startLine}` : `lines ${startLine} to ${endLine}`;
    }
    // 用 <ide_selection> 標籤包裹，讓 Claude 知道這是來自 IDE 的選取
    finalContent = `<ide_selection>The user selected ${lineInfo} from ${filePath}:\n${selectedText}</ide_selection>\n\n${content}`;
  }

  if (attachedImages.value.length > 0) {
    // 附加圖片路徑到 prompt
    const imagePaths = attachedImages.value.map(img => img.path).join(' ');
    finalContent = `${finalContent} ${imagePaths}`;
  }

  // 記住這次的 prompt（用於權限確認後重新執行）
  lastPrompt.value = finalContent;

  // 加入使用者訊息（顯示用，包含圖片和 IDE 選取標記）
  let displayContent = content;
  if (selectedText && selectedText.trim()) {
    const fileName = ideCtx?.file_path?.split(/[\\/]/).pop() || 'file';
    displayContent = `[IDE: ${fileName}]\n${content}`;
  }
  if (attachedImages.value.length > 0) {
    displayContent += `\n[附加 ${attachedImages.value.length} 張圖片]`;
  }
  messages.value.push({
    role: 'user',
    items: [{ type: 'text', content: displayContent }]
  });
  userInput.value = '';

  // 清除附加圖片（已包含在發送的 prompt 中）
  // 不需要清理臨時檔案，Claude 還需要讀取它們
  attachedImages.value = [];

  await scrollToBottom();

  // 自動生成標籤頁標題（根據第一則用戶訊息）
  if (tabManager.activeTabId.value) {
    tabManager.autoGenerateTitle(tabManager.activeTabId.value);
  }

  await sendMessageCore(finalContent);
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
    if (e.key === 'Tab' || (e.key === 'Enter' && !e.shiftKey && !e.altKey)) {
      // Tab 或純 Enter 選擇檔案，Shift+Enter/Alt+Enter 讓它換行
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

  // Enter 發送，Shift+Enter 或 Alt+Enter（Mac: Option+Enter）換行
  if (e.key === 'Enter' && !e.shiftKey && !e.altKey) {
    e.preventDefault();
    sendMessage();
  }
}

// 處理剪貼簿貼上（Ctrl+V）- 使用 Web API
async function handlePaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items;
  if (!items) return;

  // 找到圖片項目
  let imageFile: File | null = null;
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      imageFile = item.getAsFile();
      break;
    }
  }

  if (!imageFile) return;

  // 阻止預設貼上行為（不要把圖片內容貼到輸入框）
  e.preventDefault();

  // 產生唯一 ID 和時間戳記
  const id = `img_${++imageIdCounter}_${Date.now()}`;
  const timestamp = new Date().toLocaleTimeString('zh-TW', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const ext = imageFile.type.split('/')[1] || 'png';
  const name = `截圖_${timestamp}.${ext}`;

  // 立即加入 loading 狀態的項目
  attachedImages.value.push({
    id,
    path: '',
    name,
    isTemp: true,
    isLoading: true,
  });

  try {
    // 產生預覽 URL（使用 createObjectURL 更快）
    const previewUrl = URL.createObjectURL(imageFile);
    const item = attachedImages.value.find(img => img.id === id);
    if (item) {
      item.previewUrl = previewUrl;
    }

    // 儲存到臨時檔案
    const arrayBuffer = await imageFile.arrayBuffer();
    const pngData = Array.from(new Uint8Array(arrayBuffer));

    const filePath = await invoke<string>('save_temp_image_png', {
      pngData,
    });

    // 更新項目（移除 loading 狀態）
    if (item) {
      item.path = filePath;
      item.isLoading = false;
    }

    console.log('📷 Image pasted:', filePath);
  } catch (err) {
    // 處理過程中出錯，移除 loading 項目
    console.error('Failed to process clipboard image:', err);
    const index = attachedImages.value.findIndex(img => img.id === id);
    if (index !== -1) {
      attachedImages.value.splice(index, 1);
    }
  }
}

// 移除附加的圖片
async function removeAttachedImage(id: string) {
  const index = attachedImages.value.findIndex(img => img.id === id);
  if (index !== -1) {
    const image = attachedImages.value[index];

    // 如果是臨時檔案，刪除它
    if (image.isTemp) {
      try {
        await invoke('cleanup_temp_image', { filePath: image.path });
      } catch (err) {
        console.error('Failed to cleanup temp image:', err);
      }
    }

    // 從列表中移除
    attachedImages.value.splice(index, 1);
  }
}

// 觸發檔案選擇對話框
function triggerFileInput() {
  fileInputRef.value?.click();
}

// 處理檔案選擇
async function handleFileSelect(e: Event) {
  const input = e.target as HTMLInputElement;
  const files = input.files;
  if (!files || files.length === 0) return;

  for (const file of Array.from(files)) {
    // 只處理圖片檔案
    if (!file.type.startsWith('image/')) continue;

    const id = `img_${++imageIdCounter}`;

    // 建立預覽 URL
    const previewUrl = URL.createObjectURL(file);

    // 先加入 loading 狀態
    attachedImages.value.push({
      id,
      path: '',
      name: file.name,
      isTemp: true,
      previewUrl,
      isLoading: true,
    });

    // 讀取檔案並保存到臨時目錄
    try {
      const arrayBuffer = await file.arrayBuffer();
      const pngData = new Uint8Array(arrayBuffer);
      const filePath = await invoke<string>('save_temp_image_png', {
        pngData: Array.from(pngData)
      });

      // 更新路徑
      const item = attachedImages.value.find(img => img.id === id);
      if (item) {
        item.path = filePath;
        item.isLoading = false;
      }
    } catch (err) {
      console.error('Failed to save selected image:', err);
      // 移除失敗的項目
      const index = attachedImages.value.findIndex(img => img.id === id);
      if (index !== -1) {
        attachedImages.value.splice(index, 1);
      }
    }
  }

  // 重置 input，讓用戶可以再次選擇同一檔案
  input.value = '';
}

// 處理拖曳進入（防止預設行為）
function handleDragOver(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
}

// 處理拖曳放下
async function handleDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();

  const files = e.dataTransfer?.files;
  if (!files || files.length === 0) return;

  // 支援的圖片格式
  const imageTypes = ['image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/bmp'];

  for (const file of Array.from(files)) {
    if (imageTypes.includes(file.type)) {
      // 取得檔案路徑（Tauri 拖曳檔案會提供路徑）
      // 注意：瀏覽器的 File API 不會直接給路徑，但 Tauri 會
      const filePath = (file as File & { path?: string }).path;

      if (filePath) {
        // 產生唯一 ID
        const id = `img_${++imageIdCounter}_${Date.now()}`;

        // 建立預覽 URL
        const previewUrl = URL.createObjectURL(file);

        // 加入附加圖片列表
        attachedImages.value.push({
          id,
          path: filePath,
          name: file.name,
          isTemp: false,  // 不是臨時檔案，不需要清理
          previewUrl,
        });

        console.log('📷 Image dropped:', filePath);
      }
    }
  }
}

// @-Mention 和 /-Command: 處理輸入變化
async function handleInput(e: Event) {
  const target = e.target as HTMLTextAreaElement;
  const value = target.value;
  const cursorPos = target.selectionStart || 0;

  // 斜線選單：當輸入框只有 "/" 開頭時自動打開
  if (value.startsWith('/') && !showSlashMenu.value) {
    showSlashMenu.value = true;
    slashFilter.value = value; // 包含 "/"，會被 filteredSkills 處理
    loadSkills();
    return;
  }

  // 如果斜線選單開啟中，更新過濾文字
  if (showSlashMenu.value && value.startsWith('/')) {
    slashFilter.value = value;
    return;
  }

  // 如果輸入不再以 "/" 開頭，關閉斜線選單
  if (showSlashMenu.value && !value.startsWith('/')) {
    showSlashMenu.value = false;
  }

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

// 過濾同專案的 IDE 連接
// 只顯示 workspace_path 包含當前工作目錄的 IDE，或未報告 workspace 的 IDE（向後相容）
const filteredIdeClients = computed(() => {
  if (!ideStatus.value?.connected_clients) return [];
  if (!workingDir.value) return ideStatus.value.connected_clients;

  const currentDir = workingDir.value.toLowerCase().replace(/\\/g, '/');

  return ideStatus.value.connected_clients.filter(client => {
    // 未報告 workspace 的客戶端預設顯示（向後相容）
    if (!client.workspace_path) return true;

    // 正規化路徑比較
    const clientWorkspace = client.workspace_path.toLowerCase().replace(/\\/g, '/');

    // 檢查是否為同一個專案（路徑相同或其中一個是另一個的子目錄）
    return currentDir === clientWorkspace ||
           currentDir.startsWith(clientWorkspace + '/') ||
           clientWorkspace.startsWith(currentDir + '/');
  });
});

// 計算 IDE 連接狀態文字
const ideConnectionText = computed(() => {
  if (!ideStatus.value) return 'IDE: —';
  if (!ideStatus.value.running) return 'IDE: Off';
  const clients = filteredIdeClients.value;
  if (clients.length === 0) return 'IDE: Waiting';
  if (clients.length === 1) return `IDE: ${clients[0].name}`;
  return `IDE: ${clients.length} connected`;
});

// 檢查 context 來源是否為同專案的 IDE
const isContextFromSameProject = computed(() => {
  const ctx = ideStatus.value?.current_context;
  if (!ctx?.client_id) return true;  // 未知來源預設顯示（向後相容）
  if (!workingDir.value) return true;

  const sourceClient = ideStatus.value?.connected_clients?.find(
    c => c.id === ctx.client_id
  );

  // 找不到來源 client 或未報告 workspace 則預設顯示
  if (!sourceClient?.workspace_path) return true;

  const clientWorkspace = sourceClient.workspace_path.toLowerCase().replace(/\\/g, '/');
  const currentDir = workingDir.value.toLowerCase().replace(/\\/g, '/');

  return currentDir === clientWorkspace ||
         currentDir.startsWith(clientWorkspace + '/') ||
         clientWorkspace.startsWith(currentDir + '/');
});

// 計算 IDE 當前 context 顯示（類似 CLI 格式：In xxx.py, N lines selected）
// 只有同專案的 IDE 才顯示
const ideContextDisplay = computed(() => {
  if (!isContextFromSameProject.value) return null;
  if (!ideStatus.value?.current_context?.file_path) return null;
  const ctx = ideStatus.value.current_context;
  const fileName = ctx.file_path?.split(/[\\/]/).pop() || '';
  return `In ${fileName}`;
});

// 計算 IDE 選取狀態（顯示選取了幾行）
// 只有同專案的 IDE 才顯示
const ideSelectionDisplay = computed(() => {
  if (!isContextFromSameProject.value) return null;
  const ctx = ideStatus.value?.current_context;
  if (!ctx?.selected_text || !ctx.selected_text.trim()) return null;
  if (!ctx.selection) return 'selected';

  const startLine = ctx.selection.start_line;
  const endLine = ctx.selection.end_line;
  const lineCount = endLine - startLine + 1;
  return lineCount === 1 ? '1 line selected' : `${lineCount} lines selected`;
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

// 載入所有可用的 Slash Commands (Skills)
async function loadSkills() {
  if (skillsLoaded.value) return;

  try {
    // 從 Rust 掃描自訂 skills（~/.claude/skills/ 和 .claude/skills/）
    const customSkills = await invoke<SkillItem[]>('scan_skills', {
      workingDir: workingDir.value
    });

    // 從 init 事件取得的 skills（Claude CLI 提供的）
    const initSkills: SkillItem[] = availableSkills.value.map(name => ({
      name,
      description: '',  // init 事件沒有提供描述
      source: 'builtin' as const,
    }));

    // 前端特殊指令（由前端攔截處理，不送給 Claude）
    const frontendCommands: SkillItem[] = [
      {
        name: 'remember',
        description: '記住重要時刻。例如：/remember 今天終於把權限系統做完了！',
        source: 'builtin' as const,
      },
    ];

    // 合併：前端特殊指令 + init skills + custom skills（去重）
    const customNames = new Set(customSkills.map(s => s.name));
    const frontendNames = new Set(frontendCommands.map(s => s.name));
    const mergedSkills = [
      ...frontendCommands,  // 前端特殊指令優先
      ...initSkills.filter(s => !customNames.has(s.name) && !frontendNames.has(s.name)),
      ...customSkills.filter(s => !frontendNames.has(s.name)),
    ];

    allSkills.value = mergedSkills;
    skillsLoaded.value = true;
    console.log('📋 Loaded skills:', mergedSkills.length, '(init:', initSkills.length, ', custom:', customSkills.length, ')');
  } catch (error) {
    console.error('Failed to load skills:', error);
  }
}

// 過濾後的 Skills 列表
const filteredSkills = computed(() => {
  let filter = slashFilter.value.toLowerCase();
  // 移除開頭的 "/"（用戶習慣輸入 "/commit" 來搜尋）
  if (filter.startsWith('/')) {
    filter = filter.slice(1);
  }
  if (!filter) return allSkills.value;

  return allSkills.value.filter(skill =>
    skill.name.toLowerCase().includes(filter) ||
    skill.description.toLowerCase().includes(filter)
  );
});

// 顯示用的篩選文字（去除 "/" 前綴）
const displaySlashFilter = computed(() => {
  let filter = slashFilter.value;
  if (filter.startsWith('/')) {
    filter = filter.slice(1);
  }
  return filter;
});

// 打開斜線選單時載入 Skills
function openSlashMenu() {
  if (showSlashMenu.value) {
    // 關閉選單
    showSlashMenu.value = false;
    // 如果輸入框只有 "/" 開頭的內容，清空
    if (userInput.value.startsWith('/') && !userInput.value.includes(' ')) {
      userInput.value = '';
    }
  } else {
    // 打開選單
    showSlashMenu.value = true;
    loadSkills();
    // 如果輸入框不是 "/" 開頭，自動插入 "/"
    if (!userInput.value.startsWith('/')) {
      userInput.value = '/' + userInput.value;
    }
    slashFilter.value = userInput.value;
  }
}

// 執行斜線命令
async function executeSlashCommand(command: string) {
  showSlashMenu.value = false;
  userInput.value = '';  // 清空輸入框

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
// 支援兩種模式：
// 1. Hook 模式（isFromHook=true）：直接回應給 Permission Server
// 2. 後處理模式（isFromHook=false）：用 --allowedTools 重新執行（deprecated fallback）
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
  const isFromHook = pendingPermission.value.isFromHook ?? false;
  const hookSessionId = pendingPermission.value.sessionId;
  const originalPrompt = pendingPermission.value.originalPrompt;

  // 清除待確認的權限
  pendingPermission.value = null;

  // ========== Hook 模式（新機制） ==========
  if (isFromHook) {
    console.log('🔔 Using Hook mode for permission response');

    // 決定 behavior
    const isAllow = response === 'yes' || response === 'yes-all' || response === 'yes-always';
    const behavior = isAllow ? 'allow' : 'deny';
    const message = response === 'no'
      ? '使用者拒絕了這個操作'
      : (response === 'custom' ? customMessage : undefined);

    try {
      // 發送決策到 Permission Server
      await invoke('respond_to_permission', {
        toolUseId: toolId,
        behavior,
        message,
      });
      console.log(`✅ Permission response sent: ${behavior}`);

      // 標記舊的工具為已處理（不管是允許還是拒絕）
      // 這樣工具指示燈就不會繼續閃爍
      const tool = currentToolUses.value.find(t => t.id === toolId);
      if (tool) {
        tool.isCancelled = true;
        tool.userResponse = isAllow ? 'Permission granted (will retry)' : 'Permission denied by user';
      }
      const lastMsg = messages.value[messages.value.length - 1];
      if (lastMsg && lastMsg.role === 'assistant') {
        const toolItem = lastMsg.items.find(
          (item): item is { type: 'tool'; tool: ToolUseItem } =>
            item.type === 'tool' && item.tool.id === toolId
        );
        if (toolItem) {
          toolItem.tool.isCancelled = true;
          toolItem.tool.userResponse = isAllow ? 'Permission granted (will retry)' : 'Permission denied by user';
        }
      }

      // 處理白名單
      if (response === 'yes-all' && hookSessionId) {
        // 將工具加入 Session 白名單（後端）
        await invoke('add_to_session_whitelist', {
          sessionId: hookSessionId,
          toolName,
        });
        console.log(`✅ Added ${toolName} to session whitelist`);
      }

      if (response === 'yes-always' && workingDir.value) {
        // 寫入專案級白名單
        await invoke('add_project_permission', {
          workingDir: workingDir.value,
          toolName,
        });
        console.log(`✅ Added ${toolName} to project permissions`);
      }

      // 恢復狀態
      avatarState.value = isAllow ? 'working' : 'idle';
      busyStatus.value = isAllow ? '執行中...' : '已拒絕';

    } catch (error) {
      console.error('❌ Failed to send permission response:', error);
      // 如果失敗，嘗試使用後處理模式作為 fallback
      console.log('⚠️ Falling back to legacy mode');
      await handlePermissionResponseLegacy(response, toolName, toolId, customMessage, originalPrompt);
    }

    return;
  }

  // ========== 後處理模式（Fallback） ==========
  await handlePermissionResponseLegacy(response, toolName, toolId, customMessage, originalPrompt);
}

// 後處理模式：用 --allowedTools 重新執行（deprecated fallback）
async function handlePermissionResponseLegacy(
  response: 'yes' | 'yes-all' | 'yes-always' | 'no' | 'custom',
  toolName: string,
  toolId: string,
  customMessage?: string,
  originalPrompt?: string
) {
  console.log('⚠️ Using legacy post-processing mode');
  // 使用 originalPrompt（如果有）或 fallback 到 lastPrompt
  const promptToRerun = originalPrompt || lastPrompt.value;

  // 預備重新執行：創建新的 assistant 訊息，避免覆蓋之前的回應
  function prepareForRetry() {
    // 標記舊的工具為已處理（將要重試）
    const tool = currentToolUses.value.find(t => t.id === toolId);
    if (tool) {
      tool.isCancelled = true;
      tool.userResponse = 'Permission granted (will retry)';
    }
    const lastMsg = messages.value[messages.value.length - 1];
    if (lastMsg && lastMsg.role === 'assistant') {
      const toolItem = lastMsg.items.find(
        (item): item is { type: 'tool'; tool: ToolUseItem } =>
          item.type === 'tool' && item.tool.id === toolId
      );
      if (toolItem) {
        toolItem.tool.isCancelled = true;
        toolItem.tool.userResponse = 'Permission granted (will retry)';
      }
    }

    // 清空 streamingText
    streamingText.value = '';
    // 創建新的空 assistant 訊息，這樣 handleTextEvent 會在新訊息中追加
    // 而不是覆蓋之前的回應
    messages.value.push({
      role: 'assistant',
      items: []
    });
  }

  switch (response) {
    case 'yes':
      // 單次允許：用 --allowedTools 重新執行同一個請求
      if (promptToRerun) {
        console.log(`Re-executing with allowedTools: ${toolName}`);
        prepareForRetry();
        await sendMessageCore(promptToRerun, [toolName]);
      }
      break;

    case 'yes-all':
      // 本次 session 都允許：將所有這次被拒絕的工具都加入白名單後重新執行
      // 這樣可以一次處理多個被拒絕的工具，避免反覆確認
      for (const deniedTool of deniedToolsThisRequest.value) {
        sessionAllowedTools.value.add(deniedTool);
      }
      console.log(`Added all denied tools to session whitelist:`, [...sessionAllowedTools.value]);
      if (promptToRerun) {
        prepareForRetry();
        await sendMessageCore(promptToRerun);
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
      if (promptToRerun) {
        prepareForRetry();
        await sendMessageCore(promptToRerun);
      }
      break;

    case 'no':
      // 拒絕：更新工具使用記錄，並通知 Claude 提供替代方案
      {
        // 取得工具資訊用於生成提示訊息
        const tool = currentToolUses.value.find(t => t.id === toolId);
        const toolTarget = tool?.input?.file_path || tool?.input?.path || tool?.input?.description || '';

        // 更新 currentToolUses
        if (tool) {
          tool.isCancelled = true;
          tool.userResponse = 'Permission denied by user';
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
            toolItem.tool.userResponse = 'Permission denied by user';
          }
        }

        // 發送拒絕訊息給 Claude，讓它提供替代方案
        const denyMessage = `我拒絕了 ${toolName} 工具對 "${toolTarget}" 的操作權限。請告訴我如何手動完成這個操作，或是提供其他不需要這個權限的替代方案。`;

        messages.value.push({
          role: 'user',
          items: [{ type: 'text', content: denyMessage }]
        });
        await scrollToBottom();

        // 發送訊息給 Claude
        lastPrompt.value = denyMessage;
        await sendMessageCore(denyMessage);
      }
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

// 處理 ExitPlanMode 計畫審核的回應
async function handlePlanApprovalResponse(response: 'approve-auto' | 'approve-manual' | 'keep-planning' | 'custom', customMessage?: string) {
  console.log('📋 Plan approval response:', response, customMessage);

  if (!pendingPermission.value) return;

  const toolId = pendingPermission.value.toolId;
  const isFromHook = pendingPermission.value.isFromHook ?? false;

  // 清除待確認的權限
  pendingPermission.value = null;

  // 轉換為 Permission Server 需要的格式
  let behavior: string;
  let message: string | undefined;

  // 根據官方 Claude CLI 的 ExitPlanMode 選項對應
  switch (response) {
    case 'approve-auto':
      behavior = 'allow';
      message = 'Yes, and auto-accept edits';
      break;
    case 'approve-manual':
      behavior = 'allow';
      message = 'Yes, and manually approve edits';
      break;
    case 'keep-planning':
      behavior = 'deny';
      message = 'No, keep planning';
      break;
    case 'custom':
      behavior = 'deny';
      message = customMessage || 'User provided custom instructions';
      break;
  }

  const isAllowed = behavior === 'allow';

  if (isFromHook) {
    try {
      await invoke('respond_to_permission', {
        toolUseId: toolId,
        behavior,
        message,
      });
      console.log(`✅ Plan approval response sent: ${behavior} - ${message}`);

      // 如果選擇了 auto-accept edits，要實際切換到 acceptEdits 模式
      if (response === 'approve-auto') {
        editMode.value = 'acceptEdits';
        console.log('📝 Switched to acceptEdits mode');
      }

      // 恢復狀態
      avatarState.value = isAllowed ? 'working' : 'idle';
      busyStatus.value = isAllowed ? '執行計畫中...' : '';

    } catch (error) {
      console.error('❌ Failed to send plan approval response:', error);
    }
  } else {
    // 後處理模式（fallback）
    // 注意：在 fallback 模式下，Claude CLI 不會自動繼續執行
    // 我們需要發送一個明確的訊息告訴 Claude 用戶的決定
    // 直接使用上面 switch 中設定好的 message（一定有值）

    // 如果選擇了 auto-accept edits，要實際切換到 acceptEdits 模式
    if (response === 'approve-auto') {
      editMode.value = 'acceptEdits';
      console.log('📝 Switched to acceptEdits mode');
    }

    const userMessage = message!;

    messages.value.push({
      role: 'user',
      items: [{ type: 'text', content: userMessage }]
    });
    await scrollToBottom();
    lastPrompt.value = userMessage;
    await sendMessageCore(userMessage);
  }
}

// 處理 AskUserQuestion 的回應
async function handleQuestionSubmit(answers: Record<string, string>) {
  console.log('🟢 Question answers:', answers);

  if (!pendingQuestion.value) return;

  const toolId = pendingQuestion.value.toolId;
  const formattedAnswer = Object.entries(answers)
    .map(([q, a]) => `${q}: ${a}`)
    .join('\n');

  // 更新工具使用記錄
  const tool = currentToolUses.value.find(t => t.id === toolId);
  if (tool) {
    tool.result = formattedAnswer;
    tool.userAnswered = true;  // 標記用戶已回答，防止 ToolResult 覆蓋
  }

  // 同時更新 messages 中的工具（遍歷所有訊息尋找）
  for (const msg of messages.value) {
    if (msg.role === 'assistant') {
      const toolItem = msg.items.find(
        (item): item is { type: 'tool'; tool: ToolUseItem } =>
          item.type === 'tool' && item.tool.id === toolId
      );
      if (toolItem) {
        toolItem.tool.result = formattedAnswer;
        toolItem.tool.userAnswered = true;
        break;
      }
    }
  }

  // 清除待回答的問題
  pendingQuestion.value = null;

  // 將答案作為使用者訊息發送（讓 Claude 繼續）
  const answerText = Object.entries(answers)
    .map(([q, a]) => `${q}\n→ ${a}`)
    .join('\n\n');

  // 發送答案
  messages.value.push({
    role: 'user',
    items: [{ type: 'text', content: answerText }]
  });
  await scrollToBottom();
  // 注意：不更新 lastPrompt，因為這是對工具的回應，不是新的用戶請求
  // 這樣在 PermissionDenied fallback 模式下重新執行時，會使用原始的用戶請求
  await sendMessageCore(answerText);
}

// 取消 AskUserQuestion
function handleQuestionCancel() {
  console.log('🔴 Question cancelled');

  if (!pendingQuestion.value) return;

  const toolId = pendingQuestion.value.toolId;

  // 更新工具使用記錄為取消
  const tool = currentToolUses.value.find(t => t.id === toolId);
  if (tool) {
    tool.isCancelled = true;
    tool.userResponse = 'Cancelled by user';
  }

  // 同時更新 messages 中的工具
  const lastMsg = messages.value[messages.value.length - 1];
  if (lastMsg && lastMsg.role === 'assistant') {
    const toolItem = lastMsg.items.find(
      (item): item is { type: 'tool'; tool: ToolUseItem } =>
        item.type === 'tool' && item.tool.id === toolId
    );
    if (toolItem) {
      toolItem.tool.isCancelled = true;
      toolItem.tool.userResponse = 'Cancelled by user';
    }
  }

  // 清除待回答的問題
  pendingQuestion.value = null;
  avatarState.value = 'idle';
  isLoading.value = false;
  stopBusyTextAnimation();
}

// 中斷請求
async function interruptRequest() {
  console.log('Interrupt request');

  // 清除待確認的權限和問題
  pendingPermission.value = null;
  pendingQuestion.value = null;

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

  // 互動模式：排隊訊息由 applyEventResult 在 Complete 事件時自動送出
}
</script>

<template>
  <div class="app-container">
    <!-- 標題列 -->
    <header class="app-header">
      <h1>Tsunu Alive</h1>
      <span class="subtitle">阿宇陪你寫程式</span>
      <SessionSelector
        :tabs="tabManager.tabs.value"
        :activeTabId="tabManager.activeTabId.value"
        :historySessions="historySessions"
        :historyLoading="historyLoading"
        @switch-tab="handleSwitchTab"
        @close-tab="handleCloseTab"
        @new-conversation="handleNewConversation"
        @open-history="handleOpenHistory"
        @load-history="loadHistorySessions"
      />
    </header>

    <!-- 主要內容區 -->
    <div class="main-content">
      <!-- 左側：對話區域 -->
      <div class="chat-section">
        <!-- 對話訊息 -->
        <div class="chat-container" ref="chatContainer" @click="handleExternalLinkClick">
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
                  :type="item.tool.type"
                  :path="(item.tool.input?.file_path as string) || (item.tool.input?.path as string)"
                  :description="item.tool.input?.description as string"
                  :input="item.tool.input?.command as string"
                  :output="item.tool.result"
                  :query="(item.tool.input?.query as string) || (item.tool.input?.url as string)"
                  :pattern="item.tool.input?.pattern as string"
                  :prompt="item.tool.input?.prompt as string"
                  :todos="item.tool.input?.todos as any"
                  :questions="item.tool.input?.questions as any"
                  :notebookPath="item.tool.input?.notebook_path as string"
                  :cellId="item.tool.input?.cell_id as string"
                  :newSource="item.tool.input?.new_source as string"
                  :cellType="item.tool.input?.cell_type as string"
                  :editMode="item.tool.input?.edit_mode as string"
                  :shellId="item.tool.input?.shell_id as string"
                  :skill="item.tool.input?.skill as string"
                  :args="item.tool.input?.args as string"
                  :taskId="item.tool.input?.task_id as string"
                  :content="item.tool.input?.content as string"
                  :oldCode="item.tool.input?.old_string as string"
                  :newCode="item.tool.input?.new_string as string"
                  :structuredPatch="item.tool.structuredPatch"
                  :imageBase64="item.tool.imageBase64"
                  :imageMediaType="item.tool.imageMediaType"
                  :rawInput="item.tool.input as Record<string, unknown>"
                  :isRunning="!item.tool.result && !item.tool.isCancelled && !item.tool.imageBase64"
                  :isCancelled="item.tool.isCancelled"
                  :userResponse="item.tool.userResponse"
                  :ideConnected="filteredIdeClients.length > 0"
                />
              </div>

              <!-- Skill 內容（摺疊顯示） -->
              <div
                v-else-if="item.type === 'text' && item.isSkill"
                class="skill-content-wrapper"
              >
                <div class="skill-badge">
                  <span class="skill-icon">📚</span>
                  <span class="skill-name">{{ item.skillName }}</span>
                  <span class="skill-dir">{{ item.skillDir }}</span>
                </div>
              </div>

              <!-- Compact 摘要（可收合） -->
              <div
                v-else-if="item.type === 'compact'"
                class="compact-summary-wrapper"
              >
                <div class="compact-header" @click="($event.currentTarget as HTMLElement)?.parentElement?.classList.toggle('expanded')">
                  <span class="compact-icon">📦</span>
                  <span class="compact-label">對話已壓縮</span>
                  <span class="compact-hint">點擊查看摘要</span>
                  <span class="compact-chevron">▸</span>
                </div>
                <div class="compact-body">
                  <div class="message-content markdown-body" v-html="renderMarkdown(item.summary)"></div>
                </div>
              </div>

              <!-- 文字項目 -->
              <div
                v-else-if="item.type === 'text' && item.content"
                class="text-item-wrapper"
              >
                <div
                  class="message-content markdown-body"
                  v-html="renderMarkdown(item.content)"
                ></div>
                <button
                  class="copy-btn"
                  :class="{ copied: copiedIndex === `${msgIndex}-${itemIndex}` }"
                  @click="copyToClipboard(item.content, `${msgIndex}-${itemIndex}`)"
                  :title="copiedIndex === `${msgIndex}-${itemIndex}` ? '已複製！' : '複製原始文字'"
                >
                  <Check v-if="copiedIndex === `${msgIndex}-${itemIndex}`" :size="14" />
                  <template v-else><Copy :size="14" /> 複製</template>
                </button>
              </div>
            </template>
          </div>

          <!-- ExitPlanMode 專用對話框 -->
          <PlanApprovalDialog
            v-if="pendingPermission && pendingPermission.toolName === 'ExitPlanMode'"
            :plan-content="(pendingPermission.input?.plan as string)"
            :plan-file-path="(pendingPermission.input?._planFilePath as string)"
            @respond="handlePlanApprovalResponse"
          />

          <!-- 一般權限確認對話框 -->
          <PermissionDialog
            v-else-if="pendingPermission"
            :action="pendingPermission.toolName"
            :target="(pendingPermission.input?.file_path as string) || (pendingPermission.input?.path as string) || (pendingPermission.input?.description as string) || ''"
            :summary="(pendingPermission.input?.description as string)"
            :preview="(pendingPermission.input?.command as string)"
            @respond="handlePermissionResponse"
          />

          <!-- AskUserQuestion 對話框 -->
          <!-- 使用 v-else-if 確保與 Permission 對話框互斥顯示 -->
          <AskUserQuestionDialog
            v-else-if="pendingQuestion"
            :questions="pendingQuestion.questions"
            @submit="handleQuestionSubmit"
            @cancel="handleQuestionCancel"
          />

          <!-- 載入中指示器 -->
          <div v-if="isLoading && !pendingPermission && !pendingQuestion" class="message assistant loading">
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
    <div class="input-area" @dragover="handleDragOver" @drop="handleDrop">
      <!-- 忙碌狀態指示器 -->
      <div v-if="isLoading" class="busy-indicator">
        <span class="busy-dot"></span>
        <span class="busy-text">{{ busyStatus }}</span>
      </div>

      <!-- 附加圖片預覽 -->
      <div v-if="attachedImages.length > 0" class="attached-images">
        <ImagePreview
          v-for="image in attachedImages"
          :key="image.id"
          :image="image"
          @remove="removeAttachedImage"
        />
      </div>

      <!-- 輸入框 -->
      <div class="input-wrapper">
        <textarea
          v-model="userInput"
          @keydown="handleKeydown"
          @input="handleInput"
          @paste="handlePaste"
          :placeholder="pendingMessage ? '⏳ 訊息已排隊，等待 Claude 完成...' : 'Type @ to mention files... (Ctrl+V 貼上圖片)'"
          rows="2"
        ></textarea>

        <!-- 排隊中的訊息提示 -->
        <div v-if="pendingMessage" class="pending-indicator">
          <span class="pending-text">⏳ 排隊中：{{ pendingMessage.length > 30 ? pendingMessage.substring(0, 30) + '...' : pendingMessage }}</span>
          <button class="pending-cancel" @click="pendingMessage = null" title="取消排隊">✕</button>
        </div>

        <!-- @-Mention 選單 -->
        <div v-if="showMentionMenu && mentionFiles.length > 0" class="mention-menu">
          <div class="mention-header">
            <File class="mention-icon" :size="16" />
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
            <component :is="file.is_dir ? Folder : File" class="mention-item-icon" :size="16" />
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
            <span class="mode-icon">{{ editModeIcons[editMode] }}</span>
            <span class="mode-label">{{ editModeLabels[editMode] }}</span>
          </button>
          <button class="status-btn working-dir" :title="workingDir || '點擊選擇專案'" @click="selectWorkingDir">
            <FolderOpen class="dir-icon" :size="14" />
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
            class="status-btn ide-status"
            :class="{
              connected: filteredIdeClients.length > 0,
              waiting: ideStatus?.running && filteredIdeClients.length === 0,
              off: !ideStatus?.running
            }"
            :title="ideStatus?.running ? `WebSocket port ${ideStatus?.port}` : 'IDE Server not running'"
          >
            <span class="ide-icon">🔗</span>
            <span class="ide-text">{{ ideConnectionText }}</span>
            <!-- IDE 選取狀態（類似 CLI 格式，點擊可插入 @mention） -->
            <span v-if="ideContextDisplay" class="ide-context clickable" @click.stop="insertIdeContextReference" title="點擊插入檔案參考">{{ ideContextDisplay }}</span>
            <span v-if="ideSelectionDisplay" class="ide-selection">{{ ideSelectionDisplay }}</span>
          </button>
          <button
            v-if="hasUninstalledAddons"
            class="status-btn addon-status"
            @click="showSetupWizard = true"
            title="有附加組件尚未安裝，點擊開啟安裝精靈"
          >
            <span class="addon-icon">⚙</span>
            <span class="addon-text">附加組件</span>
            <span class="addon-dot"></span>
          </button>
          <button class="status-btn context-usage" v-if="contextUsage"
            :class="{ warning: contextUsage !== null && contextUsage >= 80, danger: contextUsage !== null && contextUsage >= 95 }"
            :title="contextInfo ? `Tokens: ${contextInfo.totalTokens?.toLocaleString() || '?'} / ${contextInfo.maxTokens?.toLocaleString() || '?'}${contextInfo.inputTokensThisTurn ? `\nInput: ${contextInfo.inputTokensThisTurn.toLocaleString()} tokens` : ''}${contextInfo.inputTokensDelta !== undefined ? ` (${contextInfo.inputTokensDelta >= 0 ? '+' : ''}${contextInfo.inputTokensDelta.toLocaleString()})` : ''}\n點擊執行 /compact` : '點擊執行 /compact'"
            @click="executeSlashCommand('/compact')">
            <span class="usage-icon">{{ contextUsageIcon }}</span>
            <span class="usage-text">{{ contextUsage !== null ? contextUsage + '% used' : '—' }}</span>
            <span v-if="contextInfo?.inputTokensDelta !== undefined" class="usage-delta"
              :class="{ 'delta-warning': (contextInfo?.inputTokensDelta ?? 0) > 5000 }"
            >({{ contextInfo.inputTokensDelta >= 0 ? '+' : '' }}{{ Math.round((contextInfo.inputTokensDelta ?? 0) / 1000) }}k)</span>
          </button>
        </div>

        <!-- 右側按鈕 -->
        <div class="status-right">
          <button class="status-btn attach-btn" @click="triggerFileInput" title="Attach files (images)">
            <Paperclip class="attach-icon" :size="16" />
          </button>
          <input
            ref="fileInputRef"
            type="file"
            accept="image/*"
            multiple
            style="display: none"
            @change="handleFileSelect"
          />
          <button class="status-btn slash-btn" @click="openSlashMenu" title="Commands">
            <Slash class="slash-icon" :size="16" />
          </button>
          <button
            v-if="!isLoading || userInput.trim()"
            class="status-btn send-btn"
            :class="{ 'queued': isLoading && userInput.trim() }"
            @click="sendMessage"
            :disabled="!userInput.trim()"
            :title="isLoading ? 'Queue message (Enter)' : 'Send (Enter)'"
          >
            <span class="send-icon">{{ isLoading ? '⏳' : '⏎' }}</span>
          </button>
          <button
            v-if="isLoading && !userInput.trim()"
            class="status-btn interrupt-btn"
            @click="interruptRequest"
            title="Interrupt (Esc)"
          >
            <Square class="interrupt-icon" :size="14" />
          </button>
        </div>
      </div>

      <!-- 斜線選單 -->
      <div v-if="showSlashMenu" class="slash-menu">
        <div class="slash-header">
          <Slash class="slash-header-icon" :size="16" />
          <span>Commands</span>
          <span class="slash-header-filter" v-if="displaySlashFilter">{{ displaySlashFilter }}</span>
        </div>

        <!-- 動態載入的 Slash Commands -->
        <div class="slash-section" v-if="filteredSkills.length > 0">
          <div
            v-for="skill in filteredSkills"
            :key="skill.name"
            class="slash-item"
            @click="executeSlashCommand('/' + skill.name)"
          >
            <span class="slash-name">/{{ skill.name }}</span>
            <span class="slash-hint">{{ skill.description }}</span>
            <span v-if="skill.source !== 'builtin'" class="slash-source">{{ skill.source }}</span>
          </div>
        </div>

        <!-- 載入中 -->
        <div v-if="!skillsLoaded" class="slash-section">
          <div class="slash-item loading">載入中...</div>
        </div>

        <!-- 無結果 -->
        <div v-if="skillsLoaded && filteredSkills.length === 0 && slashFilter" class="slash-section">
          <div class="slash-item no-results">找不到符合「{{ slashFilter }}」的命令</div>
        </div>

        <!-- Context 資訊 -->
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

    </div>

    <!-- Toast 通知 -->
    <Transition name="toast">
      <div v-if="toastVisible" :class="['toast', `toast-${toastVariant}`]">
        <span class="toast-message">{{ toastMessage }}</span>
        <button class="toast-close" @click="toastVisible = false">&times;</button>
      </div>
    </Transition>

    <!-- 首次啟動安裝精靈 -->
    <SetupWizard v-if="showSetupWizard" @close="showSetupWizard = false; refreshAddonStatus()" />
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

/* Table 樣式 */
.markdown-body table {
  width: 100%;
  border-collapse: collapse;
  margin: 0.75em 0;
  font-size: 0.9em;
  border-radius: 8px;
  overflow: hidden;
}

.markdown-body thead {
  background-color: rgba(74, 144, 217, 0.2);
}

.markdown-body th {
  padding: 10px 12px;
  text-align: left;
  font-weight: 600;
  color: var(--primary-light);
  border-bottom: 2px solid var(--primary-color);
}

.markdown-body td {
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
}

.markdown-body tbody tr {
  transition: background-color 0.15s;
}

.markdown-body tbody tr:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.markdown-body tbody tr:last-child td {
  border-bottom: none;
}

/* 交替行底色 */
.markdown-body tbody tr:nth-child(even) {
  background-color: rgba(255, 255, 255, 0.02);
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

.header-spacer {
  flex: 1;
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

/* Skill 內容摺疊卡片 */
.skill-content-wrapper {
  margin: 4px 0;
}

.skill-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.15), rgba(147, 51, 234, 0.15));
  border: 1px solid rgba(99, 102, 241, 0.3);
  border-radius: 8px;
  font-size: 0.9em;
}

.skill-icon {
  font-size: 1.1em;
}

.skill-name {
  font-weight: 600;
  color: #a5b4fc;
}

.skill-dir {
  font-size: 0.8em;
  color: #6b7280;
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Compact 摘要（可收合） */
.compact-summary-wrapper {
  margin: 8px 0;
  border: 1px solid rgba(251, 191, 36, 0.3);
  border-radius: 8px;
  background: rgba(251, 191, 36, 0.08);
  overflow: hidden;
}

.compact-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s;
}

.compact-header:hover {
  background: rgba(251, 191, 36, 0.12);
}

.compact-icon {
  font-size: 1.1em;
}

.compact-label {
  font-weight: 600;
  color: #fbbf24;
  font-size: 0.9em;
}

.compact-hint {
  font-size: 0.75em;
  color: var(--text-muted, #a0a0a0);
}

.compact-chevron {
  margin-left: auto;
  color: var(--text-muted);
  transition: transform 0.2s;
  font-size: 0.9em;
}

.compact-summary-wrapper.expanded .compact-chevron {
  transform: rotate(90deg);
}

.compact-body {
  max-height: 0;
  overflow: hidden;
  transition: max-height 0.3s ease;
}

.compact-summary-wrapper.expanded .compact-body {
  max-height: 2000px;
}

.compact-body .message-content {
  padding: 0 14px 12px;
  font-size: 0.85em;
  color: var(--text-muted);
  border-top: 1px solid rgba(251, 191, 36, 0.15);
  padding-top: 10px;
}

/* 文字項目包裝（用於複製按鈕） */
.text-item-wrapper {
  position: relative;
}

.text-item-wrapper:hover .copy-btn {
  opacity: 1;
}

.copy-btn {
  position: absolute;
  bottom: 4px;
  right: 4px;
  height: 28px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background-color: rgba(0, 0, 0, 0.4);
  color: var(--text-muted);
  font-size: 0.85rem;
  cursor: pointer;
  opacity: 0;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
}

.copy-btn:hover {
  background-color: rgba(0, 0, 0, 0.6);
  color: var(--text-color);
}

.copy-btn.copied {
  opacity: 1;
  background-color: rgba(46, 204, 113, 0.3);
  color: #2ecc71;
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

/* 附加圖片預覽區 */
.attached-images {
  display: flex;
  flex-wrap: wrap;
  padding: 8px 12px 0;
  gap: 0;
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

/* 排隊中的訊息指示器 */
.pending-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  font-size: 0.8rem;
  color: var(--primary-color);
  background: rgba(99, 102, 241, 0.08);
  border-radius: 0 0 12px 12px;
  margin-top: -4px;
}

.pending-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pending-cancel {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 2px 4px;
  font-size: 0.75rem;
  border-radius: 4px;
}

.pending-cancel:hover {
  color: var(--text-color);
  background: rgba(255, 255, 255, 0.1);
}

/* 排隊送出按鈕樣式 */
.send-btn.queued {
  opacity: 0.7;
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

@keyframes pulse-warning {
  0%, 100% { opacity: 0.7; }
  50% { opacity: 1; }
}

@keyframes pulse-danger {
  0%, 100% { opacity: 0.5; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.1); }
}

/* Input token 增量指示 */
.usage-delta {
  font-size: 0.7rem;
  opacity: 0.7;
  margin-left: 2px;
}

.usage-delta.delta-warning {
  color: #e67e22;
  opacity: 1;
  font-weight: 600;
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
  color: var(--text-muted);
  margin-left: 8px;
}

.ide-context.clickable {
  cursor: pointer;
  transition: opacity 0.15s;
}

.ide-context.clickable:hover {
  opacity: 0.7;
}

.ide-selection {
  font-family: monospace;
  font-size: 0.75rem;
  background-color: rgba(46, 204, 113, 0.2);
  color: rgba(46, 204, 113, 0.9);
  padding: 1px 6px;
  border-radius: 3px;
  margin-left: 4px;
}

/* 附加組件按鈕 */
.addon-status {
  color: #ff9f43;
  position: relative;
}

.addon-status:hover {
  color: #ffb86c;
}

.addon-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #ff9f43;
  display: inline-block;
  margin-left: 2px;
  animation: addon-pulse 2s ease-in-out infinite;
}

@keyframes addon-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
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

.slash-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  font-size: 0.8rem;
  color: var(--text-muted);
}

.slash-header-icon {
  font-size: 1rem;
  font-weight: bold;
  color: var(--primary-light);
}

.slash-header-filter {
  margin-left: auto;
  font-family: monospace;
  background-color: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
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
  flex: 1;
  text-align: right;
  margin-left: 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.slash-name {
  font-family: monospace;
  color: var(--primary-light);
  flex-shrink: 0;
}

.slash-source {
  font-size: 0.7rem;
  padding: 2px 6px;
  border-radius: 4px;
  background-color: var(--primary-dark);
  color: var(--text-color);
  margin-left: 8px;
  flex-shrink: 0;
}

.slash-item.loading,
.slash-item.no-results {
  justify-content: center;
  color: var(--text-muted);
  cursor: default;
}

.slash-item.loading:hover,
.slash-item.no-results:hover {
  background-color: transparent;
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

/* Toast 通知 */
.toast {
  position: fixed;
  bottom: 100px;
  left: 50%;
  transform: translateX(-50%);
  padding: 12px 20px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 12px;
  z-index: 1000;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  max-width: 80%;
}

.toast-info {
  background-color: var(--primary-color);
  color: #fff;
}

.toast-warning {
  background-color: #f39c12;
  color: #000;
}

.toast-error {
  background-color: #e74c3c;
  color: #fff;
}

.toast-message {
  flex: 1;
  font-size: 0.9rem;
}

.toast-close {
  background: none;
  border: none;
  color: inherit;
  font-size: 1.2rem;
  cursor: pointer;
  opacity: 0.7;
  padding: 0;
  line-height: 1;
}

.toast-close:hover {
  opacity: 1;
}

/* Toast 動畫 */
.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(20px);
}

</style>
