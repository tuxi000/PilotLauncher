<script setup lang="ts">
// 自绘标题栏（窗口 decorations: false）
import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Icon from './Icon.vue'
import { store, runningTaskCount } from '../store'

const win = getCurrentWindow()
const maximized = ref(false)
let unlisten: (() => void) | null = null

async function syncMax() {
  try {
    maximized.value = await win.isMaximized()
  } catch {
    /* ignore */
  }
}

onMounted(async () => {
  await syncMax()
  try {
    unlisten = await win.onResized(() => syncMax())
  } catch {
    /* ignore */
  }
})
onUnmounted(() => unlisten?.())

const minimize = () => win.minimize()
const toggleMax = () => win.toggleMaximize()
const close = () => win.close()

const titles: Record<string, string> = {
  home: '首页',
  download: '下载',
  instances: '实例',
  multiplayer: '联机',
  settings: '设置',
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="logo" data-tauri-drag-region />
      <span class="name" data-tauri-drag-region>PilotLauncher</span>
      <span class="sep" data-tauri-drag-region>·</span>
      <span class="page" data-tauri-drag-region>{{ titles[store.page] }}</span>
    </div>
    <div class="drag" data-tauri-drag-region />
    <button
      v-if="runningTaskCount > 0"
      class="tasks-pill"
      title="查看下载任务"
      @click="store.page = 'download'; store.downloadTab = 'tasks'"
    >
      <span class="spinner" />
      {{ runningTaskCount }} 个任务进行中
    </button>
    <div class="controls">
      <button class="ctl" title="最小化" @click="minimize"><Icon name="minimize" :size="16" /></button>
      <button class="ctl" :title="maximized ? '还原' : '最大化'" @click="toggleMax">
        <svg v-if="maximized" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M8 8h11v11H8zM5 16V5h11" />
        </svg>
        <Icon v-else name="maximize" :size="14" />
      </button>
      <button class="ctl close" title="关闭" @click="close"><Icon name="close" :size="16" /></button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  position: relative;
  z-index: 50;
  height: 40px;
  display: flex;
  align-items: center;
  padding-left: 16px;
  flex: none;
}
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
.logo {
  width: 18px;
  height: 18px;
  border-radius: 6px;
  background: linear-gradient(135deg, #36a3ff, #6a5cff);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6), 0 2px 6px rgba(70, 100, 255, 0.4);
}
.name {
  font-weight: 650;
  letter-spacing: 0.01em;
}
.sep,
.page {
  color: var(--text-2);
}
.drag {
  flex: 1;
  height: 100%;
}
.tasks-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  padding: 0 12px;
  margin-right: 10px;
  border: none;
  border-radius: 13px;
  font-size: 12px;
  cursor: pointer;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  transition: transform 0.4s var(--spring);
}
.tasks-pill:hover {
  transform: scale(1.05);
}
.tasks-pill .spinner {
  width: 12px;
  height: 12px;
  border-width: 2px;
}
.controls {
  display: flex;
  height: 100%;
}
.ctl {
  width: 46px;
  height: 100%;
  border: none;
  background: transparent;
  display: grid;
  place-items: center;
  cursor: pointer;
  color: var(--text);
  transition: background 0.15s ease;
}
.ctl:hover {
  background: color-mix(in srgb, var(--text) 9%, transparent);
}
.ctl.close:hover {
  background: #e81123;
  color: #fff;
}
</style>
