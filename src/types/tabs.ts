/**
 * 多對話標籤頁型別定義
 */

import type {
  Message,
  ToolUseItem,
  PendingPermission,
  AvatarState,
  ContextInfo,
} from '../utils/claudeEventHandler';

/** Claude CLI session entry（從 sessions-index.json 或 .jsonl 掃描） */
export interface SessionEntry {
  sessionId: string;
  title: string;
  modified: string;
  messageCount: number;
  gitBranch: string;
}

/** 運行時完整狀態 */
export interface TabState {
  id: string;                    // nanoid 產生
  sessionId: string | null;      // Claude session ID
  title: string;                 // 標籤頁標題
  createdAt: string;             // ISO 8601
  order: number;                 // 排序
  messages: Message[];
  streamingText: string;
  currentToolUses: ToolUseItem[];
  deniedToolsThisRequest: Set<string>;
  contextUsage: number | null;
  contextInfo: ContextInfo | null;
  lastPrompt: string;
  isLoading: boolean;
  pendingPermission: PendingPermission | null;
  avatarState: AvatarState;
}

/** 建立新標籤頁的預設狀態 */
export function createDefaultTabState(id: string, title: string = '新對話'): TabState {
  return {
    id,
    sessionId: null,
    title,
    createdAt: new Date().toISOString(),
    order: 0,
    messages: [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '欸，你來啦～有什麼需要幫忙的嗎？ *推眼鏡*' }]
      }
    ],
    streamingText: '',
    currentToolUses: [],
    deniedToolsThisRequest: new Set(),
    contextUsage: null,
    contextInfo: null,
    lastPrompt: '',
    isLoading: false,
    pendingPermission: null,
    avatarState: 'idle',
  };
}
