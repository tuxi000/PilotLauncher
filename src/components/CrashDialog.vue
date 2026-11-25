<script setup lang="ts">
// 崩溃提示
import { computed } from 'vue'
import GlassDialog from './GlassDialog.vue'
import Icon from './Icon.vue'
import { api } from '../api'
import { store, openLog } from '../store'

const info = computed(() => store.crash?.info ?? null)

function viewLog() {
  if (!store.crash) return
  const name = store.crash.instance
  store.crash = null
  openLog(name)
}

function openReport() {
  if (info.value?.reportPath) api.openPath(info.value.reportPath).catch(() => {})
}
</script>

<template>
  <GlassDialog :open="!!store.crash" :title="info?.title ?? '游戏崩溃'" :width="640" @close="store.crash = null">
    <div v-if="info" class="crash">
      <div class="hero">
        <span class="ico"><Icon name="alert" :size="26" /></span>
        <div>
          <div class="strong">{{ store.crash?.instance }} 异常退出</div>
          <div class="muted">下面是根据日志和崩溃报告分析出的可能原因：</div>
        </div>
      </div>
      <ul class="tips">
        <li v-for="(s, i) in info.suggestions" :key="i">{{ s }}</li>
      </ul>
      <details v-if="info.reportExcerpt" class="block">
        <summary>崩溃报告摘要</summary>
        <pre class="mono">{{ info.reportExcerpt }}</pre>
      </details>
      <details class="block" open>
        <summary>最后的日志</summary>
        <pre class="mono">{{ info.logTail.join('\n') }}</pre>
      </details>
    </div>
    <template #footer>
      <button v-if="info?.reportPath" class="btn" @click="openReport"><Icon name="folder" :size="16" /> 打开崩溃报告</button>
      <button class="btn" @click="viewLog"><Icon name="log" :size="16" /> 查看完整日志</button>
      <button class="btn primary" @click="store.crash = null">知道了</button>
    </template>
  </GlassDialog>
</template>

<style scoped>
.crash {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.hero {
  display: flex;
  gap: 14px;
  align-items: center;
}
.ico {
  width: 48px;
  height: 48px;
  border-radius: 16px;
  display: grid;
  place-items: center;
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
}
.strong {
  font-weight: 650;
  font-size: 15px;
}
.tips {
  margin: 0;
  padding-left: 20px;
  line-height: 1.7;
}
.block summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-2);
  margin-bottom: 6px;
}
.block pre {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  padding: 10px 12px;
  border-radius: 12px;
  font-size: 11.5px;
  white-space: pre-wrap;
  word-break: break-all;
  background: rgba(10, 12, 18, 0.78);
  color: #d8dde8;
  user-select: text;
}
</style>
