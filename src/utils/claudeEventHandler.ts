/**
 * Claude 事件處理邏輯
 * 提取自 App.vue，用於測試和重用
 */

import { isAutoAllowTool } from '../constants/autoAllowTools';

// structuredPatch 的 hunk 類型（Edit 工具的差異資訊）
export interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: string[];  // 每行開頭：' '=未變更, '-'=刪除, '+'=新增
}

// Claude CLI 事件類型
export interface ClaudeEvent {
  event_type: 'Init' | 'Text' | 'ToolUse' | 'ToolResult' | 'Complete' | 'Error' | 'Connected' | 'PermissionDenied' | 'Compacted' | 'ProcessExited';
  session_id?: string;
  model?: string;
  // 可用的 Skills（Init 事件）
  slash_commands?: string[];
  text?: string;
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
  input_tokens_this_turn?: number;
  // Edit 工具的結構化差異（ToolResult 事件）
  structured_patch?: DiffHunk[];
  // 圖片結果的 base64 資料（Read 工具讀取圖片時，ToolResult 事件）
  image_base64?: string;
  image_media_type?: string;  // 例如 'image/png', 'image/jpeg'
  // Compact 摘要（Compacted 事件）
  summary?: string;
}

// 待確認的權限請求
export interface PendingPermission {
  toolName: string;
  toolId: string;
  input: Record<string, unknown>;
  isFromHook?: boolean;  // 是否來自 PermissionRequest Hook（新機制）
  sessionId?: string;    // Session ID（用於 Hook 模式的白名單管理）
  originalPrompt?: string;  // 觸發這個權限請求的原始 prompt（用於 fallback 模式重新執行）
}

// 工具使用項目
export interface ToolUseItem {
  id: string;
  type: string;
  name: string;
  input: Record<string, unknown>;
  result?: string;
  isCancelled?: boolean;
  userResponse?: string;
  userAnswered?: boolean;  // 標記用戶已回答（AskUserQuestion）
  structuredPatch?: DiffHunk[];  // Edit 工具的結構化差異
  imageBase64?: string;  // 圖片結果的 base64 資料（Read 工具讀取圖片時）
  imageMediaType?: string;  // 圖片 MIME 類型，例如 'image/png'
}

// 對話項目（文字或工具，按時間順序）
export type ChatItem =
  | { type: 'text'; content: string; isSkill?: boolean; skillName?: string; skillDir?: string }
  | { type: 'tool'; tool: ToolUseItem }
  | { type: 'compact'; summary: string };

// 訊息類型
export interface Message {
  role: 'user' | 'assistant';
  items: ChatItem[];
}

// 阿宇的表情狀態
export type AvatarState =
  | 'idle'          // 待機（有眨眼動畫）
  | 'thinking'      // 思考中
  | 'working'       // 工作中（有連續動畫）
  | 'complete'      // 完成（比讚）
  | 'planApproved'  // 計畫批准（OK 手勢）
  | 'waiting'       // 等待選擇
  | 'error';        // 出錯

// 編輯模式
export type EditMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan';

// Context 詳細資訊
export interface ContextInfo {
  totalTokens?: number;
  maxTokens?: number;
  inputTokensThisTurn?: number;  // 本次 turn 的 input tokens
  inputTokensDelta?: number;     // 與上次 turn 的 input tokens 差量
}

// 應用狀態（用於事件處理）
export interface AppState {
  sessionId: string | null;
  currentModel: string;
  streamingText: string;
  messages: Message[];
  currentToolUses: ToolUseItem[];
  deniedToolsThisRequest: Set<string>;
  pendingPermission: PendingPermission | null;
  avatarState: AvatarState;
  busyStatus: string;
  isLoading: boolean;
  editMode: EditMode;
  // Context 相關
  contextUsage: number | null;
  contextInfo: ContextInfo | null;
  // 上一次 turn 的 input tokens（用於計算增量）
  prevInputTokens: number | null;
  // 最後一次用戶主動發起的請求（用於 fallback 模式重新執行）
  lastPrompt: string;
  // 可用的 Skills（從 init 事件取得）
  availableSkills: string[];
  // 互動模式：CLI process 是否存活
  isProcessAlive: boolean;
}

// 事件處理結果
export interface EventHandlerResult {
  stateUpdates: Partial<AppState>;
  actions: EventAction[];
}

// 可能的副作用動作
export type EventAction =
  | { type: 'scrollToBottom' }
  | { type: 'stopBusyTextAnimation' }
  | { type: 'startCompleteTimer' }
  | { type: 'addErrorMessage'; message: string }
  | { type: 'showToast'; message: string; variant?: 'info' | 'warning' | 'error' };

/**
 * 處理 Init 事件
 */
export function handleInitEvent(
  event: ClaudeEvent,
  _state: AppState
): EventHandlerResult {
  const stateUpdates: Partial<AppState> = {
    busyStatus: 'Thinking...',
  };

  if (event.session_id) {
    stateUpdates.sessionId = event.session_id;
  }
  if (event.model) {
    stateUpdates.currentModel = event.model;
  }
  // 更新可用的 Skills（從 init 事件的 slash_commands 取得）
  if (event.slash_commands) {
    stateUpdates.availableSkills = event.slash_commands;
  }

  return { stateUpdates, actions: [] };
}

/**
 * 處理 Text 事件
 */
export function handleTextEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  if (!event.text) {
    return { stateUpdates: {}, actions: [] };
  }

  const newStreamingText = state.streamingText + event.text;
  const messages = [...state.messages];

  // 取得或建立 assistant 訊息
  let assistantMsg = messages[messages.length - 1];
  if (!assistantMsg || assistantMsg.role !== 'assistant') {
    assistantMsg = { role: 'assistant', items: [] };
    messages.push(assistantMsg);
  }

  // 找到最後一個文字項目，或創建新的
  const lastItem = assistantMsg.items[assistantMsg.items.length - 1];
  if (lastItem && lastItem.type === 'text') {
    lastItem.content = newStreamingText;
  } else {
    assistantMsg.items.push({ type: 'text', content: newStreamingText });
  }

  return {
    stateUpdates: {
      streamingText: newStreamingText,
      messages,
      // 收到文字時切換到思考狀態
      avatarState: 'thinking',
    },
    actions: [{ type: 'scrollToBottom' }],
  };
}

/**
 * 處理 ToolUse 事件
 */
export function handleToolUseEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  if (!event.tool_name || !event.tool_id) {
    return { stateUpdates: {}, actions: [] };
  }

  const messages = [...state.messages];
  const currentToolUses = [...state.currentToolUses];

  // 取得或建立 assistant 訊息
  let assistantMsg = messages[messages.length - 1];
  if (!assistantMsg || assistantMsg.role !== 'assistant') {
    assistantMsg = { role: 'assistant', items: [] };
    messages.push(assistantMsg);
  }

  // 檢查是否已存在
  const existingTool = assistantMsg.items.find(
    (item): item is { type: 'tool'; tool: ToolUseItem } =>
      item.type === 'tool' && item.tool.id === event.tool_id
  );

  if (!existingTool) {
    const newTool: ToolUseItem = {
      id: event.tool_id,
      type: event.tool_name,
      name: event.tool_name,
      input: event.input || {},
    };

    assistantMsg.items.push({ type: 'tool', tool: newTool });

    if (!currentToolUses.find(t => t.id === event.tool_id)) {
      currentToolUses.push(newTool);
    }
  }

  return {
    stateUpdates: {
      busyStatus: `Using ${event.tool_name}...`,
      messages,
      currentToolUses,
      streamingText: '', // 清空累積的文字，讓工具後的文字從新開始
      // 開始執行工具時切換到工作狀態（有動畫）
      avatarState: 'working',
    },
    actions: [],
  };
}

/**
 * 判斷工具是否應該自動跳過 PermissionDenied 對話框
 *
 * 包含兩類工具：
 * 1. AUTO_ALLOW_TOOLS（與 permission_server 同步）- 只讀工具等
 * 2. ExitPlanMode - 由專門的 plan-approval-request 事件處理
 *
 * 背景：Claude CLI 的 PermissionRequest hook 有 bug (#29212)，
 * 即使工具被 auto-allow，有時仍會發出 PermissionDenied 事件到 stream。
 * 這裡作為前端的安全網，避免誤觸對話框。
 */
function isMetaTool(toolName: string): boolean {
  // AUTO_ALLOW_TOOLS 包含：AskUserQuestion, Read, Glob, Grep,
  // TodoRead, TodoWrite, Task, TaskOutput, WebSearch, WebFetch, EnterPlanMode
  if (isAutoAllowTool(toolName)) return true;
  // ExitPlanMode 走專用的 plan-approval 流程，不在這裡彈對話框
  if (toolName === 'ExitPlanMode') return true;
  return false;
}

/**
 * 處理 PermissionDenied 事件
 *
 * 邏輯：
 * 1. 「元工具」（AskUserQuestion、EnterPlanMode 等）：自動跳過，不顯示對話框
 * 2. 其他工具（包括 Read、Glob、Grep 等）：顯示對話框讓用戶確認
 *
 * 這樣既保留了元工具的自動處理，又確保只讀工具在特殊情況下（如讀取敏感路徑）可以讓用戶決定
 */
export function handlePermissionDeniedEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  if (!event.tool_name || !event.tool_id) {
    return { stateUpdates: {}, actions: [] };
  }

  // 「元工具」自動跳過，不顯示對話框
  if (isMetaTool(event.tool_name)) {
    console.log(`🔓 Auto-skipping meta tool: ${event.tool_name}`);
    const deniedToolsThisRequest = new Set(state.deniedToolsThisRequest);
    deniedToolsThisRequest.add(event.tool_name);
    return {
      stateUpdates: { deniedToolsThisRequest },
      actions: [],
    };
  }

  // 其他工具：顯示權限確認對話框
  const messages = [...state.messages];
  const currentToolUses = [...state.currentToolUses];
  const deniedToolsThisRequest = new Set(state.deniedToolsThisRequest);

  // 取得或建立 assistant 訊息
  let assistantMsg = messages[messages.length - 1];
  if (!assistantMsg || assistantMsg.role !== 'assistant') {
    assistantMsg = { role: 'assistant', items: [] };
    messages.push(assistantMsg);
  }

  // 加入或更新工具使用記錄
  // 注意：收到 PermissionDenied 表示這個工具已經被 CLI 拒絕了
  // 在非 Hook 模式下，CLI 會同時告訴 Claude API「權限被拒絕」
  // 所以這個工具已經失敗了，需要標記為 isCancelled
  let existingTool = assistantMsg.items.find(
    (item): item is { type: 'tool'; tool: ToolUseItem } =>
      item.type === 'tool' && item.tool.id === event.tool_id
  );

  if (!existingTool) {
    const newTool: ToolUseItem = {
      id: event.tool_id,
      type: event.tool_name,
      name: event.tool_name,
      input: event.input || {},
      isCancelled: true,  // 標記為已取消（權限被拒絕）
      userResponse: 'Permission denied',
    };

    assistantMsg.items.push({ type: 'tool', tool: newTool });

    if (!currentToolUses.find(t => t.id === event.tool_id)) {
      currentToolUses.push(newTool);
    }
  } else {
    // 更新已存在的工具
    existingTool.tool.isCancelled = true;
    existingTool.tool.userResponse = 'Permission denied';
  }

  // 同時更新 currentToolUses 中的工具
  const toolInList = currentToolUses.find(t => t.id === event.tool_id);
  if (toolInList) {
    toolInList.isCancelled = true;
    toolInList.userResponse = 'Permission denied';
  }

  // 累積被拒絕的工具
  deniedToolsThisRequest.add(event.tool_name);

  const stateUpdates: Partial<AppState> = {
    messages,
    currentToolUses,
    deniedToolsThisRequest,
    streamingText: '', // 清空累積的文字，讓工具後的文字從新開始
  };

  const actions: EventAction[] = [];

  // 只有當還沒有待確認的對話框時才設定
  // 信任 Claude CLI 的事件系統：收到 PermissionDenied 就表示需要用戶確認
  if (!state.pendingPermission) {
    stateUpdates.pendingPermission = {
      toolName: event.tool_name,
      toolId: event.tool_id,
      input: event.input || {},
      // 保存原始 prompt，用於 fallback 模式重新執行
      originalPrompt: state.lastPrompt,
    };
    stateUpdates.avatarState = 'waiting';
    stateUpdates.busyStatus = '等待確認...';
    actions.push({ type: 'stopBusyTextAnimation' });
  }

  return { stateUpdates, actions };
}

/**
 * 處理 ToolResult 事件
 *
 * 注意：權限允許後重新執行時，Claude CLI 可能會發送新的 tool_id，
 * 所以如果找不到匹配的工具，會嘗試找最近一個沒有 result 的同類工具來更新。
 */
export function handleToolResultEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  if (!event.tool_id) {
    return { stateUpdates: {}, actions: [] };
  }

  const messages = [...state.messages];
  const currentToolUses = [...state.currentToolUses];

  // 更新 currentToolUses
  let tool = currentToolUses.find(t => t.id === event.tool_id);

  // 如果找不到匹配的工具，嘗試找最近一個沒有 result 的工具
  // 這處理了權限允許後重新執行的情況（Claude CLI 可能會發送新的 tool_id）
  if (!tool) {
    // 從後往前找最近一個沒有 result 的工具
    for (let i = currentToolUses.length - 1; i >= 0; i--) {
      if (!currentToolUses[i].result && !currentToolUses[i].isCancelled) {
        tool = currentToolUses[i];
        console.log(`🔧 ToolResult: tool_id mismatch, updating last pending tool: ${tool.name}`);
        break;
      }
    }
  }

  if (tool) {
    // 如果用戶已經回答過這個工具（AskUserQuestion），保留用戶的回答
    if (!tool.userAnswered) {
      tool.result = event.result;
    }
    if (event.is_error) {
      tool.isCancelled = true;
    }
    // 儲存 Edit 工具的結構化差異
    if (event.structured_patch) {
      tool.structuredPatch = event.structured_patch;
    }
    // 儲存圖片結果的 base64 資料
    if (event.image_base64) {
      tool.imageBase64 = event.image_base64;
      tool.imageMediaType = event.image_media_type || 'image/png';
    }
  }

  // 更新 messages 中的工具
  const lastMsg = messages[messages.length - 1];
  if (lastMsg && lastMsg.role === 'assistant') {
    let toolItem = lastMsg.items.find(
      (item): item is { type: 'tool'; tool: ToolUseItem } =>
        item.type === 'tool' && item.tool.id === event.tool_id
    );

    // 如果找不到匹配的工具，嘗試找最近一個沒有 result 的工具
    if (!toolItem) {
      for (let i = lastMsg.items.length - 1; i >= 0; i--) {
        const item = lastMsg.items[i];
        if (item.type === 'tool' && !item.tool.result && !item.tool.isCancelled) {
          toolItem = item as { type: 'tool'; tool: ToolUseItem };
          break;
        }
      }
    }

    if (toolItem) {
      // 如果用戶已經回答過這個工具（AskUserQuestion），保留用戶的回答
      if (!toolItem.tool.userAnswered) {
        toolItem.tool.result = event.result;
      }
      if (event.is_error) {
        toolItem.tool.isCancelled = true;
      }
      // 儲存 Edit 工具的結構化差異
      if (event.structured_patch) {
        toolItem.tool.structuredPatch = event.structured_patch;
      }
      // 儲存圖片結果的 base64 資料
      if (event.image_base64) {
        toolItem.tool.imageBase64 = event.image_base64;
        toolItem.tool.imageMediaType = event.image_media_type || 'image/png';
      }
    }
  }

  // 根據工具執行結果設定表情
  const stateUpdates: Partial<AppState> = {
    messages,
    currentToolUses,
  };

  // 判斷是否為「權限未授予」的錯誤（會接著收到 PermissionDenied 事件）
  const isPermissionError = event.is_error && event.result &&
    (event.result.includes("haven't granted") || event.result.includes('permission'));

  if (event.is_error && !isPermissionError) {
    // 真正的錯誤才顯示 error 表情
    stateUpdates.avatarState = 'error';
  } else if (isPermissionError) {
    // 權限未授予：顯示 waiting 表情，等待 PermissionDenied 事件和用戶確認
    stateUpdates.avatarState = 'waiting';
  } else if (tool?.name === 'ExitPlanMode') {
    // ExitPlanMode 成功 → 計畫批准（OK 手勢）
    stateUpdates.avatarState = 'planApproved';
  }

  return {
    stateUpdates,
    actions: [],
  };
}

/**
 * 處理 Complete 事件
 */
export function handleCompleteEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  // 如果有待確認的權限請求，保持 waiting 狀態而不是 complete
  const avatarState = state.pendingPermission ? 'waiting' : 'complete';

  const stateUpdates: Partial<AppState> = {
    isLoading: false,
    avatarState,
    streamingText: '',
  };

  const actions: EventAction[] = [
    { type: 'stopBusyTextAnimation' },
    { type: 'startCompleteTimer' },
  ];

  // 更新 context 使用量資訊
  if (event.context_window_used_percent !== undefined) {
    stateUpdates.contextUsage = Math.round(event.context_window_used_percent);
  }
  if (event.total_tokens_in_conversation !== undefined || event.context_window_max !== undefined || event.input_tokens_this_turn !== undefined) {
    const inputTokensDelta = (event.input_tokens_this_turn !== undefined && state.prevInputTokens !== null)
      ? event.input_tokens_this_turn - state.prevInputTokens
      : undefined;

    stateUpdates.contextInfo = {
      totalTokens: event.total_tokens_in_conversation,
      maxTokens: event.context_window_max,
      inputTokensThisTurn: event.input_tokens_this_turn,
      inputTokensDelta,
    };

    // 記錄本次 input tokens，供下次計算差量
    if (event.input_tokens_this_turn !== undefined) {
      stateUpdates.prevInputTokens = event.input_tokens_this_turn;
    }
  }

  // 檢測 "Unknown skill: xxx" - CLI-only 命令提示
  if (event.result) {
    const unknownSkillMatch = event.result.match(/^Unknown skill:\s*(\w+)/);
    if (unknownSkillMatch) {
      const skillName = unknownSkillMatch[1];
      actions.push({
        type: 'showToast',
        message: `/${skillName} 是 CLI 專用指令，在此介面無法使用`,
        variant: 'warning',
      });
    }
  }

  return { stateUpdates, actions };
}

/**
 * 處理 Error 事件
 */
export function handleErrorEvent(
  event: ClaudeEvent,
  _state: AppState
): EventHandlerResult {
  const actions: EventAction[] = [{ type: 'stopBusyTextAnimation' }];

  if (event.message) {
    actions.push({ type: 'addErrorMessage', message: event.message });
  }

  return {
    stateUpdates: {
      isLoading: false,
      avatarState: 'error',  // 出錯時顯示緊張表情
    },
    actions,
  };
}

/**
 * 處理 Connected 事件
 */
export function handleConnectedEvent(
  _event: ClaudeEvent,
  _state: AppState
): EventHandlerResult {
  return {
    stateUpdates: {
      busyStatus: 'Connected',
    },
    actions: [],
  };
}

/**
 * 處理 Compacted 事件（對話摘要壓縮完成）
 * 注意：auto-compact 發生在 Claude 持續工作中，不代表對話結束
 */
export function handleCompactedEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  const summary = event.summary || '';

  // 將 compact 摘要加入 assistant 訊息
  const messages = [...state.messages];
  messages.push({
    role: 'assistant',
    items: [{ type: 'compact', summary }],
  });

  return {
    stateUpdates: {
      messages,
      // 重置 streamingText，避免 compact 前後的文字混在一起
      streamingText: '',
      // 不改變 isLoading 和 avatarState：Claude 在 compact 後會繼續工作
    },
    actions: [
      { type: 'scrollToBottom' },
    ],
  };
}

/**
 * 處理 ProcessExited 事件（互動模式下 CLI process 結束）
 */
export function handleProcessExitedEvent(
  _event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  const stateUpdates: Partial<AppState> = {
    isLoading: false,
    avatarState: 'idle',
    streamingText: '',
    isProcessAlive: false,
  };

  const actions: EventAction[] = [
    { type: 'stopBusyTextAnimation' },
  ];

  // 如果正在載入中（process 意外退出），顯示錯誤訊息
  if (state.isLoading) {
    actions.push({
      type: 'addErrorMessage',
      message: 'Claude CLI process 意外退出',
    });
  }

  return { stateUpdates, actions };
}

/**
 * 主要事件處理函數
 */
export function handleClaudeEvent(
  event: ClaudeEvent,
  state: AppState
): EventHandlerResult {
  switch (event.event_type) {
    case 'Init':
      return handleInitEvent(event, state);
    case 'Text':
      return handleTextEvent(event, state);
    case 'ToolUse':
      return handleToolUseEvent(event, state);
    case 'PermissionDenied':
      return handlePermissionDeniedEvent(event, state);
    case 'ToolResult':
      return handleToolResultEvent(event, state);
    case 'Compacted':
      return handleCompactedEvent(event, state);
    case 'Complete':
      return handleCompleteEvent(event, state);
    case 'Error':
      return handleErrorEvent(event, state);
    case 'Connected':
      return handleConnectedEvent(event, state);
    case 'ProcessExited':
      return handleProcessExitedEvent(event, state);
    default:
      return { stateUpdates: {}, actions: [] };
  }
}
