<script setup lang="ts">
// 通用玻璃弹窗：弹性缩放进入，点击遮罩或 Esc 关闭
import { onMounted, onUnmounted } from 'vue'
import Icon from './Icon.vue'

const props = withDefaults(defineProps<{ open: boolean; title?: string; width?: number; closable?: boolean }>(), {
  title: '',
  width: 480,
  closable: true,
})
const emit = defineEmits<{ (e: 'close'): void }>()

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.open && props.closable) emit('close')
}
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="open" class="scrim" @mousedown.self="closable && emit('close')">
        <div class="dialog glass glass-strong" :style="{ width: `${width}px` }" v-refract="{ scale: 30 }" v-light>
          <header v-if="title || closable" class="head">
            <h3>{{ title }}</h3>
            <span class="spacer" />
            <button v-if="closable" class="btn ghost sm icon" title="关闭" @click="emit('close')">
              <Icon name="close" :size="16" />
            </button>
          </header>
          <div class="body">
            <slot />
          </div>
          <footer v-if="$slots.footer" class="foot">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: grid;
  place-items: center;
  background: var(--scrim);
  -webkit-backdrop-filter: blur(6px);
  backdrop-filter: blur(6px);
}
.dialog {
  --radius: 26px;
  --blur: 34px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
}
.head {
  display: flex;
  align-items: center;
  padding: 16px 16px 4px 22px;
}
.head h3 {
  font-size: 17px;
}
.body {
  padding: 12px 22px 18px;
  overflow: auto;
}
.foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 0 22px 18px;
}

.dialog-enter-active {
  transition: opacity 0.25s ease;
}
.dialog-enter-active .dialog {
  transition: transform 0.6s var(--spring), opacity 0.25s ease;
}
.dialog-leave-active {
  transition: opacity 0.18s ease;
}
.dialog-leave-active .dialog {
  transition: transform 0.18s ease, opacity 0.18s ease;
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
.dialog-enter-from .dialog {
  transform: scale(0.86) translateY(16px);
  opacity: 0;
}
.dialog-leave-to .dialog {
  transform: scale(0.96);
  opacity: 0;
}
</style>
