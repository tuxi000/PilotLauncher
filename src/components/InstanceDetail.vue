<script setup lang="ts">
// 实例资源管理：模组 / 资源包 / 光影包 / 存档（启用、禁用、删除、定位文件），以及加载器补装 / 更换
import { computed, reactive, ref, watch } from 'vue'
import Icon from './Icon.vue'
import Segmented from './Segmented.vue'
import GlassDialog from './GlassDialog.vue'
import GlassSwitch from './GlassSwitch.vue'
import LoaderPicker from './LoaderPicker.vue'
import {
  api,
  errMsg,
  formatBytes,
  relativeTime,
  LOADER_LABEL,
  type InstallItem,
  type LoaderSpec,
  type LocalKind,
  type LocalResource,
  type MissingFile,
  type ResourceKind,
} from '../api'
import { store, requestImport, resourceVersion, toast, toastError } from '../store'
import { prefetchLoaders } from '../loaderVersions'
import { isAprilFoolsId } from '../aprilFools'

type Tab = LocalKind | 'loader'
const tab = ref<Tab>('mod')
const items = ref<LocalResource[]>([])
const loading = ref(false)
const error = ref('')
const keyword = ref('')
const pendingDelete = ref<string | null>(null)
const busy = reactive<Record<string, boolean>>({})
const missing = ref<MissingFile[]>([])
const specs = ref<LoaderSpec[]>([])
const addons = ref<InstallItem[]>([])
const installingLoader = ref(false)

const lk = computed<LocalKind>(() => (tab.value === 'loader' ? 'mod' : tab.value))
const inst = computed(() => store.instances.find((i) => i.name === store.detailInstance) ?? null)
const SUB: Record<LocalKind, string> = { mod: 'mods', resourcepack: 'resourcepacks', shaderpack: 'shaderpacks', world: 'saves' }
const ICON: Record<LocalKind, string> = { mod: 'puzzle', resourcepack: 'image', shaderpack: 'sun', world: 'map' }

async function load() {
  const name = store.detailInstance
  if (!name || tab.value === 'loader') return
  const kind = tab.value
  loading.value = true
  error.value = ''
  try {
    const list = await api.listResources(name, kind)
    if (store.detailInstance === name && tab.value === kind) items.value = list
  } catch (e) {
    error.value = errMsg(e)
  } finally {
    loading.value = false
  }
}

watch(
  () => store.detailInstance,
  async (n) => {
    items.value = []
    pendingDelete.value = null
    keyword.value = ''
    specs.value = []
    addons.value = []
    missing.value = []
    if (!n) return
    tab.value = 'mod'
    load()
    // 提前预取加载器版本列表，切到“加载器”标签时基本已经加载好（愚人节版本不支持加载器，不预取）
    const found = store.instances.find((i) => i.name === n)
    const mc = found?.mcVersion
    const fools = found?.versionType === 'april_fools' || isAprilFoolsId(mc)
    if (mc && !fools) prefetchLoaders(mc)
    try {
      missing.value = await api.getMissingFiles(n)
    } catch {
      missing.value = []
    }
  },
)
watch(tab, () => {
  items.value = []
  pendingDelete.value = null
  load()
})
// 安装 / 导入任务完成后自动刷新
watch(resourceVersion, () => {
  if (store.detailInstance) load()
})

const shown = computed(() => {
  const k = keyword.value.trim().toLowerCase()
  if (!k) return items.value
  return items.value.filter(
    (r) => r.name.toLowerCase().includes(k) || r.file.toLowerCase().includes(k) || (r.modId ?? '').toLowerCase().includes(k),
  )
})
const enabledCount = computed(() => items.value.filter((r) => r.enabled).length)

async function toggle(r: LocalResource, enabled: boolean) {
  const name = store.detailInstance
  if (!name || tab.value === 'loader') return
  const key = r.file
  busy[key] = true
  try {
    const file = await api.setResourceEnabled(name, tab.value, r.file, enabled)
    r.file = file
    r.enabled = enabled
  } catch (e) {
    toastError(e)
  } finally {
    delete busy[key]
  }
}

async function remove(r: LocalResource) {
  const name = store.detailInstance
  if (!name || tab.value === 'loader') return
  if (pendingDelete.value !== r.file) {
    pendingDelete.value = r.file
    return
  }
  try {
    await api.deleteResource(name, tab.value, r.file)
    items.value = items.value.filter((x) => x.file !== r.file)
    toast(`已删除 ${r.name}`, 'success')
  } catch (e) {
    toastError(e)
  } finally {
    pendingDelete.value = null
  }
}

function reveal(r: LocalResource) {
  const name = store.detailInstance
  if (!name || tab.value === 'loader') return
  api.revealResource(name, tab.value, r.file).catch(toastError)
}

function openFolder() {
  const name = store.detailInstance
  if (!name || tab.value === 'loader') return
  api.openInstanceDir(name, SUB[tab.value]).catch(toastError)
}

function downloadMore() {
  if (!store.detailInstance || tab.value === 'loader') return
  store.communityTarget = store.detailInstance
  store.page = 'download'
  store.downloadTab = tab.value as ResourceKind
  store.detailInstance = null
}

async function installLoader() {
  const name = store.detailInstance
  if (!name || !specs.value.length) return
  installingLoader.value = true
  try {
    await api.installLoader(name, specs.value, addons.value)
    toast(`已开始为 ${name} 安装加载器，进度见“下载 → 任务”`, 'success')
    store.detailInstance = null
  } catch (e) {
    toastError(e)
  } finally {
    installingLoader.value = false
  }
}

async function dismissMissing() {
  if (!store.detailInstance) return
  await api.dismissMissingFiles(store.detailInstance).catch(() => {})
  missing.value = []
}

const loaderText = computed(() => {
  const i = inst.value
  if (!i) return ''
  const parts: string[] = []
  if (i.loader) parts.push(`${LOADER_LABEL[i.loader.kind] ?? i.loader.kind} ${i.loader.version}`)
  if (i.optifine && i.loader?.kind !== 'optifine') parts.push(`OptiFine ${i.optifine}`)
  return parts.length ? parts.join(' + ') : '原版（未安装加载器）'
})
</script>

<template>
  <GlassDialog
    :open="!!store.detailInstance"
    :title="`资源管理 · ${store.detailInstance ?? ''}`"
    :width="880"
    @close="store.detailInstance = null"
  >
    <div v-if="inst" class="wrap">
      <div class="info faint">
        Minecraft {{ inst.mcVersion }} · {{ loaderText }}
      </div>

      <div v-if="missing.length" class="missing glass-lite">
        <div class="row">
          <Icon name="alert" :size="18" />
          <b>整合包中有 {{ missing.length }} 个文件不允许第三方下载，需要手动下载后放入对应文件夹</b>
          <span class="spacer" />
          <button class="btn sm ghost" @click="dismissMissing">不再提示</button>
        </div>
        <div v-for="m in missing" :key="m.url + m.fileName" class="mf">
          <span class="ellipsis">{{ m.name }} <span class="faint">（{{ m.fileName }} → {{ m.target }}）</span></span>
          <button class="btn sm" @click="api.openUrl(m.url)"><Icon name="external" :size="14" /> 下载页</button>
          <button class="btn sm ghost" @click="api.openInstanceDir(inst.name, m.target)"><Icon name="folder" :size="14" /></button>
        </div>
      </div>

      <div class="bar">
        <Segmented
          v-model="tab"
          :options="[
            { value: 'mod', label: '模组' },
            { value: 'resourcepack', label: '资源包' },
            { value: 'shaderpack', label: '光影包' },
            { value: 'world', label: '存档' },
            { value: 'loader', label: '加载器' },
          ]"
        />
        <template v-if="tab !== 'loader'">
          <div class="search">
            <Icon name="search" :size="15" class="faint" />
            <input v-model="keyword" class="input" placeholder="筛选" />
          </div>
          <span class="spacer" />
          <button class="btn sm icon" title="刷新" :disabled="loading" @click="load"><Icon name="refresh" :size="15" /></button>
          <button class="btn sm icon" title="打开文件夹" @click="openFolder"><Icon name="folder" :size="15" /></button>
          <button v-if="tab === 'world'" class="btn sm" @click="requestImport('world', inst.name)">
            <Icon name="upload" :size="14" /> 导入地图
          </button>
          <button class="btn sm primary" @click="downloadMore"><Icon name="download" :size="14" /> 下载更多</button>
        </template>
      </div>

      <!-- 加载器 -->
      <div v-if="tab === 'loader'" class="loader">
        <p class="muted small">
          当前：<b>{{ loaderText }}</b>。安装或更换加载器时会从原版版本文件重新生成实例 JSON；mods、存档与配置不会被删除。
          Forge / NeoForge / OptiFine 的安装器会使用该实例的 Java 运行。
        </p>
        <LoaderPicker
          v-model="specs"
          v-model:addons="addons"
          :mc-version="inst.mcVersion"
          :version-type="inst.versionType"
          :instance="inst.name"
        />
        <div class="row end">
          <button class="btn primary" :disabled="!specs.length || installingLoader || inst.running" @click="installLoader">
            <span v-if="installingLoader" class="spinner" /> {{ inst.loader ? '更换加载器' : '安装加载器' }}
          </button>
        </div>
      </div>

      <!-- 资源列表 -->
      <div v-else class="list">
        <div v-if="error" class="warn">{{ error }}</div>
        <div v-else-if="loading && !items.length" class="empty"><span class="spinner" /> 正在读取…</div>
        <div v-else-if="!items.length" class="empty">
          <Icon :name="ICON[lk]" :size="34" />
          {{ tab === 'world' ? '还没有存档' : '这里还是空的' }}
        </div>
        <template v-else>
          <div class="faint small count">
            共 {{ items.length }} 个<template v-if="tab !== 'world'">，已启用 {{ enabledCount }} 个</template>
          </div>
          <div v-for="r in shown" :key="r.file" class="res glass-lite" :class="{ off: !r.enabled }">
            <img v-if="r.icon || r.iconUrl" class="ico" :src="(r.icon || r.iconUrl)!" referrerpolicy="no-referrer" alt="" />
            <span v-else class="ico ph"><Icon :name="ICON[lk]" :size="20" /></span>
            <div class="rinfo">
              <div class="rname ellipsis" :title="r.file">
                {{ r.name }}
                <span v-if="r.version" class="faint ver">{{ r.version }}</span>
                <span v-if="r.loader" class="badge">{{ LOADER_LABEL[r.loader] ?? r.loader }}</span>
                <span v-if="!r.enabled" class="badge warning">已禁用</span>
              </div>
              <div class="faint rsub ellipsis">
                <template v-if="r.description">{{ r.description }}</template>
                <template v-else>{{ r.file }}</template>
              </div>
              <div class="faint rsub ellipsis">
                <template v-if="tab === 'world'">最近游玩 {{ relativeTime(r.modified) }}</template>
                <template v-else>{{ r.isDir ? '文件夹' : formatBytes(r.size) }}<template v-if="r.authors"> · {{ r.authors }}</template></template>
              </div>
            </div>
            <GlassSwitch
              v-if="tab !== 'world'"
              :model-value="r.enabled"
              :disabled="!!busy[r.file] || inst.running"
              :title="r.enabled ? '禁用' : '启用'"
              @update:model-value="toggle(r, $event)"
            />
            <button class="btn sm icon ghost" title="在资源管理器中显示" @click="reveal(r)"><Icon name="folder" :size="15" /></button>
            <button
              class="btn sm"
              :class="pendingDelete === r.file ? 'danger' : 'icon ghost'"
              :title="pendingDelete === r.file ? '再次点击确认删除' : '删除'"
              :disabled="inst.running"
              @click="remove(r)"
            >
              <Icon name="trash" :size="15" /><template v-if="pendingDelete === r.file"> 确认删除</template>
            </button>
          </div>
        </template>
      </div>
    </div>
  </GlassDialog>
</template>

<style scoped>
.wrap {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 56vh;
}
.info {
  font-size: 12.5px;
  margin-top: -4px;
}
.bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.search {
  position: relative;
  width: 180px;
}
.search :deep(.icon) {
  position: absolute;
  left: 10px;
  top: 8px;
}
.search .input {
  height: 30px;
  padding-left: 30px;
  font-size: 12.5px;
}
.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.count {
  margin-bottom: 2px;
}
.small {
  font-size: 12px;
}
.res {
  --radius: 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  transition: opacity 0.2s ease;
}
.res.off {
  opacity: 0.62;
}
.ico {
  width: 40px;
  height: 40px;
  flex: none;
  border-radius: 10px;
  object-fit: cover;
  image-rendering: pixelated;
  background: var(--field-bg);
}
.ico.ph {
  display: grid;
  place-items: center;
  color: var(--accent);
}
.rinfo {
  flex: 1;
  min-width: 0;
}
.rname {
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}
.ver {
  font-weight: 450;
  font-size: 12px;
}
.rsub {
  font-size: 11.5px;
  margin-top: 1px;
}
.warn {
  color: var(--danger);
  white-space: pre-wrap;
}
.loader {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.loader p {
  margin: 0;
  line-height: 1.7;
}
.end {
  justify-content: flex-end;
}
.missing {
  --radius: 14px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: color-mix(in srgb, var(--warning) 70%, var(--text));
}
.mf {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  color: var(--text);
}
.mf > span {
  flex: 1;
  min-width: 0;
}
</style>
