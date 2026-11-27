<script setup lang="ts">
// 提示条：弹簧弹入，图标轻轻弹出；多条提示之间的位置变化也用弹簧过渡
import Icon from './Icon.vue'
import { store } from '../store'

const icons = { info: 'info', success: 'check', error: 'alert' } as const
</script>

<template>
  <div class="toasts">
    <TransitionGroup name="toast">
      <div v-for="t in store.toasts" :key="t.id" class="toast glass glass-strong" :class="t.kind">
        <span class="t-ico"><Icon :name="icons[t.kind]" :size="18" /></span>
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
.t-ico {
  display: grid;
  place-items: center;
  animation: pop-in 0.55s var(--spring) 0.08s both;
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
  transition: transform 0.6s var(--spring), opacity 0.22s ease;
}
.toast-leave-active {
  transition: transform 0.28s var(--ease-out), opacity 0.22s ease;
}
.toast-move {
  transition: transform 0.5s var(--spring);
}
.toast-enter-from {
  transform: translateY(24px) scale(0.8);
  opacity: 0;
}
.toast-leave-to {
  transform: translateY(8px) scale(0.9);
  opacity: 0;
}
/* 出错时轻轻左右晃一下 */
.toast.error .t-ico {
  animation: pop-in 0.55s var(--spring) 0.08s both, nudge 0.5s var(--ease-out) 0.45s;
}
@keyframes nudge {
  0%,
  100% {
    transform: none;
  }
  30% {
    transform: translateX(-3px) rotate(-8deg);
  }
  65% {
    transform: translateX(2px) rotate(5deg);
  }
}
</style>
