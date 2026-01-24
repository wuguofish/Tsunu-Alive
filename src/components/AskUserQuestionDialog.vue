<script setup lang="ts">
import { ref, computed } from 'vue';

// 選項類型
interface QuestionOption {
  label: string;
  description?: string;
}

// 問題類型
interface Question {
  question: string;
  header: string;
  options: QuestionOption[];
  multiSelect: boolean;
}

// Props
const props = defineProps<{
  questions: Question[];
}>();

// Emits
const emit = defineEmits<{
  (e: 'submit', answers: Record<string, string>): void;
  (e: 'cancel'): void;
}>();

// 每個問題的選擇狀態
const selectedAnswers = ref<Record<number, Set<number>>>({});
const customInputs = ref<Record<number, string>>({});

// 初始化選擇狀態
props.questions.forEach((_, index) => {
  selectedAnswers.value[index] = new Set();
  customInputs.value[index] = '';
});

// 是否選擇了 Other
function isOtherSelected(questionIndex: number): boolean {
  const optionCount = props.questions[questionIndex].options.length;
  return selectedAnswers.value[questionIndex].has(optionCount);
}

// 處理選項點擊
function toggleOption(questionIndex: number, optionIndex: number) {
  const question = props.questions[questionIndex];
  const selected = selectedAnswers.value[questionIndex];

  if (question.multiSelect) {
    if (selected.has(optionIndex)) {
      selected.delete(optionIndex);
    } else {
      selected.add(optionIndex);
    }
  } else {
    // 單選：清除其他選項
    selected.clear();
    selected.add(optionIndex);
  }
}

// 檢查是否所有問題都已回答
const canSubmit = computed(() => {
  return props.questions.every((_, index) => {
    const selected = selectedAnswers.value[index];
    if (selected.size === 0) return false;
    // 如果選了 Other，需要有自訂輸入
    if (isOtherSelected(index) && !customInputs.value[index].trim()) {
      return false;
    }
    return true;
  });
});

// 提交答案
function submit() {
  const answers: Record<string, string> = {};

  props.questions.forEach((question, index) => {
    const selected = selectedAnswers.value[index];
    const selectedLabels: string[] = [];

    selected.forEach(optionIndex => {
      if (optionIndex < question.options.length) {
        selectedLabels.push(question.options[optionIndex].label);
      } else {
        // Other 選項
        selectedLabels.push(customInputs.value[index]);
      }
    });

    // 用問題文字作為 key
    answers[question.question] = selectedLabels.join(', ');
  });

  emit('submit', answers);
}
</script>

<template>
  <div class="ask-user-dialog-overlay">
    <div class="ask-user-dialog">
      <!-- 標題列 -->
      <div class="dialog-header">
        <span v-for="(q, i) in questions" :key="i" class="header-chip">
          {{ q.header }}
        </span>
      </div>

      <!-- 問題列表 -->
      <div class="questions-container">
        <div v-for="(question, qIndex) in questions" :key="qIndex" class="question-block">
          <div class="question-text">{{ question.question }}</div>

          <div class="options-list">
            <!-- 預設選項 -->
            <div
              v-for="(option, oIndex) in question.options"
              :key="oIndex"
              class="option-item"
              :class="{ selected: selectedAnswers[qIndex].has(oIndex) }"
              @click="toggleOption(qIndex, oIndex)"
            >
              <div class="option-radio">
                <span v-if="question.multiSelect" class="checkbox">
                  {{ selectedAnswers[qIndex].has(oIndex) ? '☑' : '☐' }}
                </span>
                <span v-else class="radio">
                  {{ selectedAnswers[qIndex].has(oIndex) ? '◉' : '○' }}
                </span>
              </div>
              <div class="option-content">
                <div class="option-label">{{ option.label }}</div>
                <div v-if="option.description" class="option-description">
                  {{ option.description }}
                </div>
              </div>
            </div>

            <!-- Other 選項 -->
            <div
              class="option-item other-option"
              :class="{ selected: isOtherSelected(qIndex) }"
              @click="toggleOption(qIndex, question.options.length)"
            >
              <div class="option-radio">
                <span v-if="question.multiSelect" class="checkbox">
                  {{ isOtherSelected(qIndex) ? '☑' : '☐' }}
                </span>
                <span v-else class="radio">
                  {{ isOtherSelected(qIndex) ? '◉' : '○' }}
                </span>
              </div>
              <div class="option-content">
                <div class="option-label">Other</div>
              </div>
            </div>

            <!-- Other 輸入框 -->
            <div v-if="isOtherSelected(qIndex)" class="other-input-container">
              <input
                v-model="customInputs[qIndex]"
                type="text"
                class="other-input"
                placeholder="請輸入自訂回答..."
                @click.stop
              />
            </div>
          </div>
        </div>
      </div>

      <!-- 底部按鈕 -->
      <div class="dialog-footer">
        <button class="btn btn-secondary" @click="$emit('cancel')">
          取消
        </button>
        <button
          class="btn btn-primary"
          :disabled="!canSubmit"
          @click="submit"
        >
          Submit answers
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ask-user-dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.ask-user-dialog {
  background-color: var(--bg-secondary);
  border-radius: 12px;
  border: 1px solid var(--border-color);
  width: 90%;
  max-width: 500px;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.dialog-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.header-chip {
  background-color: rgba(74, 144, 217, 0.2);
  color: var(--primary-light);
  padding: 4px 12px;
  border-radius: 16px;
  font-size: 0.85rem;
  font-weight: 500;
}

.questions-container {
  padding: 16px 20px;
  overflow-y: auto;
  flex: 1;
}

.question-block {
  margin-bottom: 20px;
}

.question-block:last-child {
  margin-bottom: 0;
}

.question-text {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-color);
  margin-bottom: 12px;
}

.options-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.option-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
  border: 1px solid transparent;
}

.option-item:hover {
  background-color: rgba(0, 0, 0, 0.3);
}

.option-item.selected {
  background-color: rgba(74, 144, 217, 0.15);
  border-color: var(--primary-color);
}

.option-radio {
  font-size: 1.1rem;
  line-height: 1;
  color: var(--text-muted);
}

.option-item.selected .option-radio {
  color: var(--primary-color);
}

.option-content {
  flex: 1;
}

.option-label {
  font-size: 0.9rem;
  color: var(--text-color);
  font-weight: 500;
}

.option-description {
  font-size: 0.8rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.other-input-container {
  margin-top: 4px;
  padding-left: 36px;
}

.other-input {
  width: 100%;
  padding: 8px 12px;
  background-color: rgba(0, 0, 0, 0.3);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  color: var(--text-color);
  font-size: 0.9rem;
}

.other-input:focus {
  outline: none;
  border-color: var(--primary-color);
}

.other-input::placeholder {
  color: var(--text-muted);
}

.dialog-footer {
  padding: 16px 20px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.btn {
  padding: 8px 20px;
  border-radius: 6px;
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  border: none;
}

.btn-secondary {
  background-color: rgba(255, 255, 255, 0.1);
  color: var(--text-color);
}

.btn-secondary:hover {
  background-color: rgba(255, 255, 255, 0.15);
}

.btn-primary {
  background-color: var(--primary-color);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-dark);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
