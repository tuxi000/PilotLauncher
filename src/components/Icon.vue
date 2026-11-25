<script setup lang="ts">
// 线性图标（24×24，描边风格）
import { computed } from 'vue'

const props = withDefaults(defineProps<{ name: string; size?: number; stroke?: number }>(), {
  size: 20,
  stroke: 1.8,
})

const PATHS: Record<string, string> = {
  home: 'M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z',
  download: 'M12 3v12m0 0-4.5-4.5M12 15l4.5-4.5M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2',
  cube: 'M12 2.8 20.5 7.5v9L12 21.2 3.5 16.5v-9zM3.5 7.5 12 12.2l8.5-4.7M12 12.2v9',
  settings:
    'M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4zM19.4 13.5a7.6 7.6 0 0 0 0-3l2-1.6-2-3.4-2.4.9a7.6 7.6 0 0 0-2.6-1.5L14 2.4h-4l-.4 2.5A7.6 7.6 0 0 0 7 6.4l-2.4-.9-2 3.4 2 1.6a7.6 7.6 0 0 0 0 3l-2 1.6 2 3.4 2.4-.9a7.6 7.6 0 0 0 2.6 1.5l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.6-1.5l2.4.9 2-3.4z',
  play: 'M7 4.5v15a.8.8 0 0 0 1.2.7l12-7.5a.8.8 0 0 0 0-1.4l-12-7.5A.8.8 0 0 0 7 4.5z',
  stop: 'M6 6h12v12H6z',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  edit: 'M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16zM13.5 6.5l4 4',
  trash: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6',
  wrench:
    'M14.7 6.3a4 4 0 0 0-5.4 5.4L3.6 17.4a1.4 1.4 0 0 0 2 2l5.7-5.7a4 4 0 0 0 5.4-5.4l-2.6 2.6-2-2z',
  plus: 'M12 5v14M5 12h14',
  user: 'M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 20.5c1.4-3.5 4.4-5 8-5s6.6 1.5 8 5',
  close: 'M6 6l12 12M18 6 6 18',
  minimize: 'M5 12h14',
  maximize: 'M6 6h12v12H6z',
  check: 'M4.5 12.5 9.5 17.5 19.5 7',
  refresh: 'M20 12a8 8 0 1 1-2.3-5.7M20 4v4.5h-4.5',
  search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4',
  log: 'M5 4h10l4 4v12H5zM14 4v5h5M8 13h8M8 17h6',
  chevron: 'M9 6l6 6-6 6',
  chevronDown: 'M6 9l6 6 6-6',
  globe: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM3 12h18M12 3c2.5 2.6 3.7 5.6 3.7 9s-1.2 6.4-3.7 9c-2.5-2.6-3.7-5.6-3.7-9S9.5 5.6 12 3z',
  key: 'M14.5 9.5a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0zM13.2 12.7 20 19.5M17 16.5l2-2M18.5 18l1.5-1.5',
  java: 'M8 18c2 1 6 1 8 0M7 15.5c2.5 1 7.5 1 10 0M10 3c3 2.5-2 4 1 6.5M14 5c2 2-1.5 3 .5 5M6 21h12',
  sparkle: 'M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8z',
  info: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v5M12 7.5v.5',
  alert: 'M12 3.5 21.5 20h-19zM12 10v4.5M12 17.5v.5',
  copy: 'M9 9h10v11H9zM5 15V4h10',
  logout: 'M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 8l-4 4 4 4M6 12h10',
  palette:
    'M12 21a9 9 0 1 1 9-9c0 2-1.6 3-3.2 3H16a2 2 0 0 0-1.4 3.4c.6.6.2 2.6-2.6 2.6zM7.5 11.5h.01M10 7.5h.01M14.5 7.5h.01M17 11h.01',
  memory: 'M5 7h14v10H5zM8 7V4M12 7V4M16 7V4M8 20v-3M12 20v-3M16 20v-3M9 10h6v4H9z',
  puzzle:
    'M9.5 4.5a2 2 0 1 1 4 0V6H18v4.5h-1.5a2 2 0 1 0 0 4H18V19h-4.5v-1.5a2 2 0 1 0-4 0V19H5v-4.5h1.5a2 2 0 1 0 0-4H5V6h4.5z',
  image: 'M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M15.5 9.5h.01',
  sun: 'M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM12 2.5v2M12 19.5v2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M2.5 12h2M19.5 12h2M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4',
  map: 'M9 4 3.5 6.5v13.5L9 17.5l6 2.5 5.5-2.5V4L15 6.5zM9 4v13.5M15 6.5V20',
  package: 'M12 3l8.5 4.5v9L12 21l-8.5-4.5v-9zM3.5 7.5 12 12l8.5-4.5M12 12v9M7.8 5.3l8.4 4.5',
  code: 'M8 8l-4 4 4 4M16 8l4 4-4 4M14 5l-4 14',
  upload: 'M12 16V4m0 0L7.5 8.5M12 4l4.5 4.5M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4',
  external: 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5',
  heart: 'M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z',
  layers: 'M12 3 21 8l-9 5-9-5zM3 12.5l9 5 9-5M3 16.5l9 5 9-5',
  filter: 'M4 5h16l-6 7.5V19l-4 1.5v-8z',
  /** 联机：三个相连的节点 */
  network:
    'M3.5 6.5a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0zM15.5 6.5a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0zM9.5 17.5a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0zM8.5 6.5h7M7.2 8.7l3.6 6.6M16.8 8.7l-3.6 6.6',
  users:
    'M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2.5 20c.9-3.3 3.4-5 6.5-5s5.6 1.7 6.5 5M16 4.3a3.5 3.5 0 0 1 0 6.4M18 15.2c1.7.7 2.9 2.4 3.5 4.8',
  link: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1',
}

const d = computed(() => PATHS[props.name] ?? PATHS.info)
</script>

<template>
  <svg
    class="icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="stroke"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="d" />
  </svg>
</template>

<style scoped>
.icon {
  flex: none;
  display: block;
}
</style>
