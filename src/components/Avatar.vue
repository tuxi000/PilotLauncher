<script setup lang="ts">
// 头像：传入 account 时显示皮肤的头部（8×8 头 + 帽子层，像素放大）；
// 不传（例如联机房间里的其他玩家）或皮肤还没准备好时，显示按 UUID 生成渐变色的字母头像
import { computed, ref, watch } from 'vue'
import type { AccountView } from '../api'
import { headUrl, resolveSkin } from '../skin'

const props = withDefaults(
  defineProps<{ name: string; seed?: string; size?: number; account?: AccountView | null }>(),
  {
    seed: '',
    size: 36,
    account: null,
  },
)

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

const box = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
}))
const style = computed(() => {
  const h = hash(props.seed || props.name) % 360
  return {
    ...box.value,
    fontSize: `${Math.round(props.size * 0.42)}px`,
    background: `linear-gradient(135deg, hsl(${h} 80% 62%), hsl(${(h + 48) % 360} 75% 52%))`,
  }
})
const letter = computed(() => (props.name || '?').slice(0, 1).toUpperCase())

const skinUrl = computed(() => (props.account ? resolveSkin(props.account).url : null))
const head = ref<string | null>(null)
let seq = 0
watch(
  skinUrl,
  async (u) => {
    const my = ++seq
    if (!u) {
      head.value = null
      return
    }
    try {
      const h = await headUrl(u)
      if (my === seq) head.value = h
    } catch {
      if (my === seq) head.value = null
    }
  },
  { immediate: true },
)
</script>

<template>
  <span v-if="head" class="avatar head" :style="box">
    <img :key="head" :src="head" alt="" draggable="false" />
  </span>
  <span v-else class="avatar letter" :style="style">{{ letter }}</span>
</template>

<style scoped>
.avatar {
  position: relative;
  display: grid;
  place-items: center;
  flex: none;
  border-radius: 30%;
  color: #fff;
  font-weight: 700;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.5), 0 3px 10px -3px rgba(0, 0, 0, 0.35);
}
.avatar.head {
  border-radius: 24%;
  overflow: hidden;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  box-shadow: 0 3px 10px -3px rgba(0, 0, 0, 0.4);
}
.avatar.head img {
  width: 100%;
  height: 100%;
  display: block;
  image-rendering: pixelated;
  animation: av-in 0.3s var(--ease-out) both;
}
/* 玻璃描边与顶部高光盖在像素头像上 */
.avatar.head::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
  pointer-events: none;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.22), inset 0 1px 0 rgba(255, 255, 255, 0.4);
  background: linear-gradient(160deg, rgba(255, 255, 255, 0.16), transparent 45%);
}
@keyframes av-in {
  from {
    opacity: 0;
    transform: scale(0.85);
  }
}
</style>
