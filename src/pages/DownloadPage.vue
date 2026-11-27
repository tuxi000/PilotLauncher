<script setup lang="ts">
// 下载页：游戏版本（含加载器）、社区资源（模组 / 整合包 / 资源包 / 光影包 / 数据包 / 地图）、下载任务
import { computed, onMounted, ref } from 'vue'
import Icon from '../components/Icon.vue'
import Segmented from '../components/Segmented.vue'
import GlassDialog from '../components/GlassDialog.vue'
import TaskCard from '../components/TaskCard.vue'
import LoaderPicker from '../components/LoaderPicker.vue'
import InstanceIcon from '../components/InstanceIcon.vue'
import CommunityBrowser from '../components/CommunityBrowser.vue'
import {
  api,
  errMsg,
  formatDate,
  uniqueInstanceName,
  LOADER_LABEL,
  VERSION_TYPE_LABEL,
  type InstallItem,
  type LoaderSpec,
  type ManifestVersion,
  type ResourceKind,
  type VersionManifest,
} from '../api'
import { store, taskList, toast, toastError, refreshInstances, type DownloadTab } from '../store'
import { versionIcon } from '../instanceIcons'
import { prefetchLoaders } from '../loaderVersions'
import { APRIL_FOOLS_NAMES, APRIL_FOOLS_OFFLINE, isAprilFools, type OfflineAprilFools } from '../aprilFools'

type Filter = 'release' | 'snapshot' | 'old' | 'april'
const filter = ref<Filter>('release')
const keyword = ref('')
const manifest = ref<VersionManifest | null>(null)
const loading = ref(false)
const loadError = ref('')

/** 版本分类：愚人节版本单独一类，不再出现在“快照”里 */
function categoryOf(v: ManifestVersion): Filter | null {
  if (isAprilFools(v)) return 'april'
  if (v.type === 'release') return 'release'
  if (v.type === 'snapshot') return 'snapshot'
  if (v.type === 'old_alpha' || v.type === 'old_beta') return 'old'
  return null
}

/** 图标与标签用的类型：愚人节版本为 april_fools（与后端 InstanceView.versionType 一致） */
function typeOf(v: ManifestVersion): string {
  return isAprilFools(v) ? 'april_fools' : v.type
}

function subtitle(v: ManifestVersion): string {
  const t = typeOf(v)
  const name = t === 'april_fools' ? APRIL_FOOLS_NAMES[v.id] : undefined
  return [VERSION_TYPE_LABEL[t] ?? v.type, name, `发布于 ${formatDate(v.releaseTime)}`].filter(Boolean).join(' · ')
}

async function load(force = false) {
  loading.value = true
  loadError.value = ''
  try {
    manifest.value = await api.manifest(force)
  } catch (e) {
    loadError.value = errMsg(e)
  } finally {
    loading.value = false
  }
}
onMounted(() => load(false))

const classified = computed(() => (manifest.value?.versions ?? []).map((v) => ({ v, cat: categoryOf(v) })))

const versions = computed(() => {
  const k = keyword.value.trim().toLowerCase()
  return classified.value
    .filter((x) => x.cat === filter.value && (!k || x.v.id.toLowerCase().includes(k)))
    .map((x) => x.v)
})

/** 官方清单里没有的早期愚人节版本（例如 2013 年的 2.0）：放在愚人节分类末尾，灰色、不能安装 */
const offline = computed<OfflineAprilFools[]>(() => {
  if (filter.value !== 'april' || !manifest.value) return []
  const k = keyword.value.trim().toLowerCase()
  const known = new Set(manifest.value.versions.map((v) => v.id))
  return APRIL_FOOLS_OFFLINE.filter(
    (o) => !known.has(o.id) && (!k || o.id.toLowerCase().includes(k) || o.name.toLowerCase().includes(k)),
  )
})

const counts = computed(() => {
  const c: Record<Filter, number> = { release: 0, snapshot: 0, old: 0, april: 0 }
  for (const x of classified.value) if (x.cat) c[x.cat]++
  return c
})

function openWiki(o: OfflineAprilFools) {
  api.openUrl(o.wiki).catch(toastError)
}

// 安装对话框
const target = ref<ManifestVersion | null>(null)
/** 安装对话框中版本的类型（愚人节版本为 april_fools，LoaderPicker 据此禁用全部加载器） */
const targetType = computed(() => (target.value ? typeOf(target.value) : null))
const instName = ref('')
const installing = ref(false)
const loaderSpecs = ref<LoaderSpec[]>([])
const addons = ref<InstallItem[]>([])
const nameTouched = ref(false)

function suggestedName(): string {
  if (!target.value) return ''
  const suffix = loaderSpecs.value
    .filter((s) => s.kind !== 'optifine')
    .map((s) => LOADER_LABEL[s.kind])
    .concat(loaderSpecs.value.some((s) => s.kind === 'optifine') ? ['OptiFine'] : [])
    .join('-')
  return uniqueInstanceName(suffix ? `${target.value.id}-${suffix}` : target.value.id, store.instances)
}

function openInstall(v: ManifestVersion) {
  // 弹窗打开前就开始并行预取各加载器的版本列表（愚人节版本不支持加载器，不预取）
  if (!isAprilFools(v)) prefetchLoaders(v.id)
  target.value = v
  loaderSpecs.value = []
  addons.value = []
  nameTouched.value = false
  instName.value = uniqueInstanceName(v.id, store.instances)
}

function onSpecs(v: LoaderSpec[]) {
  loaderSpecs.value = v
  if (!nameTouched.value) instName.value = suggestedName()
}

async function install() {
  if (!target.value) return
  installing.value = true
  try {
    const extra = loaderSpecs.value.length ? addons.value : []
    await api.createInstance(instName.value.trim(), target.value.id, loaderSpecs.value, extra)
    toast(`已开始安装 ${instName.value}`, 'success')
    target.value = null
    store.downloadTab = 'tasks'
    await refreshInstances()
  } catch (e) {
    toastError(e)
  } finally {
    installing.value = false
  }
}

async function clearFinished() {
  try {
    const list = await api.clearFinishedTasks()
    store.tasks = Object.fromEntries(list.map((t) => [t.id, t]))
  } catch (e) {
    toastError(e)
  }
}

const COMMUNITY_TABS: ResourceKind[] = ['mod', 'modpack', 'resourcepack', 'shaderpack', 'datapack', 'world']
const isCommunity = computed(() => COMMUNITY_TABS.includes(store.downloadTab as ResourceKind))

const tabs = computed<{ value: DownloadTab; label: string }[]>(() => [
  { value: 'versions', label: '游戏版本' },
  { value: 'mod', label: '模组' },
  { value: 'modpack', label: '整合包' },
  { value: 'resourcepack', label: '资源包' },
  { value: 'shaderpack', label: '光影包' },
  { value: 'datapack', label: '数据包' },
  { value: 'world', label: '地图' },
  { value: 'tasks', label: `任务${taskList.value.length ? ` ${taskList.value.length}` : ''}` },
])
</script>

<template>
  <section class="page">
    <header class="page-head">
      <h2>下载</h2>
      <Segmented v-model="store.downloadTab" :options="tabs" />
    </header>

    <!-- 版本列表 -->
    <div v-if="store.downloadTab === 'versions'" class="content">
      <div class="toolbar">
        <Segmented
          v-model="filter"
          :options="[
            { value: 'release', label: `正式版 ${counts.release || ''}`, icon: 'grass_block' },
            { value: 'snapshot', label: `快照 ${counts.snapshot || ''}`, icon: 'command_block' },
            { value: 'old', label: `远古版本 ${counts.old || ''}`, icon: 'cobblestone' },
            { value: 'april', label: `愚人节 ${counts.april || ''}`, icon: 'gold_block', title: '每年 4 月 1 日发布的玩笑版本，不支持模组加载器' },
          ]"
        />
        <div class="search">
          <Icon name="search" :size="16" class="faint" />
          <input v-model="keyword" class="input" placeholder="搜索版本号，如 1.20.1" />
        </div>
        <button class="btn icon" title="刷新" :disabled="loading" @click="load(true)">
          <span v-if="loading" class="spinner" />
          <Icon v-else name="refresh" :size="18" />
        </button>
      </div>

      <div v-if="loadError" class="glass card error-card">
        <Icon name="alert" />
        <span>{{ loadError }}</span>
        <button class="btn sm" @click="load(true)">重试</button>
      </div>

      <div class="list stagger">
        <div v-if="loading && !manifest" class="empty"><span class="spinner" />正在获取版本列表…</div>
        <div v-else-if="manifest && versions.length === 0 && offline.length === 0" class="empty">没有匹配的版本</div>
        <div v-for="v in versions" :key="v.id" class="ver glass-lite lift">
          <InstanceIcon :icon="versionIcon(typeOf(v))" :size="40" />
          <div class="ver-info">
            <div class="ver-id">
              {{ v.id }}
              <span v-if="v.id === manifest?.latest.release" class="badge success">最新正式版</span>
              <span v-else-if="v.id === manifest?.latest.snapshot" class="badge warning">最新快照</span>
            </div>
            <div class="faint ver-sub">{{ subtitle(v) }}</div>
          </div>
          <button class="btn sm primary" @click="openInstall(v)"><Icon name="download" :size="15" /> 安装</button>
        </div>
        <!-- 官方未提供下载的早期愚人节版本：灰色条目，点击打开 Minecraft Wiki 介绍页 -->
        <div
          v-for="o in offline"
          :key="`offline-${o.id}`"
          class="ver glass-lite offline"
          role="link"
          tabindex="0"
          :title="`官方未提供下载，点击在浏览器中打开 Minecraft Wiki：${o.wiki}`"
          @click="openWiki(o)"
          @keydown.enter="openWiki(o)"
        >
          <InstanceIcon icon="gold_block" :size="40" />
          <div class="ver-info">
            <div class="ver-id">
              {{ o.id }}
              <span class="badge">官方未提供下载</span>
            </div>
            <div class="faint ver-sub">愚人节版本 · {{ o.name }} · 发布于 {{ formatDate(o.releaseTime) }}</div>
          </div>
          <span class="btn sm ghost wiki"><Icon name="external" :size="15" /> Wiki</span>
        </div>
      </div>
    </div>

    <!-- 任务 -->
    <div v-else-if="store.downloadTab === 'tasks'" class="content">
      <div class="toolbar">
        <span class="muted">安装、补全文件、Java 运行时下载的进度都会显示在这里</span>
        <span class="spacer" />
        <button class="btn sm" @click="clearFinished"><Icon name="trash" :size="15" /> 清除已结束</button>
      </div>
      <div class="list tasks">
        <div v-if="taskList.length === 0" class="empty">
          <Icon name="download" :size="34" />
          暂无任务
        </div>
        <TransitionGroup name="list">
          <TaskCard v-for="(t, idx) in taskList" :key="t.id" :task="t" :style="{ '--i': idx }" />
        </TransitionGroup>
      </div>
    </div>


    <!-- 社区资源：每个标签保留自己的搜索状态（KeepAlive 放在条件块外，切到“游戏版本 / 任务”也不丢失） -->
    <div v-show="isCommunity" class="content">
      <KeepAlive>
        <CommunityBrowser v-if="isCommunity" :key="store.downloadTab" :kind="store.downloadTab as ResourceKind" />
      </KeepAlive>
    </div>

    <GlassDialog :open="!!target" :title="`安装 ${target?.id ?? ''}`" :width="640" @close="target = null">
      <div class="install">
        <div class="field">
          <label>实例名称（同时作为独立游戏目录 versions/实例名）</label>
          <input v-model="instName" class="input" maxlength="64" @input="nameTouched = true" @keydown.enter="install" />
        </div>
        <div class="field">
          <span class="field-label">模组加载器（可选）</span>
          <LoaderPicker
            v-if="target"
            v-model:addons="addons"
            :mc-version="target.id"
            :version-type="targetType"
            :model-value="loaderSpecs"
            @update:model-value="onSpecs"
          />
          <span v-if="targetType !== 'april_fools'" class="faint hint">
            Forge / NeoForge / Fabric / Quilt 只能选一个；OptiFine 可单独安装，或与 Forge 一起安装（放入 mods）。
            选 Fabric / Quilt 时默认同时安装 Fabric API / QFAPI。
            Forge / NeoForge / OptiFine 的安装器会用该版本对应的 Java 运行，未下载时自动下载。
          </span>
        </div>
      </div>
      <template #footer>
        <button class="btn" @click="target = null">取消</button>
        <button class="btn primary" :disabled="installing || !instName.trim()" @click="install">
          <span v-if="installing" class="spinner" /> 开始安装
        </button>
      </template>
    </GlassDialog>
  </section>
</template>

<style scoped>
.page {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 22px 26px 0;
  gap: 14px;
}
.page-head {
  display: flex;
  align-items: center;
  gap: 18px;
}
.page-head h2 {
  font-size: 24px;
}
.content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.search {
  position: relative;
  flex: 1;
  max-width: 320px;
}
.search :deep(.icon) {
  position: absolute;
  left: 11px;
  top: 10px;
}
.search .input {
  padding-left: 34px;
}
.list {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 2px 4px 22px 0;
}
.ver {
  --radius: 18px;
  --blur: 16px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 14px;
  flex: none;
  transition: transform 0.45s var(--spring);
}
.ver:hover {
  transform: translateX(3px) scale(1.005);
}
/* 官方未提供下载的愚人节版本：灰色、整行可点击（打开 Wiki） */
.ver.offline {
  cursor: pointer;
  opacity: 0.6;
  filter: grayscale(0.85);
  transition: transform 0.45s var(--spring), opacity 0.2s ease, filter 0.2s ease;
}
.ver.offline:hover,
.ver.offline:focus-visible {
  opacity: 0.85;
  filter: grayscale(0.4);
  outline: none;
}
.ver.offline .wiki {
  pointer-events: none;
}
.ver-info {
  flex: 1;
  min-width: 0;
}
.ver-id {
  font-weight: 650;
  font-size: 15px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.ver-sub {
  font-size: 12px;
  margin-top: 2px;
}
.error-card {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--danger);
}
.error-card span {
  flex: 1;
  white-space: pre-wrap;
}
.tasks {
  gap: 10px;
}
.install {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.hint {
  font-size: 12px;
  line-height: 1.6;
}
.page-head :deep(.segmented) {
  flex: 1;
  max-width: 760px;
}
</style>
