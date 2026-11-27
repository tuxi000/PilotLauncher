<script setup lang="ts">
// 社区资源详情：简介（markdown / HTML 净化后渲染）、截图、版本列表、安装到实例（含依赖确认）
import { computed, reactive, ref, watch } from 'vue'
import Icon from './Icon.vue'
import Segmented from './Segmented.vue'
import GlassDialog from './GlassDialog.vue'
import GlassSwitch from './GlassSwitch.vue'
import GlassSelect from './GlassSelect.vue'
import GlassCheck from './GlassCheck.vue'
import { resolveIcon } from '../instanceIcons'
import type { SelectOption } from '../selectOption'
import {
  api,
  errMsg,
  formatBytes,
  formatCount,
  formatDate,
  uniqueInstanceName,
  versionLoaders,
  KIND_LABEL,
  LOADER_LABEL,
  type PlanItem,
  type ProjectDetail,
  type ResourceKind,
  type ResourceSummary,
  type ResourceVersion,
} from '../api'
import { renderBody, resolveLink } from '../markdown'
import { store, currentInstance, toast, toastError } from '../store'

const props = defineProps<{ summary: ResourceSummary | null; kind: ResourceKind }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const tab = ref<'desc' | 'gallery' | 'versions'>('desc')
const detail = ref<ProjectDetail | null>(null)
const detailError = ref('')
const loadingDetail = ref(false)

const versions = ref<ResourceVersion[]>([])
const versionsError = ref('')
const loadingVersions = ref(false)
const filterOn = ref(true)
const showAll = ref(false)

const instName = ref('')
const saves = ref<string[]>([])
const save = ref('')
const packName = ref('')
const lightbox = ref<string | null>(null)
const iconBroken = ref(false)

const isModpack = computed(() => props.kind === 'modpack')
const instanceOptions = computed<SelectOption<string>[]>(() =>
  store.instances.map((i) => ({
    value: i.name,
    label: i.name,
    sub: `${i.mcVersion}${i.loader ? ` · ${LOADER_LABEL[i.loader.kind] ?? i.loader.kind} ${i.loader.version}` : ' · 原版'}`,
    icon: resolveIcon(i.icon, i.loader?.kind, i.versionType),
  })),
)
const saveOptions = computed<SelectOption<string>[]>(() => saves.value.map((w) => ({ value: w, label: w })))
const targetInst = computed(() => store.instances.find((i) => i.name === instName.value) ?? null)
const html = computed(() => (detail.value ? renderBody(detail.value.body, detail.value.bodyFormat) : ''))

watch(
  () => props.summary,
  (s) => {
    detail.value = null
    detailError.value = ''
    versions.value = []
    versionsError.value = ''
    plan.value = null
    tab.value = 'desc'
    showAll.value = false
    iconBroken.value = false
    if (!s) return
    instName.value = store.communityTarget ?? currentInstance.value?.name ?? store.instances[0]?.name ?? ''
    packName.value = uniqueInstanceName(s.title, store.instances)
    loadDetail()
    loadVersions()
    loadSaves()
  },
)

async function loadDetail() {
  const s = props.summary
  if (!s) return
  loadingDetail.value = true
  try {
    const d = await api.communityProject(s.platform, s.id, props.kind)
    if (props.summary?.id === s.id) detail.value = d
  } catch (e) {
    detailError.value = errMsg(e)
  } finally {
    loadingDetail.value = false
  }
}

let vseq = 0
async function loadVersions() {
  const s = props.summary
  if (!s) return
  const my = ++vseq
  loadingVersions.value = true
  versionsError.value = ''
  const inst = targetInst.value
  const useFilter = filterOn.value && !!inst && !isModpack.value
  const mc = useFilter ? inst!.mcVersion : null
  const loaders = useFilter ? versionLoaders(props.kind, inst) : props.kind === 'datapack' ? ['datapack'] : []
  try {
    const list = await api.communityVersions(s.platform, s.id, mc, loaders)
    if (my === vseq) versions.value = list
  } catch (e) {
    if (my === vseq) versionsError.value = errMsg(e)
  } finally {
    if (my === vseq) loadingVersions.value = false
  }
}

async function loadSaves() {
  saves.value = []
  save.value = ''
  if (props.kind !== 'datapack' || !instName.value) return
  try {
    saves.value = await api.listSaves(instName.value)
    save.value = saves.value[0] ?? ''
  } catch {
    /* 没有存档 */
  }
}

watch([instName, filterOn], () => {
  if (!props.summary) return
  loadVersions()
})
watch(instName, () => {
  if (props.summary) loadSaves()
})

const shownVersions = computed(() => (showAll.value ? versions.value : versions.value.slice(0, 60)))
const latestCompatible = computed(() => versions.value.find((v) => v.releaseType === 'release') ?? versions.value[0] ?? null)

const loaderWarning = computed(() => {
  if (props.kind !== 'mod' || !targetInst.value) return ''
  const k = targetInst.value.loader?.kind
  if (!k || k === 'optifine') return `实例 ${targetInst.value.name} 没有安装模组加载器（Forge / NeoForge / Fabric / Quilt），模组不会生效。可在实例的“资源管理 → 加载器”中补装。`
  return ''
})

function onBodyClick(e: MouseEvent) {
  const a = (e.target as HTMLElement | null)?.closest('a')
  if (!a) return
  e.preventDefault()
  const url = resolveLink(a.getAttribute('href'), props.summary?.platform ?? 'modrinth')
  if (url) api.openUrl(url).catch(toastError)
}

function primary(v: ResourceVersion) {
  return v.files.find((f) => f.primary) ?? v.files[0] ?? null
}

// ── 安装 ──
const plan = ref<PlanItem[] | null>(null)
const checked = reactive<Record<string, boolean>>({})
const busy = ref<string | null>(null)

function planKey(p: PlanItem) {
  return `${p.platform}:${p.projectId}`
}

function planToggleable(p: PlanItem) {
  return !!p.version && !!primary(p.version)?.url && !(p.installed && !p.root)
}

async function install(v: ResourceVersion) {
  const s = props.summary
  if (!s) return
  const f = primary(v)
  if (!f?.url) {
    toast('该文件的作者不允许第三方启动器下载，已为你打开项目页，请手动下载', 'error', 6000)
    api.openUrl(s.pageUrl).catch(() => {})
    return
  }
  if (isModpack.value) {
    if (!packName.value.trim()) return toast('请填写新实例的名称', 'error')
    busy.value = v.id
    try {
      await api.communityInstallModpack(v, packName.value.trim(), s.title)
      toast(`已开始安装整合包 ${s.title}，进度见“下载 → 任务”`, 'success')
      emit('close')
    } catch (e) {
      toastError(e)
    } finally {
      busy.value = null
    }
    return
  }
  if (!instName.value) return toast('请先选择要安装到的实例', 'error')
  if (props.kind === 'datapack' && !save.value) return toast('数据包需要安装到存档：请先选择存档（该实例还没有存档时，请先进入游戏创建一个世界）', 'error', 6000)
  busy.value = v.id
  try {
    const items = await api.communityPlan(instName.value, props.kind, s.title, s.iconUrl, v)
    if (items.length === 1 && !items[0].installed) {
      await doInstall(items)
    } else {
      for (const k of Object.keys(checked)) delete checked[k]
      for (const p of items) {
        const downloadable = !!p.version && !!primary(p.version)?.url
        checked[planKey(p)] = p.root || (!p.installed && downloadable)
      }
      plan.value = items
    }
  } catch (e) {
    toastError(e)
  } finally {
    busy.value = null
  }
}

async function doInstall(items: PlanItem[]) {
  const list = items
    .filter((p) => p.version && primary(p.version)?.url)
    .map((p) => ({ title: p.title, iconUrl: p.iconUrl, version: p.version! }))
  if (!list.length) return toast('没有可安装的文件', 'error')
  await api.communityInstall(instName.value, props.kind, props.kind === 'datapack' ? save.value : null, list)
  toast(`已开始安装 ${list.length} 个${KIND_LABEL[props.kind]}到 ${instName.value}，进度见“下载 → 任务”`, 'success')
  store.communityTarget = instName.value
  plan.value = null
}

const confirming = ref(false)
async function confirmPlan() {
  if (!plan.value) return
  confirming.value = true
  try {
    await doInstall(plan.value.filter((p) => checked[planKey(p)]))
  } catch (e) {
    toastError(e)
  } finally {
    confirming.value = false
  }
}

const releaseLabel: Record<string, string> = { release: '正式版', beta: '测试版', alpha: '早期版' }
</script>

<template>
  <GlassDialog :open="!!summary" :width="960" @close="emit('close')">
    <div v-if="summary" class="detail">
      <header class="head">
        <img
          v-if="summary.iconUrl && !iconBroken"
          class="big-ico"
          :src="summary.iconUrl"
          referrerpolicy="no-referrer"
          alt=""
          @error="iconBroken = true"
        />
        <span v-else class="big-ico ph"><Icon name="cube" :size="30" /></span>
        <div class="titles">
          <h2 class="ellipsis">{{ summary.title }}</h2>
          <div class="faint meta">
            <span v-if="summary.author">{{ summary.author }} · </span>
            {{ formatCount(summary.downloads) }} 次下载
            <span v-if="summary.updated"> · 更新于 {{ formatDate(summary.updated) }}</span>
            · <span class="badge" :class="summary.platform === 'modrinth' ? 'success' : 'warning'">
              {{ summary.platform === 'modrinth' ? 'Modrinth' : 'CurseForge' }}
            </span>
          </div>
          <p class="muted desc">{{ summary.description }}</p>
          <div v-if="detail" class="links">
            <button v-for="l in detail.links" :key="l.url" class="btn sm ghost" @click="api.openUrl(l.url)">
              <Icon name="external" :size="14" /> {{ l.label }}
            </button>
          </div>
        </div>
      </header>

      <!-- 安装目标 -->
      <div class="target glass-lite">
        <template v-if="isModpack">
          <Icon name="package" :size="18" />
          <span class="field-label">新实例名称</span>
          <input v-model="packName" class="input sm" maxlength="64" />
          <span class="faint small">整合包会创建新的实例，并自动安装对应的游戏版本与加载器</span>
        </template>
        <template v-else>
          <Icon name="cube" :size="18" />
          <span class="field-label">安装到实例</span>
          <GlassSelect
            v-model="instName"
            class="tsel"
            :options="instanceOptions"
            size="sm"
            :min-width="280"
            placeholder="（还没有实例）"
            empty-text="还没有实例"
          />
          <template v-if="kind === 'datapack'">
            <span class="field-label">存档</span>
            <GlassSelect
              v-model="save"
              class="tsel"
              :options="saveOptions"
              size="sm"
              :min-width="200"
              placeholder="（没有存档）"
              empty-text="该实例还没有存档"
            />
          </template>
          <span class="spacer" />
          <span class="faint small">只看适配该实例的版本</span>
          <GlassSwitch v-model="filterOn" />
        </template>
        <button
          class="btn sm primary"
          :disabled="!latestCompatible || !!busy || (!isModpack && !instName)"
          @click="latestCompatible && install(latestCompatible)"
        >
          <span v-if="busy && latestCompatible && busy === latestCompatible.id" class="spinner" />
          <Icon v-else name="download" :size="15" /> {{ isModpack ? '安装最新版' : '安装最新兼容版本' }}
        </button>
      </div>
      <div v-if="loaderWarning" class="warn small">{{ loaderWarning }}</div>

      <Segmented
        v-model="tab"
        :options="[
          { value: 'desc', label: '简介' },
          { value: 'gallery', label: `截图${detail?.gallery.length ? ` ${detail.gallery.length}` : ''}` },
          { value: 'versions', label: `版本${versions.length ? ` ${versions.length}` : ''}` },
        ]"
        class="tabs"
      />

      <div class="pane">
        <template v-if="tab === 'desc'">
          <div v-if="loadingDetail" class="empty"><span class="spinner" /> 正在加载简介…</div>
          <div v-else-if="detailError" class="warn">{{ detailError }}</div>
          <!-- eslint-disable-next-line vue/no-v-html -- 内容已经过 DOMPurify 净化 -->
          <article v-else class="md" @click="onBodyClick" v-html="html" />
        </template>

        <template v-else-if="tab === 'gallery'">
          <div v-if="!detail?.gallery.length" class="empty">没有截图</div>
          <div v-else class="gallery">
            <figure v-for="g in detail.gallery" :key="g.url" class="shot" @click="lightbox = g.url">
              <img :src="g.url" loading="lazy" referrerpolicy="no-referrer" alt="" />
              <figcaption v-if="g.title" class="faint">{{ g.title }}</figcaption>
            </figure>
          </div>
        </template>

        <template v-else>
          <div v-if="loadingVersions" class="empty"><span class="spinner" /> 正在获取版本列表…</div>
          <div v-else-if="versionsError" class="warn">{{ versionsError }}</div>
          <div v-else-if="!versions.length" class="empty">
            没有{{ filterOn && targetInst && !isModpack ? `适用于 ${targetInst.mcVersion} 的` : '' }}版本
            <button v-if="filterOn && !isModpack" class="btn sm" @click="filterOn = false">显示全部版本</button>
          </div>
          <div v-else class="vlist">
            <div v-for="v in shownVersions" :key="v.id" class="vrow glass-lite lift">
              <div class="vinfo">
                <div class="vname ellipsis" :title="v.name">
                  {{ v.name }}
                  <span class="badge" :class="v.releaseType === 'release' ? 'success' : 'warning'">
                    {{ releaseLabel[v.releaseType] ?? v.releaseType }}
                  </span>
                </div>
                <div class="faint vsub ellipsis">
                  {{ v.gameVersions.slice(0, 6).join(', ') }}{{ v.gameVersions.length > 6 ? ' …' : '' }}
                  <template v-if="v.loaders.length"> · {{ v.loaders.map((l) => LOADER_LABEL[l] ?? l).join(' / ') }}</template>
                  · {{ formatDate(v.date) }}
                  <template v-if="primary(v)"> · {{ formatBytes(primary(v)!.size) }}</template>
                  <template v-if="v.dependencies.some((d) => d.kind === 'required')">
                    · {{ v.dependencies.filter((d) => d.kind === 'required').length }} 个必需依赖
                  </template>
                </div>
              </div>
              <button
                v-if="primary(v)?.url"
                class="btn sm"
                :disabled="!!busy || (!isModpack && !instName)"
                @click="install(v)"
              >
                <span v-if="busy === v.id" class="spinner" />
                <Icon v-else name="download" :size="14" /> {{ isModpack ? '安装整合包' : '安装' }}
              </button>
              <button v-else class="btn sm ghost" title="作者不允许第三方下载" @click="api.openUrl(summary.pageUrl)">
                <Icon name="external" :size="14" /> 手动下载
              </button>
            </div>
            <button v-if="versions.length > 60 && !showAll" class="btn sm ghost more" @click="showAll = true">
              显示全部 {{ versions.length }} 个版本
            </button>
          </div>
        </template>
      </div>
    </div>
  </GlassDialog>

  <!-- 依赖确认 -->
  <GlassDialog :open="!!plan" title="确认安装" :width="560" @close="plan = null">
    <p class="muted small">将安装到 <b>{{ instName }}</b>{{ kind === 'datapack' ? ` 的存档 ${save}` : '' }}。必需依赖已自动勾选，已安装的会跳过：</p>
    <div class="plan">
      <div
        v-for="p in plan ?? []"
        :key="planKey(p)"
        class="pi glass-lite"
        :class="{ dim: p.installed && !p.root }"
        @click="planToggleable(p) && (checked[planKey(p)] = !checked[planKey(p)])"
      >
        <GlassCheck v-model="checked[planKey(p)]" :disabled="!planToggleable(p)" />
        <img v-if="p.iconUrl" :src="p.iconUrl" class="pi-ico" referrerpolicy="no-referrer" alt="" />
        <span v-else class="pi-ico ph"><Icon name="puzzle" :size="16" /></span>
        <span class="pi-text">
          <span class="ellipsis">{{ p.title }}</span>
          <span class="faint ellipsis small">
            {{ p.root ? '所选资源' : '必需依赖' }}
            <template v-if="p.version"> · {{ p.version.versionNumber || p.version.name }}</template>
            <template v-if="p.root && p.installed"> · 已安装（将替换为所选版本）</template>
            <template v-if="p.note"> · {{ p.note }}</template>
          </span>
        </span>
      </div>
    </div>
    <template #footer>
      <button class="btn" @click="plan = null">取消</button>
      <button class="btn primary" :disabled="confirming" @click="confirmPlan">
        <span v-if="confirming" class="spinner" /> 安装所选
      </button>
    </template>
  </GlassDialog>

  <Teleport to="body">
    <Transition name="fade">
      <div v-if="lightbox" class="lightbox" @click="lightbox = null">
        <img :src="lightbox" referrerpolicy="no-referrer" alt="" />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 60vh;
}
.head {
  display: flex;
  gap: 16px;
}
.big-ico {
  width: 84px;
  height: 84px;
  flex: none;
  border-radius: 20px;
  object-fit: cover;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--glass-edge), 0 10px 26px -14px rgba(0, 0, 0, 0.5);
}
.big-ico.ph {
  display: grid;
  place-items: center;
  color: var(--accent);
}
.titles {
  flex: 1;
  min-width: 0;
}
.titles h2 {
  font-size: 22px;
}
.meta {
  font-size: 12.5px;
  margin-top: 4px;
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}
.desc {
  margin: 6px 0 0;
  font-size: 13px;
  line-height: 1.55;
}
.links {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 6px;
}
.target {
  --radius: 16px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  flex-wrap: wrap;
}
.tsel {
  width: 220px;
  flex: none;
}
.input.sm {
  width: auto;
  min-width: 160px;
  height: 30px;
  font-size: 12.5px;
  padding: 0 10px;
  border-radius: 10px;
}
.small {
  font-size: 12px;
}
.warn {
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
  white-space: pre-wrap;
}
.tabs {
  align-self: flex-start;
}
.pane {
  min-height: 0;
}
.md {
  user-select: text;
  line-height: 1.7;
  font-size: 13.5px;
  overflow-wrap: anywhere;
}
.md :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 10px;
}
.md :deep(a) {
  cursor: pointer;
}
.md :deep(h1),
.md :deep(h2),
.md :deep(h3) {
  margin: 18px 0 8px;
}
.md :deep(pre),
.md :deep(code) {
  font-family: var(--mono);
  font-size: 12px;
  background: var(--field-bg);
  border-radius: 8px;
}
.md :deep(pre) {
  padding: 10px 12px;
  overflow: auto;
}
.md :deep(code) {
  padding: 1px 5px;
}
.md :deep(table) {
  border-collapse: collapse;
}
.md :deep(td),
.md :deep(th) {
  border: 1px solid var(--divider);
  padding: 4px 8px;
}
.md :deep(blockquote) {
  margin: 8px 0;
  padding: 4px 12px;
  border-left: 3px solid var(--accent);
  color: var(--text-2);
}
.gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 10px;
}
.shot {
  margin: 0;
  cursor: zoom-in;
}
.shot img {
  width: 100%;
  aspect-ratio: 16 / 9;
  object-fit: cover;
  border-radius: 14px;
  box-shadow: 0 8px 20px -12px rgba(0, 0, 0, 0.5);
  transition: transform 0.45s var(--spring);
}
.shot img:hover {
  transform: scale(1.02);
}
.shot figcaption {
  font-size: 12px;
  margin-top: 4px;
}
.vlist {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.vrow {
  --radius: 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
}
.vinfo {
  flex: 1;
  min-width: 0;
}
.vname {
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}
.vsub {
  font-size: 11.5px;
  margin-top: 2px;
}
.more {
  align-self: center;
}
.plan {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 46vh;
  overflow: auto;
  margin-top: 8px;
}
.pi {
  --radius: 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  cursor: pointer;
}
.pi.dim {
  opacity: 0.6;
}
.pi-ico {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  object-fit: cover;
  flex: none;
}
.pi-ico.ph {
  display: grid;
  place-items: center;
  background: var(--field-bg);
  color: var(--accent);
}
.pi-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.lightbox {
  position: fixed;
  inset: 0;
  z-index: 400;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.72);
  cursor: zoom-out;
}
.lightbox img {
  max-width: 92vw;
  max-height: 88vh;
  border-radius: 14px;
  box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.8);
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
