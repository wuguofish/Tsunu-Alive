<script setup lang="ts">
import { ref, computed } from 'vue';
import hljs from 'highlight.js';

// 已知的工具類型列表
const KNOWN_TOOLS = [
  'Read', 'Write', 'Edit', 'Bash', 'Glob', 'Grep',
  'WebSearch', 'WebFetch', 'Task', 'TaskOutput',
  'TodoWrite', 'AskUserQuestion',
  'NotebookEdit', 'KillShell', 'Skill',
  'EnterPlanMode', 'ExitPlanMode',
] as const;

// TodoWrite 的待辦事項類型
interface TodoItem {
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
  activeForm?: string;
}

// AskUserQuestion 的問題類型
interface QuestionOption {
  label: string;
  description?: string;
}

interface Question {
  question: string;
  header?: string;
  options: QuestionOption[];
  multiSelect?: boolean;
}

// 執行狀態
type ToolStatus = 'running' | 'success' | 'error' | 'cancelled';

// Props 定義 - type 改為 string 以支援未知工具
const props = defineProps<{
  type: string;
  path?: string;
  description?: string;
  reason?: string;
  input?: string;
  output?: string;
  exitCode?: number;
  // Edit 專用 - side-by-side diff
  oldCode?: string;
  newCode?: string;
  summary?: string;
  // WebSearch/WebFetch 專用
  query?: string;
  // Grep/Glob 專用
  pattern?: string;
  // Task 專用
  prompt?: string;
  // TodoWrite 專用
  todos?: TodoItem[];
  // AskUserQuestion 專用
  questions?: Question[];
  // NotebookEdit 專用
  notebookPath?: string;
  cellId?: string;
  newSource?: string;
  cellType?: string;
  editMode?: string;
  // KillShell 專用
  shellId?: string;
  // Skill 專用
  skill?: string;
  args?: string;
  // TaskOutput 專用
  taskId?: string;
  // Write 專用
  content?: string;
  // 通用
  rawInput?: Record<string, unknown>;  // 用於 fallback 顯示原始輸入
  isRunning?: boolean;
  isCancelled?: boolean;
  userResponse?: string;
}>();

// 是否展開
const isExpanded = ref(true);

// 判斷是否為已知工具
const isKnownTool = computed(() => {
  return KNOWN_TOOLS.includes(props.type as typeof KNOWN_TOOLS[number]);
});

// 狀態顏色對應
const statusColors: Record<ToolStatus, string> = {
  running: '#e67e22',   // 橘色：執行中
  success: '#2ecc71',   // 綠色：執行成功
  error: '#e74c3c',     // 紅色：執行失敗
  cancelled: '#a0a0a0', // 灰色：執行取消
};

// 計算目前狀態
const toolStatus = computed<ToolStatus>(() => {
  if (props.isCancelled) return 'cancelled';
  if (props.isRunning) return 'running';
  // 有 exitCode 且不為 0 表示失敗
  if (props.exitCode !== undefined && props.exitCode !== 0) return 'error';
  // 有結果表示成功
  if (props.output !== undefined || props.exitCode === 0) return 'success';
  // 預設為執行中
  return 'running';
});

const toolColor = computed(() => statusColors[toolStatus.value]);

// 計算輸出行數（用於 Grep/Glob 等顯示 "N lines of output"）
const outputLineCount = computed(() => {
  if (!props.output) return 0;
  return props.output.split('\n').filter(line => line.trim()).length;
});

// 格式化 JSON（用於 fallback 顯示原始輸入）
const formattedRawInput = computed(() => {
  if (!props.rawInput) return '';
  try {
    return JSON.stringify(props.rawInput, null, 2);
  } catch {
    return String(props.rawInput);
  }
});

// 高亮程式碼（用於 Bash 輸入輸出）
function highlightCode(code: string, lang: string = 'bash'): string {
  const language = hljs.getLanguage(lang) ? lang : 'plaintext';
  return hljs.highlight(code, { language }).value;
}

// 根據檔案路徑猜測語言
function guessLanguage(filePath?: string): string {
  if (!filePath) return 'plaintext';
  const ext = filePath.split('.').pop()?.toLowerCase();
  const extMap: Record<string, string> = {
    'ts': 'typescript',
    'tsx': 'typescript',
    'js': 'javascript',
    'jsx': 'javascript',
    'vue': 'html',
    'py': 'python',
    'rs': 'rust',
    'go': 'go',
    'java': 'java',
    'c': 'c',
    'cpp': 'cpp',
    'h': 'c',
    'hpp': 'cpp',
    'cs': 'csharp',
    'rb': 'ruby',
    'php': 'php',
    'swift': 'swift',
    'kt': 'kotlin',
    'scala': 'scala',
    'sh': 'bash',
    'bash': 'bash',
    'zsh': 'bash',
    'json': 'json',
    'yaml': 'yaml',
    'yml': 'yaml',
    'xml': 'xml',
    'html': 'html',
    'css': 'css',
    'scss': 'scss',
    'sass': 'scss',
    'less': 'less',
    'sql': 'sql',
    'md': 'markdown',
    'toml': 'toml',
  };
  return extMap[ext || ''] || 'plaintext';
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
      <span class="tool-dot" :class="{ running: toolStatus === 'running' }" :style="{ backgroundColor: toolColor }"></span>
      <span class="tool-type" :class="{ unknown: !isKnownTool }">{{ type }}</span>
      <span v-if="pattern || query" class="tool-pattern">"{{ pattern || query }}"</span>
      <span v-else-if="path || notebookPath" class="tool-path">{{ path || notebookPath }}</span>
      <span v-else-if="skill" class="tool-pattern">/{{ skill }}</span>
      <span v-if="summary" class="tool-summary">{{ summary }}</span>
      <span v-else-if="(type === 'Grep' || type === 'Glob') && output" class="tool-summary">{{ outputLineCount }} {{ outputLineCount === 1 ? 'line' : 'lines' }} of output</span>
      <span v-if="description && !path && !notebookPath && !pattern && !query && !skill" class="tool-description">{{ description }}</span>
      <span class="expand-icon">{{ isExpanded ? '▼' : '▶' }}</span>
    </div>

    <!-- 展開內容 -->
    <div v-if="isExpanded" class="tool-content">
      <!-- Reason（如果有）-->
      <div v-if="reason" class="tool-reason">
        <span class="reason-label">Reason:</span> {{ reason }}
      </div>

      <!-- 使用者回應（權限拒絕或自訂內容）-->
      <div v-if="userResponse" class="user-response">
        <span class="response-label">User:</span> {{ userResponse }}
      </div>

      <!-- ========== 已知工具模板 ========== -->

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
      <template v-if="type === 'Edit'">
        <!-- 如果有 oldCode/newCode，顯示 side-by-side diff -->
        <div v-if="oldCode || newCode" class="diff-sidebyside">
          <div class="diff-panel old">
            <pre><code v-html="highlightCode(oldCode || '', 'typescript')"></code></pre>
          </div>
          <div class="diff-divider"></div>
          <div class="diff-panel new">
            <pre><code v-html="highlightCode(newCode || '', 'typescript')"></code></pre>
          </div>
        </div>
        <!-- 如果只有 output（結果），顯示結果 -->
        <div v-else-if="output" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Read 工具：顯示讀取結果 -->
      <template v-if="type === 'Read'">
        <div v-if="output" class="tool-block output">
          <div class="block-label">CONTENT</div>
          <pre class="block-content"><code v-html="highlightCode(output, guessLanguage(path))"></code></pre>
        </div>
      </template>

      <!-- Write 工具：顯示寫入內容 -->
      <template v-if="type === 'Write'">
        <div v-if="content" class="tool-block">
          <div class="block-label">CONTENT</div>
          <pre class="block-content"><code v-html="highlightCode(content, guessLanguage(path))"></code></pre>
        </div>
        <div v-else-if="output" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Task 工具：顯示 prompt -->
      <template v-if="type === 'Task'">
        <div v-if="prompt" class="tool-block">
          <div class="block-label">PROMPT</div>
          <pre class="block-content"><code>{{ prompt }}</code></pre>
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- TaskOutput 工具：顯示任務輸出 -->
      <template v-if="type === 'TaskOutput'">
        <div v-if="taskId" class="tool-info">
          <span class="info-label">Task ID:</span> {{ taskId }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">OUTPUT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- WebSearch 工具：顯示搜尋查詢 -->
      <template v-if="type === 'WebSearch'">
        <div v-if="query" class="tool-block">
          <div class="block-label">QUERY</div>
          <pre class="block-content"><code>{{ query }}</code></pre>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- WebFetch 工具：顯示 URL -->
      <template v-if="type === 'WebFetch'">
        <div v-if="query" class="tool-block">
          <div class="block-label">URL</div>
          <pre class="block-content"><code>{{ query }}</code></pre>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-label">RESPONSE</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Grep 工具：顯示搜尋結果 -->
      <template v-if="type === 'Grep'">
        <div v-if="path" class="tool-info">
          <span class="info-label">in</span> {{ path }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">MATCHES</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Glob 工具：顯示匹配結果 -->
      <template v-if="type === 'Glob'">
        <div v-if="path" class="tool-info">
          <span class="info-label">in</span> {{ path }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">FILES</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- TodoWrite 工具：顯示待辦事項列表 -->
      <template v-if="type === 'TodoWrite'">
        <div v-if="todos && todos.length > 0" class="todo-list">
          <div
            v-for="(todo, index) in todos"
            :key="index"
            class="todo-item"
            :class="todo.status"
          >
            <span class="todo-icon">
              {{ todo.status === 'completed' ? '✓' : todo.status === 'in_progress' ? '●' : '○' }}
            </span>
            <span class="todo-content">{{ todo.content }}</span>
          </div>
        </div>
      </template>

      <!-- AskUserQuestion 工具：顯示問題和回答 -->
      <template v-if="type === 'AskUserQuestion'">
        <div v-if="questions && questions.length > 0" class="question-list">
          <div v-for="(q, index) in questions" :key="index" class="question-item">
            <div class="question-text">{{ q.question }}</div>
            <div v-if="q.options && q.options.length > 0" class="question-options">
              <div v-for="(opt, optIdx) in q.options" :key="optIdx" class="option-item">
                <span class="option-label">{{ opt.label }}</span>
                <span v-if="opt.description" class="option-desc">{{ opt.description }}</span>
              </div>
            </div>
          </div>
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">ANSWER</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- NotebookEdit 工具：顯示 Notebook 編輯 -->
      <template v-if="type === 'NotebookEdit'">
        <div class="tool-info">
          <span v-if="cellId" class="info-item"><span class="info-label">Cell:</span> {{ cellId }}</span>
          <span v-if="cellType" class="info-item"><span class="info-label">Type:</span> {{ cellType }}</span>
          <span v-if="editMode" class="info-item"><span class="info-label">Mode:</span> {{ editMode }}</span>
        </div>
        <div v-if="newSource" class="tool-block">
          <div class="block-label">SOURCE</div>
          <pre class="block-content"><code v-html="highlightCode(newSource, 'python')"></code></pre>
        </div>
      </template>

      <!-- KillShell 工具：顯示終止的 Shell -->
      <template v-if="type === 'KillShell'">
        <div v-if="shellId" class="tool-info">
          <span class="info-label">Shell ID:</span> {{ shellId }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Skill 工具：顯示技能呼叫 -->
      <template v-if="type === 'Skill'">
        <div v-if="args" class="tool-info">
          <span class="info-label">Args:</span> {{ args }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">RESULT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- EnterPlanMode/ExitPlanMode 工具：狀態提示 -->
      <template v-if="type === 'EnterPlanMode' || type === 'ExitPlanMode'">
        <div class="plan-mode-indicator">
          <span v-if="type === 'EnterPlanMode'" class="plan-enter">📋 進入計畫模式</span>
          <span v-else class="plan-exit">✅ 退出計畫模式</span>
        </div>
      </template>

      <!-- ========== 未知工具 Fallback ========== -->
      <template v-if="!isKnownTool">
        <div class="fallback-notice">
          <span class="fallback-icon">❓</span>
          <span>未知工具類型</span>
        </div>
        <div v-if="rawInput && Object.keys(rawInput).length > 0" class="tool-block">
          <div class="block-label">INPUT</div>
          <pre class="block-content"><code>{{ formattedRawInput }}</code></pre>
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-label">OUTPUT</div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
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

.tool-dot.running {
  animation: dot-pulse 1s infinite;
}

@keyframes dot-pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.tool-type {
  font-weight: 600;
  color: var(--text-color);
}

.tool-type.unknown {
  color: #9b59b6;
  font-style: italic;
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

.tool-pattern {
  color: #f39c12;
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

.tool-info {
  color: var(--text-muted);
  font-size: 0.85em;
  font-family: 'Consolas', 'Monaco', monospace;
  margin-bottom: 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.info-item {
  display: inline-flex;
  gap: 4px;
}

.info-label {
  color: #3498db;
}

.reason-label {
  color: #f39c12;
}

.user-response {
  color: var(--text-muted);
  font-size: 0.85em;
  margin-bottom: 8px;
  padding: 6px 10px;
  background-color: rgba(160, 160, 160, 0.15);
  border-radius: 4px;
  border-left: 3px solid #a0a0a0;
}

.response-label {
  color: #a0a0a0;
  font-weight: 600;
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

/* TodoWrite 樣式 */
.todo-list {
  margin-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.todo-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 0.85em;
}

.todo-item.pending {
  color: var(--text-muted);
}

.todo-item.in_progress {
  color: #f39c12;
  background-color: rgba(243, 156, 18, 0.1);
}

.todo-item.completed {
  color: #2ecc71;
}

.todo-item.completed .todo-content {
  text-decoration: line-through;
  opacity: 0.7;
}

.todo-icon {
  flex-shrink: 0;
  width: 16px;
  text-align: center;
}

.todo-content {
  flex: 1;
}

/* AskUserQuestion 樣式 */
.question-list {
  margin-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.question-item {
  padding: 8px 12px;
  background-color: rgba(52, 152, 219, 0.1);
  border-radius: 6px;
  border-left: 3px solid #3498db;
}

.question-text {
  font-weight: 500;
  margin-bottom: 8px;
}

.question-options {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-left: 12px;
}

.option-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 0.85em;
}

.option-label {
  color: #3498db;
}

.option-desc {
  color: var(--text-muted);
  font-size: 0.9em;
}

/* Plan Mode 樣式 */
.plan-mode-indicator {
  padding: 12px;
  text-align: center;
  font-size: 0.9em;
}

.plan-enter {
  color: #3498db;
}

.plan-exit {
  color: #2ecc71;
}

/* Fallback 樣式 */
.fallback-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin-bottom: 8px;
  background-color: rgba(155, 89, 182, 0.15);
  border-radius: 4px;
  border-left: 3px solid #9b59b6;
  font-size: 0.85em;
  color: #9b59b6;
}

.fallback-icon {
  font-size: 1em;
}
</style>
