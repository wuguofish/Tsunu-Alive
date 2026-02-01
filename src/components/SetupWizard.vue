<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface AddonStatus {
  vscode_available: boolean;
  vscode_installed: boolean;
  claude_available: boolean;
  skill_installed: boolean;
}

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const status = ref<AddonStatus | null>(null);
const loading = ref(true);
const installing = ref<string | null>(null); // 'vscode' | 'skill' | null
const installResults = ref<Record<string, { success: boolean; message: string }>>({});

// 勾選狀態
const installVscode = ref(true);
const installSkill = ref(true);

onMounted(async () => {
  try {
    status.value = await invoke<AddonStatus>('check_addon_status');
  } catch (e) {
    console.error('Failed to check addon status:', e);
  } finally {
    loading.value = false;
  }
});

async function handleInstall() {
  const tasks: { key: string; label: string; fn: () => Promise<string> }[] = [];

  if (installVscode.value && status.value?.vscode_available && !status.value?.vscode_installed) {
    tasks.push({
      key: 'vscode',
      label: 'VS Code Extension',
      fn: () => invoke<string>('install_vscode_extension'),
    });
  }

  if (installSkill.value && !status.value?.skill_installed) {
    tasks.push({
      key: 'skill',
      label: 'Claude Code Skill',
      fn: () => invoke<string>('install_skill'),
    });
  }

  for (const task of tasks) {
    installing.value = task.key;
    try {
      const msg = await task.fn();
      installResults.value[task.key] = { success: true, message: msg };
    } catch (e) {
      installResults.value[task.key] = { success: false, message: String(e) };
    }
  }

  installing.value = null;

  // 標記 setup 完成
  try {
    await invoke('mark_setup_done');
  } catch (e) {
    console.error('Failed to mark setup done:', e);
  }

  // 重新載入狀態
  try {
    status.value = await invoke<AddonStatus>('check_addon_status');
  } catch (e) { /* ignore */ }
}

function handleSkip() {
  // 標記完成，不再顯示
  invoke('mark_setup_done').catch(() => {});
  emit('close');
}

function handleDone() {
  emit('close');
}

// 是否有東西可以安裝
function hasInstallableItems(): boolean {
  if (!status.value) return false;
  return (status.value.vscode_available && !status.value.vscode_installed) ||
         !status.value.skill_installed;
}

// 是否有安裝結果
function hasResults(): boolean {
  return Object.keys(installResults.value).length > 0;
}
</script>

<template>
  <div class="setup-overlay">
    <div class="setup-wizard">
      <!-- Header -->
      <div class="wizard-header">
        <h2>歡迎使用 Tsunu Alive!</h2>
        <p class="subtitle">以下是可選的附加組件，安裝後可以獲得更完整的體驗</p>
      </div>

      <!-- Loading -->
      <div v-if="loading" class="wizard-loading">
        偵測中...
      </div>

      <!-- Content -->
      <div v-else-if="status" class="wizard-content">
        <!-- VS Code Extension -->
        <label class="addon-item" :class="{ disabled: !status.vscode_available || status.vscode_installed }">
          <input
            type="checkbox"
            v-model="installVscode"
            :disabled="!status.vscode_available || status.vscode_installed"
          />
          <div class="addon-info">
            <div class="addon-name">
              VS Code Extension
              <span v-if="status.vscode_installed" class="badge installed">已安裝</span>
              <span v-else-if="!status.vscode_available" class="badge unavailable">未偵測到 VS Code</span>
            </div>
            <div class="addon-desc">讓阿宇能讀取你的 VS Code 編輯器內容</div>
          </div>
          <!-- 安裝結果 -->
          <div v-if="installResults.vscode" class="install-result" :class="{ success: installResults.vscode.success, error: !installResults.vscode.success }">
            {{ installResults.vscode.success ? '安裝成功' : installResults.vscode.message }}
          </div>
        </label>

        <!-- Claude Code Skill -->
        <label class="addon-item" :class="{ disabled: status.skill_installed }">
          <input
            type="checkbox"
            v-model="installSkill"
            :disabled="status.skill_installed"
          />
          <div class="addon-info">
            <div class="addon-name">
              Claude Code Skill
              <span v-if="status.skill_installed" class="badge installed">已安裝</span>
            </div>
            <div class="addon-desc">讓 Claude Code CLI 也能使用阿宇的完整人設</div>
          </div>
          <!-- 安裝結果 -->
          <div v-if="installResults.skill" class="install-result" :class="{ success: installResults.skill.success, error: !installResults.skill.success }">
            {{ installResults.skill.success ? '安裝成功' : installResults.skill.message }}
          </div>
        </label>

        <!-- 安裝中提示 -->
        <div v-if="installing" class="installing-status">
          正在安裝 {{ installing === 'vscode' ? 'VS Code Extension' : 'Claude Code Skill' }}...
        </div>
      </div>

      <!-- Footer -->
      <div class="wizard-footer">
        <template v-if="hasResults()">
          <button class="btn primary" @click="handleDone">完成</button>
        </template>
        <template v-else>
          <button class="btn secondary" @click="handleSkip" :disabled="!!installing">
            略過，稍後再說
          </button>
          <button
            class="btn primary"
            @click="handleInstall"
            :disabled="!!installing || !hasInstallableItems()"
          >
            {{ installing ? '安裝中...' : '安裝選取的項目' }}
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.setup-wizard {
  background: var(--bg-secondary, #16213e);
  border: 1px solid var(--border-color, #3a3a5c);
  border-radius: 12px;
  width: 480px;
  max-width: 90vw;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.wizard-header {
  padding: 24px 24px 16px;
  border-bottom: 1px solid var(--border-color, #3a3a5c);
}

.wizard-header h2 {
  margin: 0 0 8px;
  font-size: 18px;
  color: var(--text-color, #e8e8e8);
}

.subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--text-muted, #a0a0a0);
}

.wizard-loading {
  padding: 32px;
  text-align: center;
  color: var(--text-muted);
}

.wizard-content {
  padding: 16px 24px;
}

.addon-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.15s;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.addon-item:hover:not(.disabled) {
  background: rgba(255, 255, 255, 0.05);
}

.addon-item.disabled {
  opacity: 0.6;
  cursor: default;
}

.addon-item input[type="checkbox"] {
  margin-top: 3px;
  accent-color: var(--primary-color, #4a90d9);
}

.addon-info {
  flex: 1;
  min-width: 0;
}

.addon-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-color, #e8e8e8);
  display: flex;
  align-items: center;
  gap: 8px;
}

.addon-desc {
  font-size: 12px;
  color: var(--text-muted, #a0a0a0);
  margin-top: 2px;
}

.badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 500;
}

.badge.installed {
  background: rgba(72, 199, 142, 0.2);
  color: #48c78e;
}

.badge.unavailable {
  background: rgba(255, 159, 67, 0.2);
  color: #ff9f43;
}

.install-result {
  width: 100%;
  font-size: 12px;
  padding: 4px 0 0 28px;
}

.install-result.success {
  color: #48c78e;
}

.install-result.error {
  color: #ff6b6b;
}

.installing-status {
  text-align: center;
  padding: 8px;
  font-size: 13px;
  color: var(--primary-light, #6ba3e0);
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.wizard-footer {
  padding: 16px 24px;
  border-top: 1px solid var(--border-color, #3a3a5c);
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.btn {
  padding: 8px 16px;
  border-radius: 6px;
  border: none;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s;
}

.btn.primary {
  background: var(--primary-color, #4a90d9);
  color: white;
}

.btn.primary:hover:not(:disabled) {
  background: var(--primary-dark, #357abd);
}

.btn.primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.secondary {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-muted, #a0a0a0);
}

.btn.secondary:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}

.btn.secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
