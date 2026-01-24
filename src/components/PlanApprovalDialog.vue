<script setup lang="ts">
import { ref, computed } from 'vue';
import { renderMarkdown } from '../utils/markdown';

// Props 定義
const props = defineProps<{
  planContent?: string;     // 計畫內容
  planFilePath?: string;    // 計畫檔案路徑
}>();

// Emits 定義
const emit = defineEmits<{
  (e: 'respond', response: 'approve-auto' | 'approve-manual' | 'keep-planning' | 'custom', customMessage?: string): void;
}>();

// 自訂回應輸入
const customResponse = ref('');
const showCustomInput = ref(false);

// 計算顯示的計畫內容（渲染 Markdown）
const displayContent = computed(() => {
  if (!props.planContent) return '';
  return renderMarkdown(props.planContent);
});

// 處理回應
function handleResponse(response: 'approve-auto' | 'approve-manual' | 'keep-planning') {
  emit('respond', response);
}

// 顯示自訂輸入
function showCustom() {
  showCustomInput.value = true;
}

// 送出自訂回應
function submitCustom() {
  if (customResponse.value.trim()) {
    emit('respond', 'custom', customResponse.value.trim());
  }
}

// 按 Enter 送出、Escape 取消
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    submitCustom();
  } else if (e.key === 'Escape') {
    showCustomInput.value = false;
    customResponse.value = '';
  }
}
</script>

<template>
  <div class="plan-approval-dialog">
    <!-- 標題 -->
    <div class="dialog-header">
      <span class="plan-badge">ExitPlanMode</span>
      <span class="plan-title">計畫審核</span>
    </div>

    <!-- 計畫預覽 -->
    <div class="plan-preview">
      <div class="preview-label">計畫內容：</div>
      <div class="preview-content plan-markdown" v-html="displayContent"></div>
      <div v-if="planFilePath" class="plan-path">
        檔案：{{ planFilePath }}
      </div>
    </div>

    <!-- 問題提示 -->
    <div class="question">
      計畫已經準備好了，確認後我們就可以開始執行囉！
    </div>

    <!-- 選項按鈕 -->
    <div class="options">
      <button class="option-btn primary" @click="handleResponse('approve-auto')">
        <span class="option-number">1</span>
        <span class="option-label">批准計畫，自動執行編輯</span>
      </button>

      <button class="option-btn" @click="handleResponse('approve-manual')">
        <span class="option-number">2</span>
        <span class="option-label">批准計畫，手動審核編輯</span>
      </button>

      <button class="option-btn" @click="handleResponse('keep-planning')">
        <span class="option-number">3</span>
        <span class="option-label">繼續規劃</span>
      </button>

      <button class="option-btn secondary" @click="showCustom">
        <span class="option-number">4</span>
        <span class="option-label">給阿宇其他指示</span>
      </button>
    </div>

    <!-- 自訂輸入區域 -->
    <div v-if="showCustomInput" class="custom-input-area">
      <textarea
        v-model="customResponse"
        placeholder="告訴阿宇該怎麼做..."
        @keydown="handleKeydown"
        rows="3"
      ></textarea>
      <div class="custom-actions">
        <button class="cancel-btn" @click="showCustomInput = false; customResponse = ''">取消</button>
        <button class="submit-btn" @click="submitCustom" :disabled="!customResponse.trim()">送出</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.plan-approval-dialog {
  background-color: rgba(0, 0, 0, 0.3);
  border-radius: 8px;
  padding: 16px;
  margin: 12px 0;
  border-left: 3px solid #3498db;
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.plan-badge {
  background-color: #3498db;
  color: white;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 0.85em;
  font-weight: 600;
}

.plan-title {
  color: var(--text-color);
  font-weight: 600;
  font-size: 1em;
}

.plan-preview {
  margin-bottom: 12px;
  border-radius: 6px;
  overflow: hidden;
  background-color: rgba(52, 152, 219, 0.1);
  border: 1px solid rgba(52, 152, 219, 0.3);
}

.preview-label {
  padding: 8px 12px;
  background-color: rgba(52, 152, 219, 0.15);
  color: var(--text-muted);
  font-size: 0.85em;
  border-bottom: 1px solid rgba(52, 152, 219, 0.2);
}

.preview-content {
  margin: 0;
  padding: 12px;
  font-size: 1rem;
  color: var(--text-color);
  max-height: 300px;
  overflow-y: auto;
}

/* Markdown 渲染樣式 */
.plan-markdown :deep(h1),
.plan-markdown :deep(h2),
.plan-markdown :deep(h3) {
  margin: 0.6em 0 0.3em;
  color: var(--text-color);
}

.plan-markdown :deep(h1) { font-size: 1.3em; }
.plan-markdown :deep(h2) { font-size: 1.15em; }
.plan-markdown :deep(h3) { font-size: 1em; }

.plan-markdown :deep(p) {
  margin: 0.5em 0;
  line-height: 1.5;
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
  background-color: rgba(255, 255, 255, 0.1);
  padding: 0.1em 0.4em;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.9em;
}

.plan-markdown :deep(pre) {
  background-color: rgba(0, 0, 0, 0.3);
  padding: 8px 12px;
  border-radius: 4px;
  overflow-x: auto;
  margin: 0.5em 0;
}

.plan-markdown :deep(pre code) {
  background-color: transparent;
  padding: 0;
}

/* Table 樣式 */
.plan-markdown :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: 0.75em 0;
  font-size: 0.9em;
  border-radius: 8px;
  overflow: hidden;
}

.plan-markdown :deep(thead) {
  background-color: rgba(74, 144, 217, 0.2);
}

.plan-markdown :deep(th) {
  padding: 10px 12px;
  text-align: left;
  font-weight: 600;
  color: var(--primary-light);
  border-bottom: 2px solid var(--primary-color);
}

.plan-markdown :deep(td) {
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
}

.plan-markdown :deep(tbody tr) {
  transition: background-color 0.15s;
}

.plan-markdown :deep(tbody tr:hover) {
  background-color: rgba(255, 255, 255, 0.05);
}

.plan-markdown :deep(tbody tr:last-child td) {
  border-bottom: none;
}

/* 交替行底色 */
.plan-markdown tbody tr:nth-child(even) {
  background-color: rgba(255, 255, 255, 0.02);
}

.plan-path {
  padding: 8px 12px;
  background-color: rgba(52, 152, 219, 0.1);
  color: var(--text-muted);
  font-size: 0.8em;
  border-top: 1px solid rgba(52, 152, 219, 0.2);
  font-family: 'Consolas', 'Monaco', monospace;
}

.question {
  color: var(--text-color);
  font-size: 0.95em;
  margin-bottom: 12px;
  padding: 8px 0;
  border-top: 1px solid var(--border-color);
}

.options {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.option-btn {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  background-color: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  color: var(--text-color);
  font-size: 0.9em;
  cursor: pointer;
  transition: all 0.2s;
  text-align: left;
}

.option-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  border-color: var(--primary-color);
}

.option-btn.primary {
  background-color: rgba(46, 204, 113, 0.2);
  border-color: #2ecc71;
}

.option-btn.primary:hover {
  background-color: rgba(46, 204, 113, 0.3);
}

.option-btn.danger {
  background-color: rgba(231, 76, 60, 0.1);
  border-color: rgba(231, 76, 60, 0.3);
}

.option-btn.danger:hover {
  background-color: rgba(231, 76, 60, 0.2);
  border-color: #e74c3c;
}

.option-number {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  font-size: 0.8em;
  font-weight: 600;
  color: var(--text-muted);
}

.option-btn.primary .option-number {
  background-color: rgba(46, 204, 113, 0.3);
  color: #2ecc71;
}

.option-btn.danger .option-number {
  background-color: rgba(231, 76, 60, 0.2);
  color: #e74c3c;
}

.option-label {
  flex: 1;
}

.option-btn.secondary {
  background-color: rgba(52, 152, 219, 0.1);
  border-color: rgba(52, 152, 219, 0.3);
}

.option-btn.secondary:hover {
  background-color: rgba(52, 152, 219, 0.2);
  border-color: #3498db;
}

.option-btn.secondary .option-number {
  background-color: rgba(52, 152, 219, 0.2);
  color: #3498db;
}

/* 自訂輸入區域 */
.custom-input-area {
  margin-top: 12px;
  padding: 12px;
  background-color: rgba(52, 152, 219, 0.1);
  border-radius: 6px;
  border: 1px solid rgba(52, 152, 219, 0.3);
}

.custom-input-area textarea {
  width: 100%;
  padding: 10px;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: rgba(0, 0, 0, 0.2);
  color: var(--text-color);
  font-family: inherit;
  font-size: 0.9em;
  resize: vertical;
  min-height: 60px;
}

.custom-input-area textarea:focus {
  outline: none;
  border-color: #3498db;
}

.custom-input-area textarea::placeholder {
  color: var(--text-muted);
}

.custom-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}

.cancel-btn,
.submit-btn {
  padding: 6px 16px;
  border-radius: 4px;
  font-size: 0.85em;
  cursor: pointer;
  transition: all 0.2s;
}

.cancel-btn {
  background-color: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-muted);
}

.cancel-btn:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.submit-btn {
  background-color: #3498db;
  border: 1px solid #3498db;
  color: white;
}

.submit-btn:hover:not(:disabled) {
  background-color: #2980b9;
}

.submit-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}



</style>
