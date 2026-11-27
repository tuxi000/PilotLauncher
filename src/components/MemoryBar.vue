<script setup lang="ts">
// 内存条：像 PCL 一样实时显示整台电脑的内存占用，并标出分配给游戏的部分（每秒刷新）
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref } from 'vue'
import AnimatedNumber from './AnimatedNumber.vue'
import { api, type MemoryStatus } from '../api'

const props = defineProps<{ allocatedMb: number }>()
const emit = defineEmits<{ (e: 'status', s: MemoryStatus): void }>()

const status = ref<MemoryStatus | null>(null)
let timer: number | undefined

async function poll() {
  try {
    status.value = await api.memoryStatus()
    emit('status', status.value)
  } catch {
    /* 忽略单次失败 */
  }
}
function start() {
  stop()
  poll()
  timer = window.setInterval(poll, 1000)
}
function stop() {
  if (timer !== undefined) window.clearInterval(timer)
  timer = undefined
}
// 页面被 KeepAlive 缓存时停止轮询，切回来再继续
onMounted(start)
onActivated(start)
onDeactivated(stop)
onBeforeUnmount(stop)

const gb = (mb: number) => (mb / 1024).toFixed(1)
const gbText = (mb: number) => `${gb(mb)} GB`
const total = computed(() => Math.max(1, status.value?.totalMb ?? 1))
const used = computed(() => Math.max(0, total.value - (status.value?.availableMb ?? 0)))
const avail = computed(() => status.value?.availableMb ?? 0)
const game = computed(() => Math.max(0, props.allocatedMb))
/** 游戏实际能放进剩余内存的部分；超出部分标红 */
const gameFit = computed(() => Math.min(game.value, avail.value))
const over = computed(() => Math.max(0, game.value - avail.value))
const freeAfter = computed(() => Math.max(0, avail.value - game.value))
const pct = (mb: number) => `${Math.min(100, (mb / total.value) * 100).toFixed(2)}%`
</script>

<template>
  <div class="membar" :class="{ warn: over > 0 }">
    <div class="track">
      <span class="seg used" :style="{ width: pct(used) }" />
      <span class="seg game" :style="{ width: pct(gameFit) }" />
      <span v-if="over > 0" class="seg over" :style="{ width: pct(Math.min(over, used)), marginLeft: `-${pct(Math.min(over, used))}` }" />
    </div>
    <div class="legend">
      <span><i class="dot used" />已使用 <b class="mono"><AnimatedNumber :value="used" :format="gbText" :duration="600" /></b></span>
      <span><i class="dot game" />游戏分配 <b class="mono"><AnimatedNumber :value="game" :format="gbText" /></b></span>
      <span><i class="dot free" />剩余 <b class="mono"><AnimatedNumber :value="freeAfter" :format="gbText" :duration="600" /></b></span>
      <span class="spacer" />
      <span class="faint">总内存 <span class="mono">{{ gb(total) }} GB</span></span>
    </div>
    <div v-if="over > 0" class="tip">
      分配给游戏的内存比当前可用内存多 {{ gb(over) }} GB，游戏可能会用到虚拟内存而卡顿。可关闭其他程序或调低内存。
    </div>
  </div>
</template>

<style scoped>
.membar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}
.track {
  position: relative;
  display: flex;
  height: 12px;
  border-radius: 999px;
  overflow: hidden;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.12), inset 0 0 0 0.5px var(--glass-edge);
}
.seg {
  height: 100%;
  flex: none;
  transition: width 0.7s var(--spring), margin-left 0.7s var(--spring);
}
.seg.used {
  background: linear-gradient(180deg, color-mix(in srgb, var(--text) 42%, transparent), color-mix(in srgb, var(--text) 30%, transparent));
}
.seg.game {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 80%, white), var(--accent));
  box-shadow: 0 0 10px -2px color-mix(in srgb, var(--accent) 70%, transparent);
}
.seg.over {
  position: relative;
  z-index: 1;
  background: repeating-linear-gradient(-45deg, var(--danger) 0 5px, color-mix(in srgb, var(--danger) 60%, transparent) 5px 10px);
}
.legend {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px 16px;
  font-size: 12px;
  color: var(--text-2);
}
.legend b {
  color: var(--text);
  font-weight: 600;
}
.spacer {
  flex: 1;
}
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 6px;
  border-radius: 50%;
  vertical-align: 0;
}
.dot.used {
  background: color-mix(in srgb, var(--text) 38%, transparent);
}
.dot.game {
  background: var(--accent);
}
.dot.free {
  background: color-mix(in srgb, var(--text) 10%, transparent);
  box-shadow: inset 0 0 0 1px var(--glass-edge);
}
.tip {
  font-size: 12px;
  color: var(--danger);
}
</style>
