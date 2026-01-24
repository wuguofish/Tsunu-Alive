<script setup lang="ts">
import { computed } from 'vue';
import { Image, X } from 'lucide-vue-next';

// 附加圖片項目
export interface AttachedImage {
  id: string;          // 唯一識別碼
  path: string;        // 圖片路徑（臨時檔案或原始檔案）
  name: string;        // 顯示名稱
  isTemp: boolean;     // 是否為臨時檔案（剪貼簿圖片）
  previewUrl?: string; // 預覽用的 blob URL
  isLoading?: boolean; // 是否正在處理中
}

const props = defineProps<{
  image: AttachedImage;
}>();

const emit = defineEmits<{
  (e: 'remove', id: string): void;
}>();

// 顯示縮短的檔案名稱
const displayName = computed(() => {
  const name = props.image.name;
  if (name.length > 20) {
    const ext = name.lastIndexOf('.');
    if (ext > 0) {
      const baseName = name.substring(0, ext);
      const extension = name.substring(ext);
      if (baseName.length > 15) {
        return baseName.substring(0, 12) + '...' + extension;
      }
    }
    return name.substring(0, 17) + '...';
  }
  return name;
});

function handleRemove() {
  emit('remove', props.image.id);
}
</script>

<template>
  <div class="image-preview" :class="{ loading: image.isLoading }">
    <!-- 縮圖 -->
    <div class="thumbnail">
      <!-- Loading 狀態 -->
      <div v-if="image.isLoading" class="loading-spinner"></div>
      <!-- 有預覽圖 -->
      <img
        v-else-if="image.previewUrl"
        :src="image.previewUrl"
        :alt="image.name"
      />
      <!-- 無預覽圖 -->
      <div v-else class="placeholder"><Image :size="18" /></div>
    </div>

    <!-- 檔案名稱 -->
    <span class="filename" :title="image.name">{{ displayName }}</span>

    <!-- 移除按鈕（loading 時不顯示） -->
    <button v-if="!image.isLoading" class="remove-btn" @click="handleRemove" title="移除圖片"><X :size="14" /></button>
  </div>
</template>

<style scoped>
.image-preview {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  background-color: rgba(255, 255, 255, 0.08);
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.15));
  border-radius: 8px;
  margin-right: 8px;
  margin-bottom: 8px;
}

.thumbnail {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.thumbnail img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.placeholder {
  font-size: 18px;
  opacity: 0.6;
}

.filename {
  font-size: 0.85em;
  color: var(--text-color, #fff);
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remove-btn {
  width: 20px;
  height: 20px;
  border: none;
  background-color: rgba(255, 255, 255, 0.1);
  color: var(--text-muted, rgba(255, 255, 255, 0.6));
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  line-height: 1;
  padding: 0;
  transition: all 0.2s;
}

.remove-btn:hover {
  background-color: rgba(231, 76, 60, 0.3);
  color: #e74c3c;
}

/* Loading 狀態 */
.image-preview.loading {
  opacity: 0.7;
}

.loading-spinner {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.2);
  border-top-color: var(--primary-light, #6ba3e0);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
