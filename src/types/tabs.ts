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

/** 持久化資料（存檔用） */
export interface TabPersistData {
  id: string;                    // nanoid 產生
  sessionId: string | null;      // Claude session ID
  title: string;                 // 標籤頁標題
  createdAt: string;             // ISO 8601
  order: number;                 // 排序
}

/** 運行時完整狀態 */
export interface TabState extends TabPersistData {
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

/** 持久化檔案格式 */
export interface TabsPersistFile {
  version: 1;
  activeTabId: string;
  tabs: TabPersistData[];
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

/** 從 TabState 提取持久化資料 */
export function extractPersistData(tab: TabState): TabPersistData {
  return {
    id: tab.id,
    sessionId: tab.sessionId,
    title: tab.title,
    createdAt: tab.createdAt,
    order: tab.order,
  };
}

/** 從持久化資料恢復 TabState（需要後續用 /continue 恢復 messages） */
export function restoreTabState(persist: TabPersistData): TabState {
  return {
    ...persist,
    messages: [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '*翻閱之前的筆記* 讓我看看我們上次聊到哪裡...' }]
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
