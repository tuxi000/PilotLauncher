<script setup lang="ts">
// iOS 风格开关，按下时拇指横向拉伸
defineProps<{ modelValue: boolean; disabled?: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()
</script>

<template>
  <button
    class="switch"
    :class="{ on: modelValue }"
    role="switch"
    :aria-checked="modelValue"
    :disabled="disabled"
    @click="emit('update:modelValue', !modelValue)"
  >
    <span class="knob" />
  </button>
</template>

<style scoped>
.switch {
  position: relative;
  width: 48px;
  height: 28px;
  flex: none;
  border: none;
  border-radius: 14px;
  cursor: pointer;
  background: color-mix(in srgb, var(--text) 16%, transparent);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.15);
  transition: background 0.25s ease;
}
.switch.on {
  background: var(--success);
}
.switch:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 24px;
  height: 24px;
  border-radius: 12px;
  background: #fff;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
  transition: transform 0.5s var(--spring), width 0.3s var(--spring);
}
.switch.on .knob {
  transform: translateX(20px);
}
.switch:active .knob {
  width: 30px;
}
.switch.on:active .knob {
  transform: translateX(14px);
}
</style>
