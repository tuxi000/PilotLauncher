<script setup lang="ts">
// 实例 / 版本 / 加载器图标（见 instanceIcons.ts）：原版风格方块图片，或原创像素画。
//  - icon 为空、"auto" 或旧默认值 "grass" 时按“自动”处理：有加载器显示加载器图标，否则按版本类型
//  - 换成方块图片后不再需要玻璃底座：图标直接放在卡片上，只加一层很轻的投影；大于 48px 时投影稍重一点
//  - 图片原图只有 30～40px，放大时用浏览器默认插值（不用 pixelated），边缘柔和、不会出现锯齿块
//  - plain：图标铺满整个尺寸（例如加载器卡片标题里的小图标），否则四周留一点空白，与文字的视觉重量更协调
import { computed } from 'vue'
import { iconUrl, resolveIcon } from '../instanceIcons'

const props = withDefaults(
  defineProps<{
    name?: string
    icon?: string | null
    loader?: string | null
    versionType?: string | null
    size?: number
    plain?: boolean
  }>(),
  { name: '', icon: 'auto', loader: null, versionType: 'release', size: 48, plain: false },
)

const key = computed(() => resolveIcon(props.icon, props.loader, props.versionType))
const src = computed(() => iconUrl(key.value))
const s = computed(() => `${props.size}px`)
const inner = computed(() => Math.round(props.size * (props.plain ? 1 : 0.9)))
const large = computed(() => props.size > 48)
</script>

<template>
  <span class="inst-icon" :class="{ large }" :style="{ width: s, height: s }">
    <img :src="src" :width="inner" :height="inner" alt="" draggable="false" decoding="async" />
  </span>
</template>

<style scoped>
.inst-icon {
  flex: none;
  display: grid;
  place-items: center;
}
.inst-icon img {
  display: block;
  pointer-events: none;
  object-fit: contain;
  filter: drop-shadow(0 1px 1.5px rgba(10, 20, 40, 0.18));
}
.inst-icon.large img {
  filter: drop-shadow(0 4px 8px rgba(10, 20, 40, 0.26)) drop-shadow(0 1px 2px rgba(10, 20, 40, 0.16));
}
html[data-theme='dark'] .inst-icon img {
  filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.4));
}
html[data-theme='dark'] .inst-icon.large img {
  filter: drop-shadow(0 5px 10px rgba(0, 0, 0, 0.45)) drop-shadow(0 1px 2px rgba(0, 0, 0, 0.3));
}
</style>
