<script setup lang="ts">
// 游戏日志抽屉：实时显示 stdout / stderr，自动滚动到底部
import { computed, nextTick, ref, watch } from 'vue'
import Icon from './Icon.vue'
import { api } from '../api'
import { store, gameState, killInstance, toast } from '../store'

const box = ref<HTMLElement | null>(null)
const follow = ref(true)
const filter = ref('')

const lines = computed(() => {
  const all = store.logInstance ? store.logs[store.logInstance] ?? [] : []
  const f = filter.value.trim().toLowerCase()
  const list = f ? all.filter((l) => l.toLowerCase().includes(f)) : all
  return list.slice(-2000)
})
const state = computed(() => gameState(store.logInstance))

function level(line: string): string {
  if (/\b(ERROR|FATAL|SEVERE)\b|Exception|^\s+at /.test(line)) return 'err'
  if (/\bWARN(ING)?\b/.test(line)) return 'warn'
  if (line.startsWith('[PilotLauncher]')) return 'sys'
  return ''
}

watch(
  () => lines.value.length,
  async () => {
    if (!follow.value) return
    await nextTick()
    if (box.value) box.value.scrollTop = box.value.scrollHeight
  },
)

function onScroll() {
  const el = box.value
  if (!el) return
  follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40
}

async function copyAll() {
  const all = store.logInstance ? store.logs[store.logInstance] ?? [] : []
  try {
    await navigator.clipboard.writeText(all.join('\n'))
    toast('日志已复制到剪贴板', 'success')
  } catch {
    toast('复制失败', 'error')
  }
}

function openDir() {
  if (store.logInstance) api.openInstanceDir(store.logInstance, 'logs').catch(() => {})
}
</script>

<template>
  <Teleport to="body">
    <Transition name="drawer">
      <aside v-if="store.logInstance" class="drawer glass glass-strong" v-light>
        <header class="head">
          <Icon name="log" />
          <div class="titles">
            <div class="title ellipsis">{{ store.logInstance }} · 游戏日志</div>
            <div class="sub muted">{{ state?.message ?? '未运行' }}</div>
          </div>
          <button
            v-if="state?.state === 'running'"
            class="btn danger sm"
            @click="store.logInstance && killInstance(store.logInstance)"
          >
            <Icon name="stop" :size="14" /> 结束进程
          </button>
          <button class="btn ghost sm icon" title="关闭" @click="store.logInstance = null">
            <Icon name="close" :size="16" />
          </button>
        </header>
        <div class="tools">
          <input v-model="filter" class="input" placeholder="筛选日志…" />
          <button class="btn sm" @click="copyAll"><Icon name="copy" :size="15" /> 复制</button>
          <button class="btn sm" @click="openDir"><Icon name="folder" :size="15" /> logs</button>
        </div>
        <div ref="box" class="log mono" @scroll="onScroll">
          <div v-if="lines.length === 0" class="empty">暂无日志</div>
          <div v-for="(l, i) in lines" :key="i" class="line" :class="level(l)">{{ l }}</div>
        </div>
        <button v-if="!follow" class="jump btn sm primary" @click="follow = true; box && (box.scrollTop = box.scrollHeight)">
          回到底部
        </button>
      </aside>
    </Transition>
  </Teleport>
</template>

<style scoped>
.drawer {
  --radius: 24px;
  --blur: 30px;
  position: fixed;
  top: 52px;
  right: 12px;
  bottom: 12px;
  width: min(720px, calc(100vw - 120px));
  z-index: 150;
  display: flex;
  flex-direction: column;
  padding: 14px;
  gap: 10px;
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.titles {
  flex: 1;
  min-width: 0;
}
.title {
  font-weight: 650;
}
.sub {
  font-size: 12px;
}
.tools {
  display: flex;
  gap: 8px;
}
.tools .input {
  height: 30px;
}
.log {
  flex: 1;
  overflow: auto;
  padding: 10px 12px;
  border-radius: 14px;
  font-size: 12px;
  line-height: 1.55;
  background: rgba(10, 12, 18, 0.78);
  color: #d8dde8;
  user-select: text;
}
.line {
  white-space: pre-wrap;
  word-break: break-all;
}
.line.err {
  color: #ff7b72;
}
.line.warn {
  color: #f2cc60;
}
.line.sys {
  color: #79c0ff;
}
.jump {
  position: absolute;
  right: 30px;
  bottom: 28px;
}
.drawer-enter-active {
  transition: transform 0.6s var(--spring), opacity 0.25s ease;
}
.drawer-leave-active {
  transition: transform 0.22s ease, opacity 0.22s ease;
}
.drawer-enter-from,
.drawer-leave-to {
  transform: translateX(40px) scale(0.97);
  opacity: 0;
}
</style>
