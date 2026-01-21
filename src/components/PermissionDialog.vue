<script setup lang="ts">
import { ref } from 'vue';

// Props 定義
const props = defineProps<{
  action: string;           // 操作類型，如 "Edit"
  target: string;           // 目標，如檔案路徑
  summary?: string;         // 摘要，如 "Added 2 lines"
  preview?: string;         // 預覽內容（程式碼片段）
}>();

// Emits 定義
const emit = defineEmits<{
  (e: 'respond', response: 'yes' | 'yes-all' | 'no' | 'custom', customMessage?: string): void;
}>();

// 自訂回應輸入
const customResponse = ref('');
const showCustomInput = ref(false);

// 處理回應
function handleResponse(response: 'yes' | 'yes-all' | 'no') {
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

// 按 Enter 送出自訂回應
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    submitCustom();
  }
}
</script>

<template>
  <div class="permission-dialog">
    <!-- 標題 -->
    <div class="dialog-header">
      <span class="action-badge">{{ action }}</span>
      <span class="target-path">{{ target }}</span>
      <span v-if="summary" class="summary">{{ summary }}</span>
    </div>

    <!-- 預覽內容 -->
    <div v-if="preview" class="preview-section">
      <pre class="preview-code"><code>{{ preview }}</code></pre>
    </div>

    <!-- 問題提示 -->
    <div class="question">
      Make this edit to {{ target }}?
    </div>

    <!-- 選項按鈕 -->
    <div class="options">
      <button class="option-btn primary" @click="handleResponse('yes')">
        <span class="option-number">1</span>
        <span class="option-label">Yes</span>
      </button>

      <button class="option-btn" @click="handleResponse('yes-all')">
        <span class="option-number">2</span>
        <span class="option-label">Yes, allow all edits this session</span>
      </button>

      <button class="option-btn" @click="handleResponse('no')">
        <span class="option-number">3</span>
        <span class="option-label">No</span>
      </button>

      <button v-if="!showCustomInput" class="option-btn custom-toggle" @click="showCustom">
        <span class="option-label">Tell Claude what to do instead</span>
      </button>

      <!-- 自訂輸入框 -->
      <div v-if="showCustomInput" class="custom-input-wrapper">
        <input
          v-model="customResponse"
          type="text"
          placeholder="Tell Claude what to do instead"
          class="custom-input"
          @keydown="handleKeydown"
          autofocus
        />
        <button class="submit-custom" @click="submitCustom" :disabled="!customResponse.trim()">
          送出
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.permission-dialog {
  background-color: rgba(0, 0, 0, 0.3);
  border-radius: 8px;
  padding: 16px;
  margin: 12px 0;
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.action-badge {
  background-color: #9b59b6;
  color: white;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 0.85em;
  font-weight: 600;
}

.target-path {
  color: var(--text-muted);
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary {
  color: var(--text-muted);
  font-size: 0.85em;
  margin-left: auto;
}

.preview-section {
  margin-bottom: 12px;
  border-radius: 6px;
  overflow: hidden;
  background-color: rgba(231, 76, 60, 0.15);
}

.preview-code {
  margin: 0;
  padding: 12px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85em;
  color: var(--text-color);
  white-space: pre-wrap;
  word-break: break-all;
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

.option-label {
  flex: 1;
}

.custom-toggle {
  color: var(--text-muted);
  border-style: dashed;
}

.custom-input-wrapper {
  display: flex;
  gap: 8px;
}

.custom-input {
  flex: 1;
  padding: 10px 14px;
  background-color: rgba(0, 0, 0, 0.3);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  color: var(--text-color);
  font-size: 0.9em;
  outline: none;
  transition: border-color 0.2s;
}

.custom-input:focus {
  border-color: var(--primary-color);
}

.custom-input::placeholder {
  color: var(--text-muted);
}

.submit-custom {
  padding: 10px 16px;
  background-color: var(--primary-color);
  border: none;
  border-radius: 6px;
  color: white;
  font-size: 0.9em;
  cursor: pointer;
  transition: background-color 0.2s;
}

.submit-custom:hover:not(:disabled) {
  background-color: var(--primary-dark);
}

.submit-custom:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
