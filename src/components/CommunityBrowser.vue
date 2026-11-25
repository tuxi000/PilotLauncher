<script setup lang="ts">
// 社区资源浏览：搜索 + 平台切换 + 游戏版本 / 加载器 / 分类 / 排序筛选，玻璃卡片网格，无限滚动
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import Icon from './Icon.vue'
import Segmented from './Segmented.vue'
import ResourceDetail from './ResourceDetail.vue'
import GlassSelect from './GlassSelect.vue'
import type { SelectOption } from '../selectOption'
import { loaderIcon } from '../instanceIcons'
import {
  api,
  errMsg,
  formatCount,
  formatDate,
  KIND_LABEL,
  LOADER_LABEL,
  type Category,
  type ResourceKind,
  type ResourceSummary,
  type SearchPlatform,
  type SortKey,
} from '../api'
import { store, currentInstance, requestImport } from '../store'

const props = defineProps<{ kind: ResourceKind }>()

const KIND_ICON: Record<ResourceKind, string> = {
  mod: 'puzzle',
  modpack: 'package',
  resourcepack: 'image',
  shaderpack: 'sun',
  datapack: 'code',
  world: 'map',
}

const cfKey = computed(() => !!store.sys?.curseforgeConfigured)
const isWorld = computed(() => props.kind === 'world')
// Modrinth 没有地图分类：地图固定为 CurseForge
const platform = ref<SearchPlatform>(props.kind === 'world' ? 'curseforge' : 'modrinth')
const effectivePlatform = computed<SearchPlatform>(() => (isWorld.value ? 'curseforge' : platform.value))
const blocked = computed(() => effectivePlatform.value === 'curseforge' && !cfKey.value)

const keyword = ref('')
const mcVersion = ref('')
const loader = ref('')
const category = ref('')
const sort = ref<SortKey>('relevance')
const hasLoaderFilter = computed(() => props.kind === 'mod' || props.kind === 'modpack')

const hits = ref<ResourceSummary[]>([])
const total = ref(0)
const page = ref(0)
const hasMore = ref(false)
const loading = ref(false)
const error = ref('')
const notices = ref<string[]>([])
const broken = reactive<Record<string, boolean>>({})
const selected = ref<ResourceSummary | null>(null)

const gameVersions = ref<string[]>([])
const categories = ref<Category[]>([])

const versionOptions = computed<SelectOption<string>[]>(() => [
  { value: '', label: '全部游戏版本' },
  ...gameVersions.value.map((v) => ({ value: v, label: v })),
])
const loaderOptions = computed<SelectOption<string>[]>(() => [
  { value: '', label: '全部加载器' },
  ...['fabric', 'forge', 'neoforge', 'quilt'].map((l) => ({ value: l, label: LOADER_LABEL[l], icon: loaderIcon(l) ?? undefined })),
])
const categoryOptions = computed<SelectOption<string>[]>(() => [
  { value: '', label: '全部分类' },
  ...categories.value.map((c) => ({ value: c.id, label: c.name })),
])
const SORT_OPTIONS: SelectOption<SortKey>[] = [
  { value: 'relevance', label: '相关度' },
  { value: 'downloads', label: '下载量' },
  { value: 'follows', label: '关注 / 人气' },
  { value: 'updated', label: '最近更新' },
  { value: 'newest', label: '最新发布' },
]

const targetInst = computed(
  () => store.instances.find((i) => i.name === store.communityTarget) ?? currentInstance.value ?? null,
)

let seq = 0
async function search(reset = true) {
  if (blocked.value) {
    hits.value = []
    total.value = 0
    hasMore.value = false
    return
  }
  if (reset) {
    page.value = 0
    hits.value = []
    hasMore.value = false
    notices.value = []
  }
  const my = ++seq
  loading.value = true
  error.value = ''
  try {
    const res = await api.communitySearch({
      platform: effectivePlatform.value,
      kind: props.kind,
      keyword: keyword.value.trim(),
      mcVersion: mcVersion.value || null,
      loader: hasLoaderFilter.value ? loader.value || null : null,
      category: category.value || null,
      sort: sort.value,
      page: page.value,
      pageSize: 24,
    })
    if (my !== seq) return
    const seen = new Set(hits.value.map((h) => h.platform + h.id))
    hits.value.push(...res.hits.filter((h) => !seen.has(h.platform + h.id)))
    total.value = res.total
    hasMore.value = res.hasMore
    notices.value = res.notices
    page.value++
  } catch (e) {
    if (my === seq) error.value = errMsg(e)
  } finally {
    if (my === seq) loading.value = false
  }
}

function loadMore() {
  if (!loading.value && hasMore.value && !error.value) search(false)
}

async function loadCategories() {
  categories.value = []
  category.value = ''
  const p = effectivePlatform.value
  if (p === 'all' || (p === 'curseforge' && !cfKey.value)) return
  try {
    categories.value = await api.communityCategories(p, props.kind)
  } catch {
    /* 分类加载失败不影响搜索 */
  }
}

let timer = 0
watch(keyword, () => {
  window.clearTimeout(timer)
  timer = window.setTimeout(() => search(true), 450)
})
watch([mcVersion, loader, category, sort], () => search(true))
watch(effectivePlatform, async () => {
  await loadCategories()
  search(true)
})

function useInstance() {
  const i = targetInst.value
  if (!i) return
  mcVersion.value = i.mcVersion
  const k = i.loader?.kind
  if (hasLoaderFilter.value && k && k !== 'optifine') loader.value = k
}

// 无限滚动
const sentinel = ref<HTMLElement | null>(null)
let io: IntersectionObserver | null = null
onMounted(async () => {
  io = new IntersectionObserver((entries) => {
    if (entries.some((e) => e.isIntersecting)) loadMore()
  }, { rootMargin: '300px' })
  if (sentinel.value) io.observe(sentinel.value)
  api
    .manifest(false)
    .then((m) => (gameVersions.value = m.versions.filter((v) => v.type === 'release').map((v) => v.id)))
    .catch(() => {})
  await loadCategories()
  search(true)
})
onUnmounted(() => io?.disconnect())

function importLocal() {
  if (props.kind === 'world') requestImport('world', targetInst.value?.name ?? null)
  else requestImport('modpack')
}
</script>

<template>
  <div class="browser">
    <div class="toolbar">
      <div class="search">
        <Icon name="search" :size="16" class="faint" />
        <input v-model="keyword" class="input" :placeholder="`搜索${KIND_LABEL[kind]}`" @keydown.enter="search(true)" />
      </div>
      <Segmented
        v-if="!isWorld"
        v-model="platform"
        :options="[
          { value: 'modrinth', label: 'Modrinth' },
          { value: 'curseforge', label: 'CurseForge', disabled: !cfKey },
          { value: 'all', label: '全部' },
        ]"
      />
      <span v-else class="badge accent" title="Modrinth 没有地图分类，地图只从 CurseForge 获取">CurseForge</span>
      <span class="spacer" />
      <button v-if="kind === 'modpack' || kind === 'world'" class="btn sm" @click="importLocal">
        <Icon name="upload" :size="15" /> {{ kind === 'world' ? '导入本地地图' : '导入本地整合包' }}
      </button>
    </div>

    <div class="filters">
      <GlassSelect v-model="mcVersion" class="flt" :options="versionOptions" size="sm" :min-width="180" />
      <GlassSelect v-if="hasLoaderFilter" v-model="loader" class="flt" :options="loaderOptions" size="sm" />
      <GlassSelect v-if="categories.length" v-model="category" class="flt" :options="categoryOptions" size="sm" :min-width="200" />
      <GlassSelect v-model="sort" class="flt" :options="SORT_OPTIONS" size="sm" />
      <button v-if="targetInst" class="btn sm ghost" :title="`游戏版本 ${targetInst.mcVersion}`" @click="useInstance">
        <Icon name="filter" :size="14" /> 适配 {{ targetInst.name }}
      </button>
      <span class="spacer" />
      <span v-if="total" class="faint count">共 {{ formatCount(total) }} 个</span>
    </div>

    <div v-if="blocked" class="glass card notice">
      <h4><Icon name="key" :size="18" /> 需要 CurseForge API Key</h4>
      <p class="muted">
        <template v-if="isWorld">Modrinth 没有地图分类，地图只能从 CurseForge 搜索。</template>
        当前没有配置 CurseForge API Key：复制 <code>config.local.example.json</code> 为 <code>config.local.json</code>，
        填写 <code>curseforgeApiKey</code>，放到启动器 exe 同目录（或 <code>.minecraft\PilotLauncher\</code>）后重启启动器。
      </p>
      <p v-if="isWorld" class="muted">
        不配置 Key 也可以<b>导入本地地图</b>：点右上角“导入本地地图”选择 zip，或直接把 zip 拖进窗口。
      </p>
      <p v-else class="muted">没有 Key 时可以使用 Modrinth。</p>
    </div>
    <div v-else-if="!cfKey && !isWorld" class="tip faint">未配置 CurseForge API Key，只能使用 Modrinth（见 config.local.example.json）。</div>
    <div v-for="n in notices" :key="n" class="tip warn">{{ n }}</div>

    <div class="scroller">
      <div v-if="error" class="glass card error-card">
        <Icon name="alert" />
        <span>{{ error }}</span>
        <button class="btn sm" @click="search(true)">重试</button>
      </div>
      <TransitionGroup name="list" tag="div" class="grid">
        <article
          v-for="h in hits"
          :key="h.platform + h.id"
          class="rc glass-lite"
          role="button"
          tabindex="0"
          @click="selected = h"
          @keydown.enter="selected = h"
        >
          <img
            v-if="h.iconUrl && !broken[h.platform + h.id]"
            class="ico"
            :src="h.iconUrl"
            loading="lazy"
            referrerpolicy="no-referrer"
            alt=""
            @error="broken[h.platform + h.id] = true"
          />
          <span v-else class="ico ph"><Icon :name="KIND_ICON[kind]" :size="22" /></span>
          <div class="rc-body">
            <div class="rc-title ellipsis" :title="h.title">{{ h.title }}</div>
            <div class="rc-meta faint ellipsis">
              <span v-if="h.author">{{ h.author }} · </span>
              <Icon name="download" :size="12" class="inline" /> {{ formatCount(h.downloads) }}
              <span v-if="h.updated"> · {{ formatDate(h.updated) }}</span>
            </div>
            <p class="rc-desc muted">{{ h.description }}</p>
            <div class="chips">
              <span class="badge" :class="h.platform === 'modrinth' ? 'success' : 'warning'">
                {{ h.platform === 'modrinth' ? 'Modrinth' : 'CurseForge' }}
              </span>
              <span v-for="c in h.categories.slice(0, 3)" :key="c" class="badge">{{ c }}</span>
            </div>
          </div>
        </article>
      </TransitionGroup>
      <div ref="sentinel" class="sentinel">
        <span v-if="loading" class="row faint"><span class="spinner" /> 加载中…</span>
        <span v-else-if="hasMore" class="btn sm ghost" @click="loadMore">加载更多</span>
        <span v-else-if="hits.length" class="faint">没有更多了</span>
        <span v-else-if="!error && !blocked" class="faint">没有找到符合条件的{{ KIND_LABEL[kind] }}</span>
      </div>
    </div>

    <ResourceDetail :summary="selected" :kind="kind" @close="selected = null" />
  </div>
</template>

<style scoped>
.browser {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.toolbar,
.filters {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.search {
  position: relative;
  flex: 1;
  min-width: 220px;
  max-width: 380px;
}
.search :deep(.icon) {
  position: absolute;
  left: 11px;
  top: 10px;
}
.search .input {
  padding-left: 34px;
}
.flt {
  width: 140px;
  flex: none;
}
.count {
  font-size: 12px;
}
.notice h4 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.notice p {
  margin: 6px 0 0;
  line-height: 1.7;
  font-size: 13px;
}
.notice code {
  font-family: var(--mono);
  font-size: 12px;
  padding: 1px 5px;
  border-radius: 6px;
  background: var(--field-bg);
}
.tip {
  font-size: 12px;
}
.tip.warn {
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
  white-space: pre-wrap;
}
.scroller {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 2px 4px 22px 0;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 10px;
}
.rc {
  --radius: 18px;
  display: flex;
  gap: 12px;
  padding: 12px;
  cursor: pointer;
  min-width: 0;
  transition: transform 0.45s var(--spring), background 0.2s ease;
}
.rc:hover {
  transform: translateY(-2px) scale(1.01);
}
.ico {
  width: 56px;
  height: 56px;
  flex: none;
  border-radius: 14px;
  object-fit: cover;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--glass-edge);
}
.ico.ph {
  display: grid;
  place-items: center;
  color: var(--accent);
}
.rc-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.rc-title {
  font-weight: 650;
  font-size: 14.5px;
}
.rc-meta {
  font-size: 11.5px;
}
.inline {
  display: inline-block;
  vertical-align: -2px;
}
.rc-desc {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 2px;
}
.chips .badge {
  height: 18px;
  font-size: 10.5px;
}
.sentinel {
  display: flex;
  justify-content: center;
  padding: 18px 0 6px;
  font-size: 12.5px;
}
.error-card {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--danger);
  margin-bottom: 10px;
}
.error-card span {
  flex: 1;
  white-space: pre-wrap;
}
</style>
