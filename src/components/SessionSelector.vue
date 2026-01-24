<script setup lang="ts">
import { ref, computed } from 'vue';
import type { TabState } from '../types/tabs';

// Props
const props = defineProps<{
  tabs: TabState[];
  activeTabId: string | null;
  historySessions: Array<{
    session_id: string;
    created_at: string;
    last_modified: string;
    summary: string | null;
  }>;
  historyLoading: boolean;
}>();

// Emits
const emit = defineEmits<{
  (e: 'switch-tab', tabId: string): void;
  (e: 'close-tab', tabId: string): void;
  (e: 'new-conversation'): void;
  (e: 'open-history', sessionId: string, summary: string | null): void;
  (e: 'load-history'): void;
}>();

// 下拉選單是否展開
const isOpen = ref(false);

// 當前活躍的標籤頁
const activeTab = computed(() => {
  return props.tabs.find(t => t.id === props.activeTabId) || null;
});

// 切換選單
function toggleMenu() {
  isOpen.value = !isOpen.value;
  if (isOpen.value) {
    emit('load-history');
  }
}

// 關閉選單
function closeMenu() {
  isOpen.value = false;
}

// 切換標籤頁
function handleSwitchTab(tabId: string) {
  emit('switch-tab', tabId);
  closeMenu();
}

// 關閉標籤頁（阻止事件冒泡）
function handleCloseTab(e: Event, tabId: string) {
  e.stopPropagation();
  emit('close-tab', tabId);
}

// 開始新對話
function handleNewConversation() {
  emit('new-conversation');
  closeMenu();
}

// 從歷史對話開啟
function handleOpenHistory(session: { session_id: string; summary: string | null }) {
  emit('open-history', session.session_id, session.summary);
  closeMenu();
}

// 格式化時間顯示
function formatTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffMins < 1) return 'just now';
  if (diffMins < 60) return `${diffMins}m`;
  if (diffHours < 24) return `${diffHours}h`;
  if (diffDays < 7) return `${diffDays}d`;
  return date.toLocaleDateString('zh-TW', { month: 'short', day: 'numeric' });
}

// 注意：暫時移除 watch 監聽器來測試
</script>

<template>
  <div class="session-selector">
    <!-- 選擇器按鈕 -->
    <div class="selector-button" @click="toggleMenu">
      <span class="selector-title">{{ activeTab?.title || '新對話' }}</span>
      <span class="selector-arrow">{{ isOpen ? '▲' : '▼' }}</span>
    </div>

    <!-- 新對話按鈕 -->
    <button class="new-btn" @click="handleNewConversation" title="開始新對話">
      +
    </button>

    <!-- 下拉選單 -->
    <div v-if="isOpen" class="dropdown-menu">
      <!-- 已開啟的標籤頁 -->
      <div class="menu-section" v-if="props.tabs.length > 0">
        <div class="section-title">OPEN TABS</div>
        <div
          v-for="tab in props.tabs"
          :key="tab.id"
          class="menu-item"
          :class="{ active: tab.id === activeTabId }"
          @click="handleSwitchTab(tab.id)"
        >
          <span class="item-indicator">{{ tab.id === activeTabId ? '●' : '' }}</span>
          <span class="item-title">{{ tab.title }}</span>
          <span class="item-time">{{ formatTime(tab.createdAt) }}</span>
          <button
            class="item-close"
            @click="handleCloseTab($event, tab.id)"
            title="關閉標籤頁"
            v-if="props.tabs.length > 1"
          >
            x
          </button>
        </div>
      </div>

      <!-- 歷史對話 -->
      <div class="menu-section" v-if="props.historySessions.length > 0 || historyLoading">
        <div class="section-title">HISTORY</div>
        <div v-if="historyLoading" class="loading-hint">載入中...</div>
        <div
          v-for="session in props.historySessions"
          :key="session.session_id"
          class="menu-item history-item"
          @click="handleOpenHistory(session)"
        >
          <span class="item-indicator"></span>
          <span class="item-title">{{ session.summary || '(無摘要)' }}</span>
          <span class="item-time">{{ formatTime(session.last_modified) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.session-selector {
  flex: 2;
  display: flex;
  align-items: center;
  gap: 4px;
  position: relative;
}

.selector-button {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  max-width: 280px;
}

.selector-button:hover {
  background-color: rgba(0, 0, 0, 0.3);
  border-color: var(--primary-color);
}

.selector-title {
  color: var(--text-color);
  font-size: 0.9rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.selector-arrow {
  color: var(--text-muted);
  font-size: 0.7rem;
}

.new-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  color: var(--text-muted);
  font-size: 1.2rem;
  cursor: pointer;
  transition: all 0.2s;
}

.new-btn:hover {
  background-color: rgba(46, 204, 113, 0.2);
  border-color: #2ecc71;
  color: #2ecc71;
}

.dropdown-menu {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 4px;
  width: 360px;
  max-height: 480px;
  overflow-y: auto;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  z-index: 1000;
}

.menu-section {
  padding: 8px 0;
}

.section-title {
  padding: 4px 12px;
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  cursor: pointer;
  transition: background-color 0.15s;
}

.menu-item:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.menu-item.active {
  background-color: rgba(74, 144, 217, 0.15);
}

.item-indicator {
  width: 12px;
  color: var(--primary-color);
  font-size: 0.6rem;
}

.item-title {
  flex: 1;
  color: var(--text-color);
  font-size: 0.9rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-time {
  color: var(--text-muted);
  font-size: 0.8rem;
  margin-left: auto;
}

.item-close {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--text-muted);
  font-size: 1rem;
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s;
}

.menu-item:hover .item-close {
  opacity: 1;
}

.item-close:hover {
  background-color: rgba(231, 76, 60, 0.2);
  color: #e74c3c;
}

.history-item {
  opacity: 0.8;
}

.history-item:hover {
  opacity: 1;
}

.loading-hint {
  padding: 12px;
  text-align: center;
  color: var(--text-muted);
  font-size: 0.85rem;
}
</style>
