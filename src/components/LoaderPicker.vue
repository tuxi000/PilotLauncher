<script setup lang="ts">
// 加载器选择卡片：Forge / NeoForge / Fabric / Quilt / OptiFine，带版本下拉框与互斥逻辑
//  - Forge、NeoForge、Fabric、Quilt 四选一
//  - OptiFine 可单独安装，或搭配 Forge（放进 mods）；不能搭配 Fabric / Quilt（提示改用 OptiFabric）、NeoForge
//  - 选好游戏版本后并行预取所有加载器的版本列表，每张卡片单独显示加载状态
//  - 选了 Fabric / Quilt 时显示“Fabric API / QFAPI”一行（默认勾选），加载器装好后自动下载到 mods
//  - 愚人节版本不支持任何加载器：全部卡片禁用，不请求版本列表
import { computed, reactive, watch } from 'vue'
import Icon from './Icon.vue'
import InstanceIcon from './InstanceIcon.vue'
import GlassSelect from './GlassSelect.vue'
import GlassCheck from './GlassCheck.vue'
import {
  api,
  errMsg,
  formatDate,
  LOADER_ADDONS,
  LOADER_LABEL,
  type InstallItem,
  type LoaderKind,
  type LoaderSpec,
  type LoaderVersion,
  type ResourceVersion,
} from '../api'
import { loaderIcon } from '../instanceIcons'
import { addonVersions, loaderVersions } from '../loaderVersions'
import { isAprilFoolsId } from '../aprilFools'
import type { SelectOption, SelectTag } from '../selectOption'

const props = defineProps<{
  mcVersion: string
  modelValue: LoaderSpec[]
  addons?: InstallItem[]
  /** 给已有实例补装时传入：用于检查是否已经装过 Fabric API */
  instance?: string | null
  /** 版本类型（实例的 versionType 或下载页算出的类型）；april_fools 时禁用全部加载器 */
  versionType?: string | null
}>()
const emit = defineEmits<{
  (e: 'update:modelValue', v: LoaderSpec[]): void
  (e: 'update:addons', v: InstallItem[]): void
}>()

const KINDS: LoaderKind[] = ['forge', 'neoforge', 'fabric', 'quilt', 'optifine']
const PRIMARY: LoaderKind[] = ['forge', 'neoforge', 'fabric', 'quilt']
const NOTES: Partial<Record<LoaderKind, string>> = {
  quilt: '仅官方源（BMCLAPI 没有 Quilt 镜像）',
  optifine: '版本列表与下载固定使用 BMCLAPI',
  neoforge: '1.20.1 及以上',
}
const COLORS: Record<LoaderKind, string> = {
  forge: '#df7d3a',
  neoforge: '#e0752c',
  fabric: '#c6b38b',
  quilt: '#9b6bdb',
  optifine: '#c7344a',
}

const APRIL_FOOLS_NOTE = '愚人节版本不支持模组加载器'
/** 愚人节版本：类型由父组件传入；只有版本号时按内置名单判断（与后端 april_fools.rs 一致） */
const aprilFools = computed(() => props.versionType === 'april_fools' || isAprilFoolsId(props.mcVersion))

interface CardState {
  on: boolean
  loading: boolean
  loaded: boolean
  error: string
  versions: LoaderVersion[]
  selected: string
  stableOnly: boolean
}

const cards = reactive<Record<LoaderKind, CardState>>(
  Object.fromEntries(
    KINDS.map((k) => [
      k,
      { on: false, loading: false, loaded: false, error: '', versions: [], selected: '', stableOnly: false },
    ]),
  ) as unknown as Record<LoaderKind, CardState>,
)

interface AddonState {
  enabled: boolean
  loading: boolean
  loaded: boolean
  error: string
  versions: ResourceVersion[]
  selected: string
  /** 实例中已安装的版本号（补装时） */
  installed: string | null
}
const ADDON_KINDS = Object.keys(LOADER_ADDONS) as LoaderKind[]
const addonState = reactive<Record<string, AddonState>>(
  Object.fromEntries(
    ADDON_KINDS.map((k) => [
      k,
      { enabled: true, loading: false, loaded: false, error: '', versions: [], selected: '', installed: null },
    ]),
  ),
)

let gen = 0

async function load(k: LoaderKind, force = false) {
  const c = cards[k]
  if (c.loading || (c.loaded && !force)) return
  const my = gen
  const mc = props.mcVersion
  c.loading = true
  c.error = ''
  try {
    const list = await loaderVersions(k, mc, force)
    if (my !== gen) return
    c.versions = list
    c.loaded = true
    if (!list.some((v) => v.id === c.selected)) {
      const pick = list.find((v) => v.recommended) ?? list.find((v) => v.latest) ?? list[0]
      c.selected = pick?.id ?? ''
    }
  } catch (e) {
    if (my !== gen) return
    c.error = errMsg(e)
    c.loaded = true
    c.on = false
  } finally {
    if (my === gen) {
      c.loading = false
      sync()
    }
  }
}

async function loadAddon(k: LoaderKind) {
  const a = addonState[k]
  if (!a || a.loading || a.loaded) return
  const my = gen
  const mc = props.mcVersion
  a.loading = true
  a.error = ''
  try {
    const list = await addonVersions(k, mc)
    if (my !== gen) return
    a.versions = list
    const pick = list.find((v) => v.releaseType === 'release') ?? list[0]
    a.selected = pick?.id ?? ''
  } catch (e) {
    if (my !== gen) return
    a.error = errMsg(e)
  } finally {
    if (my === gen) {
      a.loading = false
      a.loaded = true
      syncAddons()
    }
  }
}

async function detectInstalled() {
  const name = props.instance
  if (!name) return
  const my = gen
  try {
    const mods = await api.listResources(name, 'mod')
    if (my !== gen) return
    for (const k of ADDON_KINDS) {
      const ids = LOADER_ADDONS[k]?.modIds ?? []
      const hit = mods.find((m) => m.modId && ids.includes(m.modId))
      addonState[k].installed = hit ? hit.version || hit.file : null
    }
  } catch {
    /* 读取失败不影响安装 */
  }
}

// 游戏版本变化：清空并重新并行预取全部加载器（愚人节版本不预取）
watch(
  () => [props.mcVersion, aprilFools.value] as const,
  ([mc, fools]) => {
    gen++
    for (const k of KINDS) {
      Object.assign(cards[k], { on: false, loading: false, loaded: false, error: '', versions: [], selected: '' })
    }
    for (const k of ADDON_KINDS) {
      Object.assign(addonState[k], { enabled: true, loading: false, loaded: false, error: '', versions: [], selected: '', installed: null })
    }
    emit('update:modelValue', [])
    emit('update:addons', [])
    if (!mc || fools) return
    for (const k of KINDS) load(k)
    for (const k of ADDON_KINDS) loadAddon(k)
    detectInstalled()
  },
  { immediate: true },
)

const primary = computed(() => PRIMARY.find((k) => cards[k].on) ?? null)

function blockedReason(k: LoaderKind): string | null {
  if (aprilFools.value) return APRIL_FOOLS_NOTE
  const p = primary.value
  if (k === 'optifine') {
    if (p === 'fabric' || p === 'quilt') return `OptiFine 不能与 ${LOADER_LABEL[p]} 共存，可在模组里搜索 OptiFabric（或使用 Sodium / Iris）`
    if (p === 'neoforge') return 'OptiFine 不支持 NeoForge'
    return null
  }
  if ((k === 'fabric' || k === 'quilt' || k === 'neoforge') && cards.optifine.on) {
    return `${LOADER_LABEL[k]} 不能与 OptiFine 共存`
  }
  return null
}

function toggle(k: LoaderKind) {
  const c = cards[k]
  if (c.on) {
    c.on = false
    sync()
    return
  }
  if (blockedReason(k)) return
  if (PRIMARY.includes(k)) {
    for (const o of PRIMARY) if (o !== k) cards[o].on = false
  }
  c.on = true
  if (c.error) {
    // 之前失败过：重新请求（跳过缓存）
    c.loaded = false
    load(k, true)
  } else {
    load(k)
  }
  sync()
}

function shown(k: LoaderKind): LoaderVersion[] {
  const c = cards[k]
  return c.stableOnly ? c.versions.filter((v) => v.stable || v.id === c.selected) : c.versions
}

function options(k: LoaderKind): SelectOption<string>[] {
  return shown(k).map((v) => {
    const tags: SelectTag[] = []
    if (v.recommended) tags.push({ text: '推荐', tone: 'success' })
    if (v.latest) tags.push({ text: '最新', tone: 'accent' })
    if (!v.stable) tags.push({ text: '测试版', tone: 'warning' })
    return { value: v.id, label: v.version, sub: v.note ?? undefined, tags }
  })
}

function cardNote(k: LoaderKind): string {
  const c = cards[k]
  if (aprilFools.value) return '不可用'
  if (c.error) return c.error
  const b = blockedReason(k)
  if (b) return b
  if (c.loaded && c.versions.length) {
    const rec = c.versions.find((v) => v.recommended) ?? c.versions[0]
    return `${c.versions.length} 个版本 · ${rec.recommended ? '推荐' : '最新'} ${rec.version}`
  }
  return NOTES[k] ?? '点击选择'
}

function sync() {
  const specs: LoaderSpec[] = []
  for (const k of KINDS) {
    const c = cards[k]
    if (c.on && c.selected) specs.push({ kind: k, version: c.selected })
  }
  emit('update:modelValue', specs)
  syncAddons()
}

// ── Fabric API / QFAPI ──
const addonKind = computed<LoaderKind | null>(() => {
  const p = primary.value
  return p && LOADER_ADDONS[p] ? p : null
})
const addon = computed(() => (addonKind.value ? addonState[addonKind.value] : null))
const addonMeta = computed(() => (addonKind.value ? LOADER_ADDONS[addonKind.value] ?? null : null))

function addonOptions(a: AddonState): SelectOption<string>[] {
  return a.versions.map((v, i) => {
    const tags: SelectTag[] = []
    if (i === 0) tags.push({ text: '最新', tone: 'accent' })
    if (v.releaseType !== 'release') tags.push({ text: '测试版', tone: 'warning' })
    return {
      value: v.id,
      label: v.versionNumber || v.name,
      sub: `${v.gameVersions.slice(0, 4).join(', ')}${v.gameVersions.length > 4 ? ' …' : ''} · ${formatDate(v.date)}`,
      tags,
    }
  })
}

function syncAddons() {
  const k = addonKind.value
  const a = k ? addonState[k] : null
  const meta = k ? LOADER_ADDONS[k] : null
  const out: InstallItem[] = []
  if (k && a && meta && cards[k].on && a.enabled) {
    const v = a.versions.find((x) => x.id === a.selected)
    if (v) out.push({ title: meta.title, iconUrl: null, version: v })
  }
  emit('update:addons', out)
}

// 勾选、改选版本、版本列表加载完成后同步给父组件
watch(
  () => [addonKind.value, addon.value?.enabled, addon.value?.selected, addon.value?.versions.length],
  () => syncAddons(),
)

const summary = computed(() => {
  if (!props.modelValue.length) return '原版（不安装加载器）'
  const parts = props.modelValue.map((s) => {
    const v = cards[s.kind].versions.find((x) => x.id === s.version)
    return `${LOADER_LABEL[s.kind]} ${v?.version ?? s.version}`
  })
  for (const a of props.addons ?? []) parts.push(`${a.title} ${a.version.versionNumber || a.version.name}`)
  return parts.join(' + ')
})

const loadingAny = computed(() => KINDS.some((k) => cards[k].on && cards[k].loading))
defineExpose({ loadingAny })
</script>

<template>
  <div class="picker">
    <div v-if="aprilFools" class="fools glass-lite">
      <InstanceIcon icon="gold_block" plain :size="20" />
      <span>{{ APRIL_FOOLS_NOTE }}，将只安装原版。</span>
    </div>
    <div class="cards">
      <div
        v-for="k in KINDS"
        :key="k"
        class="lc glass-lite"
        :class="{ on: cards[k].on, blocked: !cards[k].on && !!blockedReason(k), unsupported: !cards[k].on && !!cards[k].error }"
        :style="{ '--lc': COLORS[k] }"
        :title="blockedReason(k) ?? cards[k].error ?? ''"
        role="button"
        tabindex="0"
        @click="toggle(k)"
        @keydown.enter.self="toggle(k)"
      >
        <div class="lc-head">
          <InstanceIcon :icon="loaderIcon(k)" plain :size="24" />
          <span class="lc-name">{{ LOADER_LABEL[k] }}</span>
          <span class="spacer" />
          <span v-if="cards[k].loading" class="spinner tiny" title="正在获取版本列表" />
          <span class="check"><Icon v-if="cards[k].on" name="check" :size="14" :stroke="2.4" /></span>
        </div>
        <div v-if="cards[k].on" class="lc-body" @click.stop>
          <template v-if="cards[k].loading && !cards[k].versions.length">
            <span class="skeleton" />
            <span class="skeleton short" />
          </template>
          <template v-else-if="cards[k].versions.length">
            <GlassSelect
              v-model="cards[k].selected"
              :options="options(k)"
              size="sm"
              :min-width="260"
              placeholder="选择版本"
              @change="sync"
            />
            <GlassCheck v-model="cards[k].stableOnly" class="stable faint" label="只看稳定版" />
          </template>
        </div>
        <div v-else class="lc-note faint">
          <template v-if="cards[k].loading">
            <span class="skeleton" />
          </template>
          <template v-else>{{ cardNote(k) }}</template>
        </div>
      </div>
    </div>

    <!-- Fabric API / QFAPI -->
    <Transition name="addon">
      <div v-if="addon && addonMeta" class="addon glass-lite" :style="{ '--lc': COLORS[addonKind!] }">
        <GlassCheck v-model="addon.enabled" :disabled="!addon.versions.length">
          <b>{{ addonMeta.title }}</b>
        </GlassCheck>
        <span class="faint small addon-tip">装好加载器后自动下载到 mods（绝大多数 {{ LOADER_LABEL[addonKind!] }} 模组都需要它）</span>
        <span class="spacer" />
        <template v-if="addon.loading">
          <span class="row faint small"><span class="spinner tiny" /> 正在查找适配 {{ mcVersion }} 的版本…</span>
        </template>
        <template v-else-if="addon.error">
          <span class="warn small" :title="addon.error">获取 {{ addonMeta.title }} 版本失败，将不安装</span>
        </template>
        <template v-else-if="!addon.versions.length">
          <span class="faint small">没有适配此版本的 {{ addonMeta.title }}</span>
        </template>
        <GlassSelect
          v-else
          v-model="addon.selected"
          class="addon-select"
          :options="addonOptions(addon)"
          :disabled="!addon.enabled"
          size="sm"
          :min-width="300"
        />
        <div v-if="addon.installed && addon.versions.length" class="installed faint small">
          实例中已有 {{ addonMeta.title }} {{ addon.installed }}<template v-if="addon.enabled">，安装后将替换为所选版本</template>
        </div>
      </div>
    </Transition>

    <div class="sum faint">将安装：<b>{{ summary }}</b></div>
  </div>
</template>

<style scoped>
.picker {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.fools {
  --radius: 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  font-size: 12.5px;
  font-weight: 600;
  color: color-mix(in srgb, var(--warning) 70%, var(--text));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--warning) 40%, transparent), inset 0 1px 0 var(--hl-top);
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 8px;
}
.lc {
  --radius: 16px;
  padding: 10px 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 78px;
  transition: transform 0.45s var(--spring), box-shadow 0.2s ease, opacity 0.2s;
}
.lc:hover {
  transform: translateY(-2px);
}
.lc.on {
  box-shadow: inset 0 0 0 1.5px var(--lc), 0 8px 22px -12px var(--lc), inset 0 1px 0 var(--hl-top);
}
.lc.blocked {
  opacity: 0.5;
  cursor: not-allowed;
}
.lc.unsupported {
  opacity: 0.62;
}
.lc-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.lc-name {
  font-weight: 650;
}
.check {
  width: 20px;
  height: 20px;
  border-radius: 7px;
  display: grid;
  place-items: center;
  color: #fff;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  transition: background 0.2s ease;
}
.lc.on .check {
  background: var(--lc);
}
.lc-note {
  font-size: 11.5px;
  line-height: 1.45;
}
.lc-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
  cursor: default;
}
.stable {
  font-size: 11.5px;
}
.spinner.tiny {
  width: 13px;
  height: 13px;
  border-width: 2px;
  color: var(--text-3);
}
.skeleton {
  display: block;
  height: 12px;
  border-radius: 6px;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--text) 8%, transparent) 0%,
    color-mix(in srgb, var(--text) 16%, transparent) 50%,
    color-mix(in srgb, var(--text) 8%, transparent) 100%
  );
  background-size: 200% 100%;
  animation: shimmer 1.2s linear infinite;
}
.lc-body .skeleton {
  height: 30px;
  border-radius: 10px;
}
.skeleton.short {
  width: 55%;
  height: 12px !important;
}
@keyframes shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}
.addon {
  --radius: 14px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 12px;
  padding: 10px 12px;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--lc) 45%, transparent), inset 0 1px 0 var(--hl-top);
}
.addon-tip {
  min-width: 0;
}
.addon-select {
  width: 240px;
  flex: none;
}
.installed {
  flex-basis: 100%;
}
.small {
  font-size: 12px;
}
.warn {
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
}
.sum {
  font-size: 12.5px;
}
.sum b {
  color: var(--text);
}
.addon-enter-active {
  transition: transform 0.45s var(--spring), opacity 0.2s ease;
}
.addon-leave-active {
  transition: transform 0.15s ease, opacity 0.15s ease;
}
.addon-enter-from,
.addon-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.98);
}
</style>
