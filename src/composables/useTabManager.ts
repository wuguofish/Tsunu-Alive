/**
 * 標籤頁管理 Composable
 * 資料來源：~/.claude/projects/ 下的 sessions-index.json 或 .jsonl 檔案
 */

import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { nanoid } from 'nanoid';
import type {
  TabState,
  SessionEntry,
} from '../types/tabs';
import { createDefaultTabState } from '../types/tabs';

// 單例模式：全域狀態
const tabs = ref<TabState[]>([]);
const activeTabId = ref<string | null>(null);
let workingDir: string | null = null;
let initialized = false;

/**
 * 標籤頁管理 Composable
 */
export function useTabManager() {
  // 當前活躍的標籤頁
  const activeTab = computed(() => {
    if (!activeTabId.value) return null;
    return tabs.value.find(t => t.id === activeTabId.value) || null;
  });

  // 所有標籤頁（按 order 排序）
  const sortedTabs = computed(() => {
    return [...tabs.value].sort((a, b) => a.order - b.order);
  });

  // 標籤頁數量
  const tabCount = computed(() => tabs.value.length);

  /**
   * 初始化（從 Claude CLI sessions 載入標籤頁）
   */
  async function initialize(dir: string) {
    // 如果是同一個專案且已初始化，跳過
    if (initialized && workingDir === dir) return;

    // 切換專案時，先重置狀態
    if (workingDir && workingDir !== dir) {
      initialized = false;
    }

    workingDir = dir;

    try {
      const data = await invoke<{ sessions: SessionEntry[] }>('load_sessions', { workingDir: dir });

      if (data && data.sessions.length > 0) {
        // 從 sessions 建立標籤頁（依 modified 排序，最新的在前）
        const sorted = [...data.sessions].sort((a, b) => {
          return (b.modified || '').localeCompare(a.modified || '');
        });

        // 只取最近幾個 session 當作標籤頁（避免太多）
        const maxTabs = 10;
        const recentSessions = sorted.slice(0, maxTabs);

        tabs.value = recentSessions.map((session, index) => {
          const title = session.title
            ? session.title.slice(0, 30) + (session.title.length > 30 ? '...' : '')
            : '對話';
          const tab = createDefaultTabState(nanoid(), title);
          tab.sessionId = session.sessionId;
          tab.order = index;
          // 有 session 的標籤頁先顯示載入中的訊息
          tab.messages = [{
            role: 'assistant',
            items: [{ type: 'text', content: '*翻閱之前的筆記* 讓我看看我們上次聊到哪裡...' }]
          }];
          return tab;
        });

        // 最新的 session 為 active
        activeTabId.value = tabs.value[0]?.id || null;
      } else {
        // 沒有 session，建立預設標籤頁
        const defaultTab = createDefaultTabState(nanoid());
        tabs.value = [defaultTab];
        activeTabId.value = defaultTab.id;
      }

      initialized = true;
    } catch (error) {
      console.error('Failed to load sessions:', error);
      const defaultTab = createDefaultTabState(nanoid());
      tabs.value = [defaultTab];
      activeTabId.value = defaultTab.id;
      initialized = true;
    }
  }

  /**
   * 建立新標籤頁
   */
  function createTab(title: string = '新對話'): TabState {
    const newTab = createDefaultTabState(nanoid(), title);
    newTab.order = 0;
    // 把現有標籤頁的 order 都往後推
    tabs.value.forEach(t => t.order++);
    tabs.value.unshift(newTab);
    activeTabId.value = newTab.id;
    return newTab;
  }

  /**
   * 切換到指定標籤頁
   */
  function switchTab(tabId: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (tab) {
      activeTabId.value = tabId;
    }
  }

  /**
   * 關閉標籤頁
   */
  function closeTab(tabId: string) {
    const index = tabs.value.findIndex(t => t.id === tabId);
    if (index === -1) return;

    tabs.value.splice(index, 1);

    // 如果關閉的是當前標籤頁，切換到相鄰的標籤頁
    if (activeTabId.value === tabId) {
      if (tabs.value.length > 0) {
        const newIndex = Math.min(index, tabs.value.length - 1);
        activeTabId.value = tabs.value[newIndex].id;
      } else {
        // 沒有標籤頁了，建立新的
        const newTab = createDefaultTabState(nanoid());
        tabs.value.push(newTab);
        activeTabId.value = newTab.id;
      }
    }

    // 重新計算 order
    tabs.value.forEach((tab, i) => {
      tab.order = i;
    });
  }

  /**
   * 清除當前標籤頁（開始新對話）
   */
  function clearCurrentTab() {
    const tab = activeTab.value;
    if (!tab) return;

    tab.sessionId = null;
    tab.title = '新對話';
    tab.messages = [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '好，我們開始新的對話吧！有什麼需要幫忙的嗎？ *推眼鏡*' }]
      }
    ];
    tab.streamingText = '';
    tab.currentToolUses = [];
    tab.deniedToolsThisRequest.clear();
    tab.contextUsage = null;
    tab.contextInfo = null;
    tab.lastPrompt = '';
    tab.isLoading = false;
    tab.pendingPermission = null;
    tab.avatarState = 'idle';
  }

  /**
   * 更新標籤頁標題
   */
  function updateTabTitle(tabId: string, title: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (tab) {
      tab.title = title;
    }
  }

  /**
   * 從歷史對話開啟（在新標籤頁中）
   */
  function openFromHistory(sessionId: string, summary: string | null): TabState {
    // 檢查是否已有該 session 的標籤頁
    const existingTab = tabs.value.find(t => t.sessionId === sessionId);
    if (existingTab) {
      activeTabId.value = existingTab.id;
      return existingTab;
    }

    // 建立新標籤頁
    const newTab = createDefaultTabState(nanoid(), summary || '對話');
    newTab.sessionId = sessionId;
    newTab.order = 0;
    tabs.value.forEach(t => t.order++);
    newTab.messages = [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '*翻閱之前的筆記* 嗯，讓我看看我們上次聊到哪裡...' }]
      }
    ];
    tabs.value.unshift(newTab);
    activeTabId.value = newTab.id;
    return newTab;
  }

  /**
   * 更新當前標籤頁的狀態（用於與 App.vue 同步）
   */
  function updateCurrentTabState(updates: Partial<TabState>) {
    const tab = activeTab.value;
    if (!tab) return;
    Object.assign(tab, updates);
  }

  /**
   * 根據第一則用戶訊息自動生成標題
   */
  function autoGenerateTitle(tabId: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (!tab || tab.title !== '新對話') return;

    const firstUserMsg = tab.messages.find(m => m.role === 'user');
    if (firstUserMsg) {
      const textItem = firstUserMsg.items.find(item => item.type === 'text');
      if (textItem && textItem.type === 'text') {
        let title = textItem.content.slice(0, 30);
        if (textItem.content.length > 30) {
          title += '...';
        }
        tab.title = title;
      }
    }
  }

  return {
    // 狀態
    tabs,
    activeTabId,
    activeTab,
    sortedTabs,
    tabCount,

    // 方法
    initialize,
    createTab,
    switchTab,
    closeTab,
    clearCurrentTab,
    updateTabTitle,
    openFromHistory,
    updateCurrentTabState,
    autoGenerateTitle,
  };
}
