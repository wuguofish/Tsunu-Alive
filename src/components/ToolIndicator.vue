<script setup lang="ts">
import { ref, computed } from 'vue';
import hljs from 'highlight.js';
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener';
import { renderMarkdown } from '../utils/markdown';
import { AlertTriangle, Check, Circle, CircleDot, Copy } from 'lucide-vue-next';

// 複製狀態追蹤
const copiedBlockId = ref<string | null>(null);

// Block 內容展開狀態追蹤（預設全部摺疊）
const expandedBlocks = ref<Set<string>>(new Set());

// 切換 Block 展開狀態
function toggleBlockExpand(blockId: string) {
  if (expandedBlocks.value.has(blockId)) {
    expandedBlocks.value.delete(blockId);
  } else {
    expandedBlocks.value.add(blockId);
  }
  // 觸發響應式更新
  expandedBlocks.value = new Set(expandedBlocks.value);
}

// 檢查 Block 是否展開
function isBlockExpanded(blockId: string): boolean {
  return expandedBlocks.value.has(blockId);
}

// 檢查內容是否超過指定行數（預設 8 行）
// 同時檢查行數和字元數，取較寬鬆的判斷
function shouldShowExpand(content: string | undefined, maxLines: number = 8): boolean {
  if (!content) return false;
  const lineCount = content.split('\n').length;
  // 超過指定行數，或者內容超過約 400 字元（約 8 行 x 50 字元）
  return lineCount > maxLines || content.length > 400;
}

// 複製到剪貼簿
async function copyToClipboard(text: string, blockId: string) {
  try {
    await navigator.clipboard.writeText(text);
    copiedBlockId.value = blockId;
    setTimeout(() => {
      copiedBlockId.value = null;
    }, 2000);
  } catch (err) {
    console.error('Failed to copy:', err);
  }
}

// 智慧開啟檔案：IDE 連線時用編輯器開啟，否則在檔案總管顯示
async function openFile(filePath: string) {
  try {
    if (props.ideConnected) {
      // IDE 連線中 → 用系統預設編輯器開啟
      console.log('Opening file in editor:', filePath);
      await openPath(filePath);
    } else {
      // 未連線 → 在檔案總管中顯示
      console.log('Revealing file in explorer:', filePath);
      await revealItemInDir(filePath);
    }
  } catch (err) {
    console.error('Failed to open/reveal file:', err);
  }
}

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

// Edit 工具的差異 hunk 類型
interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: string[];  // 每行開頭：' '=未變更, '-'=刪除, '+'=新增
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
  // IDE 連線狀態
  ideConnected?: boolean;
  // Edit 工具的結構化差異（VS Code 風格 Diff View）
  structuredPatch?: DiffHunk[];
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

// 計算 diff hunk 中每行的行號
interface DiffLineInfo {
  oldLineNo: number | null;  // 舊檔案行號，null 表示新增行
  newLineNo: number | null;  // 新檔案行號，null 表示刪除行
  prefix: string;
  content: string;
}

function computeDiffLineNumbers(hunk: DiffHunk): DiffLineInfo[] {
  const result: DiffLineInfo[] = [];
  let oldLine = hunk.oldStart;
  let newLine = hunk.newStart;

  for (const line of hunk.lines) {
    const prefix = line.charAt(0);
    const content = line.substring(1);

    if (prefix === ' ') {
      // 未改變行：兩邊都有行號
      result.push({ oldLineNo: oldLine, newLineNo: newLine, prefix, content });
      oldLine++;
      newLine++;
    } else if (prefix === '-') {
      // 刪除行：只有舊行號
      result.push({ oldLineNo: oldLine, newLineNo: null, prefix, content });
      oldLine++;
    } else if (prefix === '+') {
      // 新增行：只有新行號
      result.push({ oldLineNo: null, newLineNo: newLine, prefix, content });
      newLine++;
    }
  }

  return result;
}

// 判斷工具是否失敗（通用，來自 Claude CLI 的 is_error 欄位或權限被拒絕）
const isToolError = computed(() => {
  return props.isCancelled === true;
});

// 判斷 Edit 工具是否失敗（保留向後兼容）
const isEditError = computed(() => {
  if (props.type !== 'Edit') return false;
  return isToolError.value;
});

// 格式化錯誤訊息（移除 <tool_use_error> 標籤，通用）
const formattedErrorMessage = computed(() => {
  if (!props.output) return '';
  // 提取 <tool_use_error> 標籤中的內容
  const match = props.output.match(/<tool_use_error>([\s\S]*?)<\/tool_use_error>/);
  if (match) {
    return match[1].trim();
  }
  // 如果沒有標籤，直接返回原始內容
  return props.output;
});

// 格式化 Edit 錯誤訊息（保留向後兼容）
const formattedEditError = computed(() => formattedErrorMessage.value);

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
      <span
        v-else-if="path || notebookPath"
        class="tool-path clickable"
        @click.stop="openFile(path || notebookPath!)"
        :title="ideConnected ? '在編輯器中開啟' : '在檔案總管中顯示'"
      >{{ path || notebookPath }}</span>
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
        <!-- 錯誤訊息（權限被拒絕等） -->
        <div v-if="isToolError && output" class="tool-error">
          <AlertTriangle class="error-icon" :size="14" />
          <span class="error-message">{{ formattedErrorMessage }}</span>
        </div>
        <div v-if="input" class="tool-block">
          <div class="block-header">
            <div class="block-label">IN</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'bash-in' }"
              @click.stop="copyToClipboard(input, 'bash-in')"
            ><Check v-if="copiedBlockId === 'bash-in'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('bash-in') && shouldShowExpand(input) }">
            <pre class="block-content"><code v-html="highlightCode(input, 'bash')"></code></pre>
            <button v-if="shouldShowExpand(input)" class="expand-btn" @click.stop="toggleBlockExpand('bash-in')">
              {{ isBlockExpanded('bash-in') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-header">
            <div class="block-label">OUT</div>
            <button
              v-if="output"
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'bash-out' }"
              @click.stop="copyToClipboard(output, 'bash-out')"
            ><Check v-if="copiedBlockId === 'bash-out'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('bash-out') && shouldShowExpand(output) }">
            <div class="block-content">
              <div v-if="exitCode !== undefined" class="exit-code" :class="{ error: exitCode !== 0 }">
                Exit code {{ exitCode }}
              </div>
              <pre v-if="output"><code>{{ output }}</code></pre>
            </div>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('bash-out')">
              {{ isBlockExpanded('bash-out') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Edit 工具：VS Code 風格 Diff View -->
      <template v-if="type === 'Edit'">
        <!-- 錯誤訊息（優先顯示） -->
        <div v-if="isEditError && output" class="tool-error">
          <AlertTriangle class="error-icon" :size="14" />
          <span class="error-message">{{ formattedEditError }}</span>
        </div>
        <!-- 優先使用 structuredPatch（VS Code 風格） -->
        <div v-else-if="structuredPatch && structuredPatch.length > 0" class="diff-vscode">
          <div v-for="(hunk, hunkIdx) in structuredPatch" :key="hunkIdx" class="diff-hunk">
            <div class="diff-hunk-header">
              @@ -{{ hunk.oldStart }},{{ hunk.oldLines }} +{{ hunk.newStart }},{{ hunk.newLines }} @@
            </div>
            <div class="diff-lines">
              <div
                v-for="(lineInfo, lineIdx) in computeDiffLineNumbers(hunk)"
                :key="lineIdx"
                class="diff-line"
                :class="{
                  'diff-line-unchanged': lineInfo.prefix === ' ',
                  'diff-line-deleted': lineInfo.prefix === '-',
                  'diff-line-added': lineInfo.prefix === '+'
                }"
              >
                <span class="diff-line-no old">{{ lineInfo.oldLineNo ?? '' }}</span>
                <span class="diff-line-no new">{{ lineInfo.newLineNo ?? '' }}</span>
                <span class="diff-line-prefix">{{ lineInfo.prefix }}</span>
                <span class="diff-line-content"><code v-html="highlightCode(lineInfo.content, guessLanguage(path))"></code></span>
              </div>
            </div>
          </div>
        </div>
        <!-- Fallback：如果有 oldCode/newCode，顯示 side-by-side diff -->
        <div v-else-if="oldCode || newCode" class="diff-sidebyside">
          <div class="diff-panel old">
            <div class="diff-panel-header">
              <span>OLD</span>
              <button
                v-if="oldCode"
                class="copy-btn copy-btn-small"
                :class="{ copied: copiedBlockId === 'edit-old' }"
                @click.stop="copyToClipboard(oldCode, 'edit-old')"
              ><Check v-if="copiedBlockId === 'edit-old'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
            </div>
            <pre><code v-html="highlightCode(oldCode || '', guessLanguage(path))"></code></pre>
          </div>
          <div class="diff-divider"></div>
          <div class="diff-panel new">
            <div class="diff-panel-header">
              <span>NEW</span>
              <button
                v-if="newCode"
                class="copy-btn copy-btn-small"
                :class="{ copied: copiedBlockId === 'edit-new' }"
                @click.stop="copyToClipboard(newCode, 'edit-new')"
              ><Check v-if="copiedBlockId === 'edit-new'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
            </div>
            <pre><code v-html="highlightCode(newCode || '', guessLanguage(path))"></code></pre>
          </div>
        </div>
        <!-- 如果只有 output（結果），顯示結果 -->
        <div v-else-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'edit-result' }"
              @click.stop="copyToClipboard(output, 'edit-result')"
            ><Check v-if="copiedBlockId === 'edit-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Read 工具：顯示讀取結果 -->
      <template v-if="type === 'Read'">
        <!-- 錯誤訊息（權限被拒絕等） -->
        <div v-if="isToolError && output" class="tool-error">
          <AlertTriangle class="error-icon" :size="14" />
          <span class="error-message">{{ formattedErrorMessage }}</span>
        </div>
        <!-- 正常內容 -->
        <div v-else-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">CONTENT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'read-content' }"
              @click.stop="copyToClipboard(output, 'read-content')"
            ><Check v-if="copiedBlockId === 'read-content'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('read-content') && shouldShowExpand(output) }">
            <pre class="block-content"><code v-html="highlightCode(output, guessLanguage(path))"></code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('read-content')">
              {{ isBlockExpanded('read-content') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Write 工具：顯示寫入內容 -->
      <template v-if="type === 'Write'">
        <div v-if="content" class="tool-block">
          <div class="block-header">
            <div class="block-label">CONTENT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'write-content' }"
              @click.stop="copyToClipboard(content, 'write-content')"
            ><Check v-if="copiedBlockId === 'write-content'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('write-content') && shouldShowExpand(content) }">
            <pre class="block-content"><code v-html="highlightCode(content, guessLanguage(path))"></code></pre>
            <button v-if="shouldShowExpand(content)" class="expand-btn" @click.stop="toggleBlockExpand('write-content')">
              {{ isBlockExpanded('write-content') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
        <div v-else-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'write-result' }"
              @click.stop="copyToClipboard(output, 'write-result')"
            ><Check v-if="copiedBlockId === 'write-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <pre class="block-content"><code>{{ output }}</code></pre>
        </div>
      </template>

      <!-- Task 工具：顯示 prompt -->
      <template v-if="type === 'Task'">
        <div v-if="prompt" class="tool-block">
          <div class="block-header">
            <div class="block-label">PROMPT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'task-prompt' }"
              @click.stop="copyToClipboard(prompt, 'task-prompt')"
            ><Check v-if="copiedBlockId === 'task-prompt'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('task-prompt') && shouldShowExpand(prompt) }">
            <pre class="block-content"><code>{{ prompt }}</code></pre>
            <button v-if="shouldShowExpand(prompt)" class="expand-btn" @click.stop="toggleBlockExpand('task-prompt')">
              {{ isBlockExpanded('task-prompt') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'task-result' }"
              @click.stop="copyToClipboard(output, 'task-result')"
            ><Check v-if="copiedBlockId === 'task-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('task-result') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('task-result')">
              {{ isBlockExpanded('task-result') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- TaskOutput 工具：顯示任務輸出 -->
      <template v-if="type === 'TaskOutput'">
        <div v-if="taskId" class="tool-info">
          <span class="info-label">Task ID:</span> {{ taskId }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">OUTPUT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'taskoutput-output' }"
              @click.stop="copyToClipboard(output, 'taskoutput-output')"
            ><Check v-if="copiedBlockId === 'taskoutput-output'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('taskoutput-output') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('taskoutput-output')">
              {{ isBlockExpanded('taskoutput-output') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- WebSearch 工具：顯示搜尋查詢 -->
      <template v-if="type === 'WebSearch'">
        <div v-if="query" class="tool-block">
          <div class="block-header">
            <div class="block-label">QUERY</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'websearch-query' }"
              @click.stop="copyToClipboard(query, 'websearch-query')"
            ><Check v-if="copiedBlockId === 'websearch-query'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <pre class="block-content"><code>{{ query }}</code></pre>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              v-if="output"
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'websearch-result' }"
              @click.stop="copyToClipboard(output, 'websearch-result')"
            ><Check v-if="copiedBlockId === 'websearch-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('websearch-result') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('websearch-result')">
              {{ isBlockExpanded('websearch-result') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- WebFetch 工具：顯示 URL -->
      <template v-if="type === 'WebFetch'">
        <div v-if="query" class="tool-block">
          <div class="block-header">
            <div class="block-label">URL</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'webfetch-url' }"
              @click.stop="copyToClipboard(query, 'webfetch-url')"
            ><Check v-if="copiedBlockId === 'webfetch-url'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <pre class="block-content"><code>{{ query }}</code></pre>
        </div>
        <div v-if="output !== undefined" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESPONSE</div>
            <button
              v-if="output"
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'webfetch-response' }"
              @click.stop="copyToClipboard(output, 'webfetch-response')"
            ><Check v-if="copiedBlockId === 'webfetch-response'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('webfetch-response') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('webfetch-response')">
              {{ isBlockExpanded('webfetch-response') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Grep 工具：顯示搜尋結果 -->
      <template v-if="type === 'Grep'">
        <div v-if="path" class="tool-info">
          <span class="info-label">in</span> {{ path }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">MATCHES</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'grep-matches' }"
              @click.stop="copyToClipboard(output, 'grep-matches')"
            ><Check v-if="copiedBlockId === 'grep-matches'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('grep-matches') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('grep-matches')">
              {{ isBlockExpanded('grep-matches') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Glob 工具：顯示匹配結果 -->
      <template v-if="type === 'Glob'">
        <div v-if="path" class="tool-info">
          <span class="info-label">in</span> {{ path }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">FILES</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'glob-files' }"
              @click.stop="copyToClipboard(output, 'glob-files')"
            ><Check v-if="copiedBlockId === 'glob-files'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('glob-files') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('glob-files')">
              {{ isBlockExpanded('glob-files') ? '收合' : '展開' }}
            </button>
          </div>
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
              <Check v-if="todo.status === 'completed'" :size="14" />
              <CircleDot v-else-if="todo.status === 'in_progress'" :size="14" />
              <Circle v-else :size="14" />
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
          <div class="block-header">
            <div class="block-label">ANSWER</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'ask-answer' }"
              @click.stop="copyToClipboard(output, 'ask-answer')"
            ><Check v-if="copiedBlockId === 'ask-answer'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
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
          <div class="block-header">
            <div class="block-label">SOURCE</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'notebook-source' }"
              @click.stop="copyToClipboard(newSource, 'notebook-source')"
            ><Check v-if="copiedBlockId === 'notebook-source'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('notebook-source') && shouldShowExpand(newSource) }">
            <pre class="block-content"><code v-html="highlightCode(newSource, 'python')"></code></pre>
            <button v-if="shouldShowExpand(newSource)" class="expand-btn" @click.stop="toggleBlockExpand('notebook-source')">
              {{ isBlockExpanded('notebook-source') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- KillShell 工具：顯示終止的 Shell -->
      <template v-if="type === 'KillShell'">
        <div v-if="shellId" class="tool-info">
          <span class="info-label">Shell ID:</span> {{ shellId }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'killshell-result' }"
              @click.stop="copyToClipboard(output, 'killshell-result')"
            ><Check v-if="copiedBlockId === 'killshell-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('killshell-result') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('killshell-result')">
              {{ isBlockExpanded('killshell-result') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- Skill 工具：顯示技能呼叫 -->
      <template v-if="type === 'Skill'">
        <div v-if="args" class="tool-info">
          <span class="info-label">Args:</span> {{ args }}
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">RESULT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'skill-result' }"
              @click.stop="copyToClipboard(output, 'skill-result')"
            ><Check v-if="copiedBlockId === 'skill-result'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('skill-result') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('skill-result')">
              {{ isBlockExpanded('skill-result') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- EnterPlanMode 工具：進入計畫模式 -->
      <template v-if="type === 'EnterPlanMode'">
        <div class="plan-mode-indicator">
          <span class="plan-enter">📋 進入計畫模式</span>
        </div>
      </template>

      <!-- ExitPlanMode 工具：退出計畫模式，顯示計畫內容 -->
      <template v-if="type === 'ExitPlanMode'">
        <div class="plan-mode-indicator">
          <span class="plan-exit">✅ 退出計畫模式</span>
        </div>
        <!-- 顯示計畫檔案路徑 -->
        <div v-if="rawInput?._planFilePath" class="tool-block plan-file-path">
          <div class="block-header">
            <div class="block-label">PLAN FILE</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'plan-filepath' }"
              @click.stop="copyToClipboard(String(rawInput._planFilePath), 'plan-filepath')"
            ><Check v-if="copiedBlockId === 'plan-filepath'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content plan-path-content">
            <span
              class="plan-path-text clickable"
              @click.stop="openFile(String(rawInput._planFilePath))"
              title="點擊開啟檔案"
            >{{ rawInput._planFilePath }}</span>
          </div>
        </div>
        <!-- 顯示遠端 Session -->
        <div v-if="rawInput?.pushToRemote && rawInput?.remoteSessionUrl" class="tool-block plan-remote">
          <div class="block-label">REMOTE SESSION</div>
          <div class="block-content">
            <a :href="rawInput.remoteSessionUrl as string" target="_blank" class="plan-link">
              {{ rawInput.remoteSessionTitle || 'Claude.ai Session' }}
            </a>
          </div>
        </div>
        <!-- 顯示允許的操作 -->
        <div v-if="rawInput?.allowedPrompts && (rawInput.allowedPrompts as any[]).length > 0" class="tool-block plan-prompts">
          <div class="block-label">ALLOWED ACTIONS</div>
          <ul class="plan-actions-list">
            <li v-for="(prompt, index) in (rawInput.allowedPrompts as any[])" :key="index">
              <span class="action-tool">{{ prompt.tool }}</span>: {{ prompt.prompt }}
            </li>
          </ul>
        </div>
        <!-- 顯示計畫內容（如果有的話） -->
        <div v-if="output" class="tool-block plan-content">
          <div class="block-header">
            <div class="block-label">PLAN</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'plan-content' }"
              @click.stop="copyToClipboard(output, 'plan-content')"
            ><Check v-if="copiedBlockId === 'plan-content'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('plan-content') && shouldShowExpand(output) }">
            <div class="block-content plan-markdown" v-html="renderMarkdown(output)"></div>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('plan-content')">
              {{ isBlockExpanded('plan-content') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
      </template>

      <!-- ========== 未知工具 Fallback ========== -->
      <template v-if="!isKnownTool">
        <div class="fallback-notice">
          <span class="fallback-icon">?</span>
          <span>未知工具類型</span>
        </div>
        <div v-if="rawInput && Object.keys(rawInput).length > 0" class="tool-block">
          <div class="block-header">
            <div class="block-label">INPUT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'fallback-input' }"
              @click.stop="copyToClipboard(formattedRawInput, 'fallback-input')"
            ><Check v-if="copiedBlockId === 'fallback-input'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('fallback-input') && shouldShowExpand(formattedRawInput) }">
            <pre class="block-content"><code>{{ formattedRawInput }}</code></pre>
            <button v-if="shouldShowExpand(formattedRawInput)" class="expand-btn" @click.stop="toggleBlockExpand('fallback-input')">
              {{ isBlockExpanded('fallback-input') ? '收合' : '展開' }}
            </button>
          </div>
        </div>
        <div v-if="output" class="tool-block output">
          <div class="block-header">
            <div class="block-label">OUTPUT</div>
            <button
              class="copy-btn"
              :class="{ copied: copiedBlockId === 'fallback-output' }"
              @click.stop="copyToClipboard(output, 'fallback-output')"
            ><Check v-if="copiedBlockId === 'fallback-output'" :size="14" /><template v-else><Copy :size="12" /> 複製</template></button>
          </div>
          <div class="block-content-container" :class="{ collapsed: !isBlockExpanded('fallback-output') && shouldShowExpand(output) }">
            <pre class="block-content"><code>{{ output }}</code></pre>
            <button v-if="shouldShowExpand(output)" class="expand-btn" @click.stop="toggleBlockExpand('fallback-output')">
              {{ isBlockExpanded('fallback-output') ? '收合' : '展開' }}
            </button>
          </div>
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

.tool-path.clickable {
  color: var(--primary-light, #6ba3e0);
  cursor: pointer;
  text-decoration: underline;
  text-decoration-style: dotted;
  text-underline-offset: 3px;
}

.tool-path.clickable:hover {
  color: #fff;
  text-decoration-style: solid;
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

/* 確保 pre 元素（無論是在 .block-content 內還是本身就是 .block-content）都能自動換行 */
.block-content pre,
pre.block-content {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.block-content code {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
  color: var(--text-color);
}

/* Block Content Container - 可展開/摺疊 */
.block-content-container {
  position: relative;
}

/* 摺疊時只限制內容高度，不影響展開按鈕 */
.block-content-container.collapsed > .block-content,
.block-content-container.collapsed > pre.block-content {
  max-height: calc(1.5em * 8 + 16px); /* 8 行 + padding */
  overflow: hidden;
}

/* 漸層遮罩放在 container 上，避免被 content 的 overflow 截斷 */
.block-content-container.collapsed::before {
  content: '';
  position: absolute;
  bottom: 30px; /* expand-btn 高度上方 */
  left: 0;
  right: 0;
  height: 40px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.6));
  pointer-events: none;
  z-index: 1;
}

/* 展開按鈕 */
.expand-btn {
  display: block;
  width: 100%;
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.08);
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--text-muted);
  font-size: 0.8rem;
  cursor: pointer;
  transition: background 0.2s;
}

.expand-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  color: var(--text-color);
}

/* Block Header 樣式 (label + copy button) */
.block-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background-color: rgba(255, 255, 255, 0.1);
}

.block-header .block-label {
  background: none;
}

.block-header .copy-btn {
  position: static;
  margin: 2px 4px;
}

/* 複製按鈕樣式 */
.copy-btn {
  background: #444;
  color: #fff;
  border: none;
  border-radius: 4px;
  padding: 4px 10px;
  font-size: 0.75rem;
  cursor: pointer;
  opacity: 0.7;
  transition: opacity 0.2s, background 0.2s;
}

.copy-btn:hover {
  opacity: 1;
  background: #666;
}

.copy-btn.copied {
  background: #22c55e;
  opacity: 1;
}

.copy-btn-small {
  padding: 2px 6px;
  font-size: 0.7rem;
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

.diff-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 8px;
  font-size: 0.75em;
  font-weight: 600;
  color: var(--text-muted);
  background-color: rgba(255, 255, 255, 0.05);
}

.diff-panel-header .copy-btn {
  position: static;
  opacity: 0.6;
}

.diff-panel-header .copy-btn:hover {
  opacity: 1;
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

/* Edit 工具錯誤訊息 */
.tool-error {
  display: flex;
  align-items: center;
  justify-content: center;
  
  gap: 8px;
  margin-top: 8px;
  padding: 12px;
  background-color: rgba(231, 156, 60, 0.15);
  border: 1px solid rgba(231, 156, 60, 0.3);
  border-radius: 6px;
  color: #d7974d;
}

.tool-error .error-icon {
  flex-shrink: 0;
  font-size: 1.1em;
}

.tool-error .error-message {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.9em;
  line-height: 1.4;
  word-break: break-word;
}

/* VS Code 風格 Diff View */
.diff-vscode {
  margin-top: 8px;
  border-radius: 6px;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.3);
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
}

.diff-hunk {
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.diff-hunk:last-child {
  border-bottom: none;
}

.diff-hunk-header {
  padding: 4px 12px;
  background-color: rgba(52, 152, 219, 0.2);
  color: #6ba3e0;
  font-size: 0.85em;
  font-weight: 500;
}

.diff-lines {
  padding: 0;
}

.diff-line {
  display: flex;
  line-height: 1.5;
  padding: 0 8px 0 0;
}

.diff-line-no {
  width: 40px;
  flex-shrink: 0;
  text-align: right;
  padding-right: 8px;
  user-select: none;
  color: var(--text-muted);
  opacity: 0.6;
  font-size: 0.9em;
}

.diff-line-no.old {
  border-right: 1px solid rgba(255, 255, 255, 0.1);
}

.diff-line-no.new {
  border-right: 1px solid rgba(255, 255, 255, 0.1);
}

.diff-line-deleted .diff-line-no.old {
  color: #e74c3c;
  opacity: 0.8;
}

.diff-line-added .diff-line-no.new {
  color: #2ecc71;
  opacity: 0.8;
}

.diff-line-prefix {
  width: 20px;
  flex-shrink: 0;
  text-align: center;
  user-select: none;
  color: var(--text-muted);
}

.diff-line-content {
  flex: 1;
  white-space: pre-wrap;
  word-break: break-all;
}

.diff-line-content code {
  font-family: inherit;
  font-size: inherit;
}

.diff-line-unchanged {
  background-color: transparent;
}

.diff-line-deleted {
  background-color: rgba(231, 76, 60, 0.15);
}

.diff-line-deleted .diff-line-prefix {
  color: #e74c3c;
}

.diff-line-added {
  background-color: rgba(46, 204, 113, 0.15);
}

.diff-line-added .diff-line-prefix {
  color: #2ecc71;
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

/* ExitPlanMode 計畫內容樣式 */
.plan-remote .block-content {
  padding: 8px 12px;
}

.plan-link {
  color: var(--primary-light, #6ba3e0);
  text-decoration: none;
}

.plan-link:hover {
  text-decoration: underline;
}

.plan-actions-list {
  margin: 0;
  padding: 8px 12px 8px 28px;
  font-size: 0.85em;
  line-height: 1.6;
}

.plan-actions-list li {
  margin-bottom: 4px;
}

.action-tool {
  font-family: monospace;
  background: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.9em;
}

/* Plan Markdown 渲染樣式 */
.plan-markdown {
  font-size: 1rem;
  line-height: 1.6;
}

.plan-markdown :deep(h1),
.plan-markdown :deep(h2),
.plan-markdown :deep(h3) {
  margin: 0.8em 0 0.4em;
  color: var(--text-color);
}

.plan-markdown :deep(h1) { font-size: 1.3em; }
.plan-markdown :deep(h2) { font-size: 1.15em; }
.plan-markdown :deep(h3) { font-size: 1em; }

.plan-markdown :deep(p) {
  margin: 0.5em 0;
}

.plan-markdown :deep(ul),
.plan-markdown :deep(ol) {
  margin: 0.5em 0;
  padding-left: 1.5em;
}

.plan-markdown :deep(li) {
  margin: 0.25em 0;
}

.plan-markdown :deep(code) {
  background: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.9em;
}

.plan-markdown :deep(pre) {
  background: rgba(0, 0, 0, 0.3);
  padding: 8px 12px;
  border-radius: 4px;
  overflow-x: auto;
  margin: 0.5em 0;
}

.plan-markdown :deep(pre code) {
  background: none;
  padding: 0;
}

.plan-markdown :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin: 0.5em 0;
  font-size: 0.9em;
}

.plan-markdown :deep(th),
.plan-markdown :deep(td) {
  border: 1px solid rgba(255, 255, 255, 0.2);
  padding: 6px 10px;
  text-align: left;
}

.plan-markdown :deep(th) {
  background: rgba(255, 255, 255, 0.1);
}

.plan-markdown :deep(blockquote) {
  margin: 0.5em 0;
  padding-left: 12px;
  border-left: 3px solid var(--primary-color, #3498db);
  color: var(--text-muted);
}

/* 計畫檔案路徑樣式 */
.plan-path-content {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
}

.plan-path-icon {
  font-size: 1rem;
  flex-shrink: 0;
}

.plan-path-text {
  color: var(--primary-light, #6ba3e0);
  word-break: break-all;
}

.plan-path-text.clickable {
  cursor: pointer;
  text-decoration: underline;
  text-decoration-style: dotted;
  text-underline-offset: 3px;
}

.plan-path-text.clickable:hover {
  color: #fff;
  text-decoration-style: solid;
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
