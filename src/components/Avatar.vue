<script setup lang="ts">
// 字母头像：根据 UUID 生成稳定的渐变色
import { computed } from 'vue'

const props = withDefaults(defineProps<{ name: string; seed?: string; size?: number }>(), {
  seed: '',
  size: 36,
})

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

const style = computed(() => {
  const h = hash(props.seed || props.name) % 360
  return {
    width: `${props.size}px`,
    height: `${props.size}px`,
    fontSize: `${Math.round(props.size * 0.42)}px`,
    background: `linear-gradient(135deg, hsl(${h} 80% 62%), hsl(${(h + 48) % 360} 75% 52%))`,
  }
})
const letter = computed(() => (props.name || '?').slice(0, 1).toUpperCase())
</script>

<template>
  <span class="avatar" :style="style">{{ letter }}</span>
</template>

<style scoped>
.avatar {
  display: grid;
  place-items: center;
  flex: none;
  border-radius: 30%;
  color: #fff;
  font-weight: 700;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.5), 0 3px 10px -3px rgba(0, 0, 0, 0.35);
}
</style>
