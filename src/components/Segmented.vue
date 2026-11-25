<script setup lang="ts" generic="T extends string | number">
// 分段控件：玻璃滑块按选中按钮的实际位置和宽度弹性移动（按钮宽度可以不一样）
//  - 选项可带 icon（instanceIcons 中的图标名），显示在文字左侧
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { iconUrl } from '../instanceIcons'

const props = defineProps<{
  modelValue: T
  options: { value: T; label: string; disabled?: boolean; icon?: string; title?: string }[]
}>()
const emit = defineEmits<{ (e: 'update:modelValue', v: T): void }>()

const root = ref<HTMLElement | null>(null)
const btns = ref<HTMLElement[]>([])
const thumb = ref({ x: 0, w: 0 })
const ready = ref(false)
const idx = computed(() => Math.max(0, props.options.findIndex((o) => o.value === props.modelValue)))

function measure() {
  const b = btns.value[idx.value]
  if (!b) return
  thumb.value = { x: b.offsetLeft, w: b.offsetWidth }
}
let ro: ResizeObserver | undefined
onMounted(async () => {
  await nextTick()
  measure()
  ro = new ResizeObserver(measure)
  if (root.value) ro.observe(root.value)
  btns.value.forEach((b) => ro?.observe(b))
  // 首次定位不做动画
  requestAnimationFrame(() => (ready.value = true))
})
onBeforeUnmount(() => ro?.disconnect())
watch([idx, () => props.options.map((o) => o.label).join('|')], () => nextTick(measure))
</script>

<template>
  <div ref="root" class="segmented">
    <span
      class="thumb"
      :class="{ anim: ready }"
      :style="{ width: `${thumb.w}px`, transform: `translateX(${thumb.x}px)`, opacity: thumb.w ? 1 : 0 }"
    />
    <button
      v-for="o in options"
      :key="String(o.value)"
      ref="btns"
      class="seg"
      :class="{ active: o.value === modelValue }"
      :disabled="o.disabled"
      :title="o.title"
      @click="!o.disabled && emit('update:modelValue', o.value)"
    >
      <img v-if="o.icon" class="seg-ico" :src="iconUrl(o.icon)" alt="" draggable="false" />
      {{ o.label }}
    </button>
  </div>
</template>

<style scoped>
.segmented {
  position: relative;
  display: inline-flex;
  flex: none;
  padding: 3px;
  border-radius: 13px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.08);
  max-width: 100%;
}
.thumb {
  position: absolute;
  top: 3px;
  bottom: 3px;
  left: 0;
  border-radius: 10px;
  background: var(--glass-bg-strong);
  box-shadow: 0 3px 10px -3px rgba(0, 0, 0, 0.25), inset 0 1px 0 var(--hl-top), inset 0 0 0 0.5px var(--glass-edge);
  pointer-events: none;
}
.thumb.anim {
  transition: transform 0.55s var(--spring), width 0.55s var(--spring);
}
.seg {
  position: relative;
  z-index: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  /* 容器被拉宽时（如下载页标签栏）按钮平分多出的宽度；否则按文字宽度 */
  flex: 1 0 auto;
  height: 30px;
  padding: 0 14px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 13px;
  font-weight: 550;
  color: var(--text-2);
  white-space: nowrap;
  transition: color 0.2s ease;
}
.seg.active {
  color: var(--text);
}
.seg-ico {
  width: 16px;
  height: 16px;
  flex: none;
  pointer-events: none;
  filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.18));
}
.seg:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
