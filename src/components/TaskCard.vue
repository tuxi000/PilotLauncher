<script setup lang="ts">
// 下载任务的玻璃卡片：进行中的进度条带流光，速度和百分比平滑滚动；完成时勾号弹出并扩散一圈
import { computed } from 'vue'
import Icon from './Icon.vue'
import AnimatedNumber from './AnimatedNumber.vue'
import { api, formatBytes, type TaskSnapshot } from '../api'
import { toastError } from '../store'

const props = defineProps<{ task: TaskSnapshot }>()

const pct = computed(() => Math.round(props.task.progress * 100))
const statusText = computed(() => {
  switch (props.task.status) {
    case 'success':
      return '已完成'
    case 'failed':
      return '失败'
    case 'cancelled':
      return '已取消'
    default:
      return props.task.stageCount > 0 ? `阶段 ${props.task.stageIndex}/${props.task.stageCount}` : '进行中'
  }
})
const badge = computed(
  () => ({ success: 'success', failed: 'danger', cancelled: 'warning', running: 'accent' })[props.task.status],
)
const indeterminate = computed(
  () => props.task.status === 'running' && props.task.bytesTotal === 0 && props.task.filesTotal <= 1,
)
/** 刚刚完成（3 秒内）：播放完成动画；切换标签后重新显示的旧任务不再播放 */
const fresh = computed(() => !!props.task.finishedAt && Date.now() - props.task.finishedAt < 3000)

const speedText = (v: number) => `${formatBytes(v)}/s`
const pctText = (v: number) => `${Math.round(v)}%`

async function cancel() {
  try {
    await api.cancelTask(props.task.id)
  } catch (e) {
    toastError(e)
  }
}
</script>

<template>
  <div class="task glass" :class="task.status" v-light>
    <div class="top">
      <span class="ring" :class="task.status">
        <Transition name="ring-swap" mode="out-in">
          <span v-if="task.status === 'running'" key="run" class="spinner" />
          <span v-else :key="task.status" class="ring-ico">
            <Icon :name="task.status === 'success' ? 'check' : task.status === 'failed' ? 'alert' : 'close'" :size="16" :stroke="2.2" />
          </span>
        </Transition>
        <span v-if="fresh && task.status === 'success'" class="ring-burst" />
      </span>
      <div class="titles">
        <div class="title ellipsis">{{ task.title }}</div>
        <Transition name="swap" mode="out-in">
          <div :key="task.stage" class="stage muted ellipsis">{{ task.stage }}</div>
        </Transition>
      </div>
      <span class="badge" :class="badge">{{ statusText }}</span>
      <button v-if="task.status === 'running'" class="btn ghost sm icon" title="取消" @click="cancel">
        <Icon name="close" :size="16" />
      </button>
    </div>
    <div class="progress" :class="{ indeterminate, flowing: task.status === 'running' && !indeterminate, done: task.status === 'success' }">
      <i :style="{ width: `${task.status === 'success' ? 100 : pct}%` }" />
    </div>
    <div class="meta faint">
      <span v-if="task.filesTotal > 0">{{ task.filesDone }} / {{ task.filesTotal }} 个文件</span>
      <span v-if="task.bytesTotal > 0">{{ formatBytes(task.bytesDone) }} / {{ formatBytes(task.bytesTotal) }}</span>
      <span class="spacer" />
      <span v-if="task.status === 'running' && task.speed > 0"><AnimatedNumber :value="task.speed" :format="speedText" /></span>
      <span v-if="task.status === 'running'"><AnimatedNumber :value="pct" :format="pctText" :duration="350" /></span>
    </div>
    <pre v-if="task.error" class="error">{{ task.error }}</pre>
  </div>
</template>

<style scoped>
.task {
  --radius: 20px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: transform 0.5s var(--spring);
}
.task:hover {
  transform: translateY(-2px);
}
.top {
  display: flex;
  align-items: center;
  gap: 12px;
}
.ring {
  position: relative;
  width: 34px;
  height: 34px;
  flex: none;
  border-radius: 12px;
  display: grid;
  place-items: center;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  transition: background 0.3s ease, color 0.3s ease;
}
.ring.success {
  color: var(--success);
  background: color-mix(in srgb, var(--success) 16%, transparent);
}
.ring.failed {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
}
.ring.cancelled {
  color: var(--warning);
  background: color-mix(in srgb, var(--warning) 16%, transparent);
}
.ring-ico {
  display: grid;
  place-items: center;
}
/* 完成时扩散一圈 */
.ring-burst {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  pointer-events: none;
  box-shadow: 0 0 0 2px var(--success);
  opacity: 0;
  animation: ring-burst 0.8s var(--ease-out) both;
}
@keyframes ring-burst {
  0% {
    opacity: 0.8;
    transform: scale(1);
  }
  100% {
    opacity: 0;
    transform: scale(1.7);
  }
}
.ring-swap-enter-active {
  transition: transform 0.55s var(--spring), opacity 0.18s ease;
}
.ring-swap-leave-active {
  transition: transform 0.14s ease, opacity 0.14s ease;
}
.ring-swap-enter-from {
  opacity: 0;
  transform: scale(0.3) rotate(-35deg);
}
.ring-swap-leave-to {
  opacity: 0;
  transform: scale(0.5);
}
.titles {
  flex: 1;
  min-width: 0;
}
.title {
  font-weight: 600;
}
.stage {
  font-size: 12.5px;
  margin-top: 2px;
}
.progress.done > i {
  background: linear-gradient(90deg, color-mix(in srgb, var(--success) 70%, var(--accent)), var(--success));
  box-shadow: 0 0 12px color-mix(in srgb, var(--success) 55%, transparent), inset 0 1px 0 rgba(255, 255, 255, 0.5);
}
.meta {
  display: flex;
  gap: 14px;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.error {
  margin: 0;
  padding: 10px 12px;
  border-radius: 12px;
  max-height: 140px;
  overflow: auto;
  font-family: var(--mono);
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 8%, transparent);
  user-select: text;
}
</style>
