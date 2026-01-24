/**
 * 標籤頁管理 Composable
 */

import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { nanoid } from 'nanoid';
import {
  type TabState,
  type TabsPersistFile,
  createDefaultTabState,
  extractPersistData,
  restoreTabState,
} from '../types/tabs';

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
   * 初始化（載入持久化的標籤頁）
   */
  async function initialize(dir: string) {
    if (initialized && workingDir === dir) return;

    workingDir = dir;

    try {
      const data = await invoke<TabsPersistFile | null>('load_tabs', { workingDir: dir });

      if (data && data.tabs.length > 0) {
        // 恢復標籤頁
        tabs.value = data.tabs.map(persist => restoreTabState(persist));
        activeTabId.value = data.activeTabId;

        // 確保 activeTabId 有效
        if (!tabs.value.find(t => t.id === activeTabId.value)) {
          activeTabId.value = tabs.value[0]?.id || null;
        }

        console.log(`📂 Loaded ${tabs.value.length} tabs`);
      } else {
        // 沒有持久化資料，建立預設標籤頁
        const defaultTab = createDefaultTabState(nanoid());
        tabs.value = [defaultTab];
        activeTabId.value = defaultTab.id;
        console.log('📂 Created default tab');
      }

      initialized = true;
    } catch (error) {
      console.error('Failed to load tabs:', error);
      // 發生錯誤時建立預設標籤頁
      const defaultTab = createDefaultTabState(nanoid());
      tabs.value = [defaultTab];
      activeTabId.value = defaultTab.id;
      initialized = true;
    }
  }

  /**
   * 儲存標籤頁到持久化儲存
   */
  async function saveTabs() {
    if (!workingDir) return;

    const persistFile: TabsPersistFile = {
      version: 1,
      activeTabId: activeTabId.value || '',
      tabs: tabs.value.map(extractPersistData),
    };

    try {
      await invoke('save_tabs', { workingDir, data: persistFile });
      console.log('💾 Tabs saved');
    } catch (error) {
      console.error('Failed to save tabs:', error);
    }
  }

  /**
   * 建立新標籤頁
   */
  function createTab(title: string = '新對話'): TabState {
    const newTab = createDefaultTabState(nanoid(), title);
    newTab.order = tabs.value.length;
    tabs.value.push(newTab);
    activeTabId.value = newTab.id;
    saveTabs();
    return newTab;
  }

  /**
   * 切換到指定標籤頁
   */
  function switchTab(tabId: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (tab) {
      activeTabId.value = tabId;
      saveTabs();
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
        // 優先切換到下一個，否則切換到上一個
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

    saveTabs();
  }

  /**
   * 清除當前標籤頁（開始新對話）
   */
  function clearCurrentTab() {
    const tab = activeTab.value;
    if (!tab) return;

    // 重置標籤頁狀態
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

    saveTabs();
  }

  /**
   * 更新標籤頁標題
   */
  function updateTabTitle(tabId: string, title: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (tab) {
      tab.title = title;
      saveTabs();
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
      saveTabs();
      return existingTab;
    }

    // 建立新標籤頁
    const newTab = createDefaultTabState(nanoid(), summary || '對話');
    newTab.sessionId = sessionId;
    newTab.order = tabs.value.length;
    newTab.messages = [
      {
        role: 'assistant',
        items: [{ type: 'text', content: '*翻閱之前的筆記* 嗯，讓我看看我們上次聊到哪裡...' }]
      }
    ];
    tabs.value.push(newTab);
    activeTabId.value = newTab.id;
    saveTabs();
    return newTab;
  }

  /**
   * 更新當前標籤頁的狀態（用於與 App.vue 同步）
   */
  function updateCurrentTabState(updates: Partial<TabState>) {
    const tab = activeTab.value;
    if (!tab) return;

    Object.assign(tab, updates);

    // 如果更新了 sessionId 或 title，儲存到持久化
    if ('sessionId' in updates || 'title' in updates) {
      saveTabs();
    }
  }

  /**
   * 根據第一則用戶訊息自動生成標題
   */
  function autoGenerateTitle(tabId: string) {
    const tab = tabs.value.find(t => t.id === tabId);
    if (!tab || tab.title !== '新對話') return;

    // 找到第一則用戶訊息
    const firstUserMsg = tab.messages.find(m => m.role === 'user');
    if (firstUserMsg) {
      const textItem = firstUserMsg.items.find(item => item.type === 'text');
      if (textItem && textItem.type === 'text') {
        // 取前 30 個字元作為標題
        let title = textItem.content.slice(0, 30);
        if (textItem.content.length > 30) {
          title += '...';
        }
        tab.title = title;
        saveTabs();
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
    saveTabs,
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
