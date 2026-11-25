<script setup lang="ts">
import Icon from './Icon.vue'
import { store } from '../store'

const icons = { info: 'info', success: 'check', error: 'alert' } as const
</script>

<template>
  <div class="toasts">
    <TransitionGroup name="toast">
      <div v-for="t in store.toasts" :key="t.id" class="toast glass glass-strong" :class="t.kind">
        <Icon :name="icons[t.kind]" :size="18" />
        <span class="text">{{ t.text }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toasts {
  position: fixed;
  left: 50%;
  bottom: 26px;
  transform: translateX(-50%);
  z-index: 300;
  display: flex;
  flex-direction: column-reverse;
  align-items: center;
  gap: 8px;
  pointer-events: none;
}
.toast {
  --radius: 18px;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  max-width: 560px;
  padding: 11px 16px;
  font-size: 13.5px;
  pointer-events: auto;
}
.text {
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
}
.toast.success :deep(.icon) {
  color: var(--success);
}
.toast.error :deep(.icon) {
  color: var(--danger);
}
.toast.info :deep(.icon) {
  color: var(--accent);
}
.toast-enter-active {
  transition: transform 0.6s var(--spring), opacity 0.25s ease;
}
.toast-leave-active {
  transition: transform 0.25s ease, opacity 0.25s ease;
}
.toast-enter-from {
  transform: translateY(24px) scale(0.8);
  opacity: 0;
}
.toast-leave-to {
  transform: scale(0.9);
  opacity: 0;
}
</style>
