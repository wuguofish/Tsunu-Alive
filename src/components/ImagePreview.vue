<script setup lang="ts">
import { computed } from 'vue';

// 附加圖片項目
export interface AttachedImage {
  id: string;          // 唯一識別碼
  path: string;        // 圖片路徑（臨時檔案或原始檔案）
  name: string;        // 顯示名稱
  isTemp: boolean;     // 是否為臨時檔案（剪貼簿圖片）
  previewUrl?: string; // 預覽用的 blob URL
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
  <div class="image-preview">
    <!-- 縮圖 -->
    <div class="thumbnail">
      <img
        v-if="image.previewUrl"
        :src="image.previewUrl"
        :alt="image.name"
      />
      <div v-else class="placeholder">📷</div>
    </div>

    <!-- 檔案名稱 -->
    <span class="filename" :title="image.name">{{ displayName }}</span>

    <!-- 移除按鈕 -->
    <button class="remove-btn" @click="handleRemove" title="移除圖片">×</button>
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
</style>
