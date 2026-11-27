<script setup lang="ts">
// 数字平滑过渡：数值变化时在约 0.45 秒内从旧值滚动到新值（只改文字内容）。
// 关闭“界面动画”或系统开启“减少动画”时直接显示新值。
import { onBeforeUnmount, ref, watch } from 'vue'
import { motionOn } from '../motion'

const props = withDefaults(
  defineProps<{
    value: number
    /** 格式化函数，默认四舍五入成整数 */
    format?: (v: number) => string
    duration?: number
  }>(),
  {
    format: undefined,
    duration: 450,
  },
)

const shown = ref(Number.isFinite(props.value) ? props.value : 0)
let raf = 0

watch(
  () => props.value,
  (to) => {
    cancelAnimationFrame(raf)
    const from = shown.value
    if (!motionOn() || !Number.isFinite(from) || !Number.isFinite(to) || from === to || document.hidden) {
      shown.value = Number.isFinite(to) ? to : 0
      return
    }
    const start = performance.now()
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / props.duration)
      const e = 1 - Math.pow(1 - t, 3)
      shown.value = from + (to - from) * e
      if (t < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
  },
)
onBeforeUnmount(() => cancelAnimationFrame(raf))

function text(v: number) {
  return props.format ? props.format(v) : String(Math.round(v))
}
</script>

<template>
  <span class="anum">{{ text(shown) }}</span>
</template>

<style scoped>
.anum {
  font-variant-numeric: tabular-nums;
}
</style>
