<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { X } from 'lucide-vue-next';

const props = defineProps<{
  imageBase64: string;
  mediaType?: string;  // 例如 'image/png', 'image/jpeg'
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

// 計算完整的 data URL
const imageSrc = computed(() => {
  const type = props.mediaType || 'image/png';
  return `data:${type};base64,${props.imageBase64}`;
});

// ESC 關閉
function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    emit('close');
  }
}

// 點擊背景關閉
function handleBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) {
    emit('close');
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div class="image-modal-backdrop" @click="handleBackdropClick">
      <div class="image-modal-container">
        <!-- 關閉按鈕 -->
        <button class="close-btn" @click="emit('close')" title="關閉 (ESC)">
          <X :size="24" />
        </button>

        <!-- 圖片 -->
        <img :src="imageSrc" alt="Image preview" class="modal-image" />
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.image-modal-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.85);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  padding: 40px;
}

.image-modal-container {
  position: relative;
  max-width: 100%;
  max-height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn {
  position: absolute;
  top: -40px;
  right: -40px;
  width: 36px;
  height: 36px;
  border: none;
  background-color: rgba(255, 255, 255, 0.15);
  color: #fff;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  z-index: 1;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.3);
  transform: scale(1.1);
}

.modal-image {
  max-width: calc(100vw - 80px);
  max-height: calc(100vh - 80px);
  object-fit: contain;
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
}

/* 小螢幕調整 */
@media (max-width: 768px) {
  .image-modal-backdrop {
    padding: 20px;
  }

  .close-btn {
    top: -30px;
    right: 0;
  }

  .modal-image {
    max-width: calc(100vw - 40px);
    max-height: calc(100vh - 60px);
  }
}
</style>
