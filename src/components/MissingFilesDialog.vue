<script setup lang="ts">
// CurseForge 整合包中不允许第三方下载的文件：列出来，提示手动下载
import GlassDialog from './GlassDialog.vue'
import Icon from './Icon.vue'
import { api } from '../api'
import { store, toastError } from '../store'

function openDir(sub: string) {
  if (store.missing) api.openInstanceDir(store.missing.instance, sub).catch(toastError)
}
</script>

<template>
  <GlassDialog :open="!!store.missing" title="有文件需要手动下载" :width="600" @close="store.missing = null">
    <p class="muted small">
      整合包 <b>{{ store.missing?.instance }}</b> 中有 {{ store.missing?.files.length }} 个文件的作者不允许第三方启动器下载。
      请在浏览器中打开下载页手动下载，然后放进实例对应的文件夹。之后也可以在实例的“资源管理”中再次查看这个列表。
    </p>
    <div class="list">
      <div v-for="f in store.missing?.files ?? []" :key="f.url + f.fileName" class="mf glass-lite">
        <div class="info">
          <div class="ellipsis name">{{ f.name }}</div>
          <div class="faint ellipsis small">{{ f.fileName }} → {{ f.target }}/</div>
        </div>
        <button class="btn sm" @click="api.openUrl(f.url)"><Icon name="external" :size="14" /> 下载页</button>
        <button class="btn sm icon ghost" title="打开目标文件夹" @click="openDir(f.target)"><Icon name="folder" :size="15" /></button>
      </div>
    </div>
    <template #footer>
      <button class="btn primary" @click="store.missing = null">知道了</button>
    </template>
  </GlassDialog>
</template>

<style scoped>
.small {
  font-size: 12.5px;
  line-height: 1.7;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 50vh;
  overflow: auto;
  margin-top: 8px;
}
.mf {
  --radius: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
}
.info {
  flex: 1;
  min-width: 0;
}
.name {
  font-weight: 600;
}
</style>
