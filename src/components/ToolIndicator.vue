<script setup lang="ts">
import { ref, computed } from 'vue';
import hljs from 'highlight.js';

// 工具類型定義
type ToolType = 'Read' | 'Bash' | 'Edit' | 'Write' | 'Glob' | 'Grep' | 'Task';

// Props 定義
const props = defineProps<{
  type: ToolType;
  path?: string;
  description?: string;
  reason?: string;
  input?: string;
  output?: string;
  exitCode?: number;
  // Edit 專用 - side-by-side diff
  oldCode?: string;  // 修改前的程式碼
  newCode?: string;  // 修改後的程式碼
  summary?: string;  // 如 "Added 1 line"
}>();

// 是否展開
const isExpanded = ref(true);

// 工具顏色對應
const toolColors: Record<ToolType, string> = {
  Read: '#e74c3c',    // 紅色
  Bash: '#e67e22',    // 橘色
  Edit: '#9b59b6',    // 紫色
  Write: '#3498db',   // 藍色
  Glob: '#1abc9c',    // 青色
  Grep: '#2ecc71',    // 綠色
  Task: '#f39c12',    // 金色
};

const toolColor = computed(() => toolColors[props.type] || '#a0a0a0');

// 高亮程式碼（用於 Bash 輸入輸出）
function highlightCode(code: string, lang: string = 'bash'): string {
  const language = hljs.getLanguage(lang) ? lang : 'plaintext';
  return hljs.highlight(code, { language }).value;
}

// 切換展開狀態
function toggleExpand() {
  isExpanded.value = !isExpanded.value;
}
</script>

<template>
  <div class="tool-indicator">
    <!-- 標題列 -->
    <div class="tool-header" @click="toggleExpand">
      <span class="tool-dot" :style="{ backgroundColor: toolColor }"></span>
      <span class="tool-type">{{ type }}</span>
      <span v-if="path" class="tool-path">{{ path }}</span>
      <span v-if="summary" class="tool-summary">{{ summary }}</span>
      <span v-if="description && !path" class="tool-description">{{ description }}</span>
      <span class="expand-icon">{{ isExpanded ? '▼' : '▶' }}</span>
    </div>

    <!-- 展開內容 -->
    <div v-if="isExpanded" class="tool-content">
      <!-- Reason（如果有）-->
      <div v-if="reason" class="tool-reason">
        <span class="reason-label">Reason:</span> {{ reason }}
      </div>

      <!-- Bash 工具：顯示輸入和輸出 -->
      <template v-if="type === 'Bash'">
        <div v-if="input" class="tool-block">
          <div class="block-label">IN</div>
          <pre class="block-content"><code v-html="highlightCode(input, 'bash')"></code></pre>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-label">OUT</div>
          <div class="block-content">
            <div v-if="exitCode !== undefined" class="exit-code" :class="{ error: exitCode !== 0 }">
              Exit code {{ exitCode }}
            </div>
            <pre v-if="output"><code>{{ output }}</code></pre>
          </div>
        </div>
      </template>

      <!-- Edit 工具：Side-by-side Diff -->
      <template v-if="type === 'Edit' && (oldCode || newCode)">
        <div class="diff-sidebyside">
          <!-- 左側：修改前 -->
          <div class="diff-panel old">
            <pre><code v-html="highlightCode(oldCode || '', 'python')"></code></pre>
          </div>
          <!-- 中間分隔線 -->
          <div class="diff-divider"></div>
          <!-- 右側：修改後 -->
          <div class="diff-panel new">
            <pre><code v-html="highlightCode(newCode || '', 'python')"></code></pre>
          </div>
        </div>
      </template>

      <!-- Read 工具：簡單顯示 -->
      <template v-if="type === 'Read' && !input && !output">
        <!-- Read 通常只顯示標題，不需要額外內容 -->
      </template>
    </div>
  </div>
</template>

<style scoped>
.tool-indicator {
  margin: 8px 0;
  border-radius: 8px;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.2);
  font-size: 0.9rem;
}

.tool-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
  user-select: none;
  transition: background-color 0.2s;
}

.tool-header:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.tool-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tool-type {
  font-weight: 600;
  color: var(--text-color);
}

.tool-path {
  color: var(--text-muted);
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.tool-summary {
  color: var(--text-muted);
  font-size: 0.85em;
  margin-left: auto;
  padding-right: 8px;
}

.tool-description {
  color: var(--text-muted);
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.expand-icon {
  color: var(--text-muted);
  font-size: 0.7em;
  margin-left: auto;
}

.tool-content {
  padding: 0 12px 12px;
}

.tool-reason {
  color: var(--text-muted);
  font-size: 0.85em;
  margin-bottom: 8px;
  padding: 4px 0;
}

.reason-label {
  color: #f39c12;
}

.tool-block {
  margin-top: 8px;
  border-radius: 6px;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.3);
}

.block-label {
  display: inline-block;
  padding: 2px 8px;
  font-size: 0.75em;
  font-weight: 600;
  color: var(--text-muted);
  background-color: rgba(255, 255, 255, 0.1);
}

.block-content {
  padding: 8px 12px;
}

.block-content pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.block-content code {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
  color: var(--text-color);
}

.exit-code {
  font-size: 0.85em;
  color: #2ecc71;
  margin-bottom: 4px;
}

.exit-code.error {
  color: #e74c3c;
}

/* Side-by-side Diff 樣式 */
.diff-sidebyside {
  margin-top: 8px;
  display: flex;
  border-radius: 6px;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.3);
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
}

.diff-panel {
  flex: 1;
  overflow-x: auto;
  min-width: 0;
}

.diff-panel pre {
  margin: 0;
  padding: 12px;
  min-height: 100%;
}

.diff-panel code {
  display: block;
  white-space: pre;
  line-height: 1.5;
}

.diff-panel.old {
  background-color: rgba(231, 76, 60, 0.1);
  border-right: 1px solid rgba(231, 76, 60, 0.3);
}

.diff-panel.old code {
  color: #e8e8e8;
}

.diff-panel.new {
  background-color: rgba(46, 204, 113, 0.1);
}

.diff-panel.new code {
  color: #e8e8e8;
}

.diff-divider {
  width: 2px;
  background: linear-gradient(180deg, rgba(231, 76, 60, 0.5) 0%, rgba(46, 204, 113, 0.5) 100%);
  flex-shrink: 0;
}
</style>
