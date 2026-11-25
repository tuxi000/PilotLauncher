<script setup lang="ts">
// 玻璃风格复选框：替代原生 checkbox（勾选时弹性缩放）
import Icon from './Icon.vue'

defineProps<{ modelValue: boolean; disabled?: boolean; label?: string }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()
</script>

<template>
  <button
    type="button"
    class="gc"
    :class="{ on: modelValue }"
    role="checkbox"
    :aria-checked="modelValue"
    :disabled="disabled"
    @click.stop="emit('update:modelValue', !modelValue)"
  >
    <span class="box"><Icon name="check" :size="12" :stroke="3" class="tick" /></span>
    <span v-if="label || $slots.default" class="gc-label"><slot>{{ label }}</slot></span>
  </button>
</template>

<style scoped>
.gc {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
  color: inherit;
  font: inherit;
  text-align: left;
}
.gc:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.box {
  width: 18px;
  height: 18px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 6px;
  color: #fff;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1.2px color-mix(in srgb, var(--text) 24%, transparent), inset 0 1px 0 var(--hl-bottom);
  transition: background 0.2s ease, box-shadow 0.2s ease, transform 0.45s var(--spring);
}
.gc:hover:not(:disabled) .box {
  box-shadow: inset 0 0 0 1.2px color-mix(in srgb, var(--accent) 60%, transparent);
}
.gc:active:not(:disabled) .box {
  transform: scale(0.86);
}
.gc.on .box {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 80%, white), var(--accent));
  box-shadow: 0 3px 8px -3px color-mix(in srgb, var(--accent) 70%, transparent), inset 0 1px 0 rgba(255, 255, 255, 0.45);
}
.tick {
  opacity: 0;
  transform: scale(0.4);
  transition: transform 0.45s var(--spring), opacity 0.15s ease;
}
.gc.on .tick {
  opacity: 1;
  transform: scale(1);
}
.gc:focus-visible {
  outline: none;
}
.gc:focus-visible .box {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent), inset 0 0 0 1.2px var(--accent);
}
.gc-label {
  min-width: 0;
}
</style>
