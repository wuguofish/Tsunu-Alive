/**
 * Claude 事件處理邏輯
 * 提取自 App.vue，用於測試和重用
 */

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
  event_type: 'Init' | 'Text' | 'ToolUse' | 'ToolResult' | 'Complete' | 'Error' | 'Connected' | 'PermissionDenied';
  session_id?: string;
  model?: string;
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
  // Edit 工具的結構化差異（ToolResult 事件）
  structured_patch?: DiffHunk[];
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
}

// 對話項目（文字或工具，按時間順序）
export type ChatItem =
  | { type: 'text'; content: string }
  | { type: 'tool'; tool: ToolUseItem };

// 訊息類型
export interface Message {
  role: 'user' | 'assistant';
  items: ChatItem[];
}

// 阿宇的表情狀態
export type AvatarState = 'idle' | 'processing' | 'complete' | 'waiting';

// 編輯模式
export type EditMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan';

// Context 詳細資訊
export interface ContextInfo {
  totalTokens?: number;
  maxTokens?: number;
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
  // 最後一次用戶主動發起的請求（用於 fallback 模式重新執行）
  lastPrompt: string;
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
  | { type: 'addErrorMessage'; message: string };

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
    },
    actions: [],
  };
}

/**
 * 「元工具」- 這些工具本身就是與用戶互動或內部追蹤用，不應該需要權限確認
 * 如果 Claude CLI 對這些工具發出 PermissionDenied，我們應該自動跳過（不顯示對話框）
 *
 * 注意：這與 AUTO_ALLOW_TOOLS（給 permission_server 用）不同！
 * - META_TOOLS：前端自動跳過，不顯示對話框
 * - AUTO_ALLOW_TOOLS：permission_server 自動回應 allow
 */
const META_TOOLS = [
  'AskUserQuestion',  // 本身就是詢問用戶
  'EnterPlanMode',    // 進入計畫模式
  'ExitPlanMode',     // 退出計畫模式
  'TodoWrite',        // 內部追蹤
  'TodoRead',         // 內部追蹤
  'Task',             // 子代理任務
  'TaskOutput',       // 子代理任務輸出
] as const;

function isMetaTool(toolName: string): boolean {
  return META_TOOLS.includes(toolName as typeof META_TOOLS[number]);
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
    }
  }

  return {
    stateUpdates: {
      messages,
      currentToolUses,
    },
    actions: [],
  };
}

/**
 * 處理 Complete 事件
 */
export function handleCompleteEvent(
  event: ClaudeEvent,
  _state: AppState
): EventHandlerResult {
  const stateUpdates: Partial<AppState> = {
    isLoading: false,
    avatarState: 'complete',
    streamingText: '',
  };

  // 更新 context 使用量資訊
  if (event.context_window_used_percent !== undefined) {
    stateUpdates.contextUsage = Math.round(event.context_window_used_percent);
  }
  if (event.total_tokens_in_conversation !== undefined || event.context_window_max !== undefined) {
    stateUpdates.contextInfo = {
      totalTokens: event.total_tokens_in_conversation,
      maxTokens: event.context_window_max,
    };
  }

  return {
    stateUpdates,
    actions: [
      { type: 'stopBusyTextAnimation' },
      { type: 'startCompleteTimer' },
    ],
  };
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
      avatarState: 'idle',
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
    case 'Complete':
      return handleCompleteEvent(event, state);
    case 'Error':
      return handleErrorEvent(event, state);
    case 'Connected':
      return handleConnectedEvent(event, state);
    default:
      return { stateUpdates: {}, actions: [] };
  }
}
