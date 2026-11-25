<script setup lang="ts">
// 下载任务的玻璃卡片
import { computed } from 'vue'
import Icon from './Icon.vue'
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
        <span v-if="task.status === 'running'" class="spinner" />
        <Icon v-else :name="task.status === 'success' ? 'check' : task.status === 'failed' ? 'alert' : 'close'" :size="16" />
      </span>
      <div class="titles">
        <div class="title ellipsis">{{ task.title }}</div>
        <div class="stage muted ellipsis">{{ task.stage }}</div>
      </div>
      <span class="badge" :class="badge">{{ statusText }}</span>
      <button v-if="task.status === 'running'" class="btn ghost sm icon" title="取消" @click="cancel">
        <Icon name="close" :size="16" />
      </button>
    </div>
    <div class="progress" :class="{ indeterminate }">
      <i :style="{ width: `${task.status === 'success' ? 100 : pct}%` }" />
    </div>
    <div class="meta faint">
      <span v-if="task.filesTotal > 0">{{ task.filesDone }} / {{ task.filesTotal }} 个文件</span>
      <span v-if="task.bytesTotal > 0">{{ formatBytes(task.bytesDone) }} / {{ formatBytes(task.bytesTotal) }}</span>
      <span class="spacer" />
      <span v-if="task.status === 'running' && task.speed > 0">{{ formatBytes(task.speed) }}/s</span>
      <span v-if="task.status === 'running'">{{ pct }}%</span>
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
  width: 34px;
  height: 34px;
  flex: none;
  border-radius: 12px;
  display: grid;
  place-items: center;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
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
