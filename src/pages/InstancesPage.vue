<script setup lang="ts">
// 实例页：实例的增删改查（版本隔离：每个实例独立的 versions/<名称>/ 游戏目录）
import { computed, reactive, ref } from 'vue'
import Icon from '../components/Icon.vue'
import InstanceIcon from '../components/InstanceIcon.vue'
import GlassDialog from '../components/GlassDialog.vue'
import GlassSwitch from '../components/GlassSwitch.vue'
import MemoryBar from '../components/MemoryBar.vue'
import GlassSelect from '../components/GlassSelect.vue'
import { ICON_LABEL, ICON_NAMES, editableIcon } from '../instanceIcons'
import type { SelectOption } from '../selectOption'
import { api, relativeTime, LOADER_LABEL, VERSION_TYPE_LABEL, type InstanceView, type JavaInfo } from '../api'
import {
  store,
  currentInstance,
  gameState,
  isBusy,
  launchInstance,
  openLog,
  refreshInstances,
  selectInstance,
  toast,
  toastError,
} from '../store'

const keyword = ref('')
const list = computed(() => {
  const k = keyword.value.trim().toLowerCase()
  return store.instances.filter((i) => !k || i.name.toLowerCase().includes(k) || i.mcVersion.toLowerCase().includes(k))
})

/** 直接打开实例的游戏目录 versions/<实例名>/ */
function openFolder(name: string) {
  api.openInstanceDir(name).catch(toastError)
}

async function repair(i: InstanceView) {
  try {
    await api.repairInstance(i.name)
    toast(`已开始修复 ${i.name}，进度见下载页`, 'success')
  } catch (e) {
    toastError(e)
  }
}

// ── 删除 ──
const deleting = ref<InstanceView | null>(null)
async function confirmDelete() {
  if (!deleting.value) return
  const name = deleting.value.name
  try {
    await api.deleteInstance(name)
    toast(`已删除 ${name}`, 'success')
    deleting.value = null
    await refreshInstances()
    if (store.settings?.selectedInstance === name) store.settings.selectedInstance = null
  } catch (e) {
    toastError(e)
  }
}

// ── 编辑 ──
const editing = ref<InstanceView | null>(null)
const form = reactive({
  name: '',
  description: '',
  icon: 'auto',
  javaPath: '',
  memoryOverride: false,
  memoryMb: 4096,
  jvmArgs: '',
  gameArgs: '',
  windowWidth: 0,
  windowHeight: 0,
})
const javas = ref<JavaInfo[]>([])
const javaOptions = computed<SelectOption<string>[]>(() => [
  { value: '', label: '使用全局设置 / 自动', sub: '按版本自动选择 Mojang 官方运行时' },
  ...javas.value.map((j) => ({ value: j.path, label: `Java ${j.major || '?'} · ${j.source}`, sub: j.path })),
])
const javaChoice = computed(() => (javaOptions.value.some((o) => o.value === form.javaPath) ? form.javaPath : null))
const saving = ref(false)
const maxMem = computed(() => Math.max(2048, Math.floor((store.sys?.totalMemoryMb ?? 8192) / 512) * 512))
// 实例实际会用的内存：单独设置 > 全局手动 > 全局自动（自动值由 MemoryBar 实时刷新）
const liveAutoMb = ref<number | null>(null)
const effectiveMem = computed(() => {
  if (form.memoryOverride) return form.memoryMb
  if (store.settings?.memoryMode === 'manual') return store.settings.memoryMb
  return liveAutoMb.value ?? store.sys?.autoMemoryMb ?? 4096
})

function openEdit(i: InstanceView) {
  editing.value = i
  Object.assign(form, {
    name: i.name,
    description: i.description,
    icon: editableIcon(i.icon),
    javaPath: i.javaPath ?? '',
    memoryOverride: i.memoryMb != null,
    memoryMb: i.memoryMb ?? store.sys?.autoMemoryMb ?? 4096,
    jvmArgs: i.jvmArgs,
    gameArgs: i.gameArgs,
    windowWidth: i.windowWidth ?? 0,
    windowHeight: i.windowHeight ?? 0,
  })
  api.listJava().then((l) => (javas.value = l)).catch(() => {})
}

async function saveEdit() {
  if (!editing.value) return
  saving.value = true
  let name = editing.value.name
  try {
    if (form.name.trim() !== name) {
      await api.renameInstance(name, form.name.trim())
      name = form.name.trim()
    }
    await api.updateInstance(name, {
      description: form.description,
      icon: form.icon,
      javaPath: form.javaPath.trim() || null,
      memoryMb: form.memoryOverride ? form.memoryMb : null,
      jvmArgs: form.jvmArgs,
      gameArgs: form.gameArgs,
      windowWidth: form.windowWidth > 0 ? form.windowWidth : null,
      windowHeight: form.windowHeight > 0 ? form.windowHeight : null,
    })
    toast('实例设置已保存', 'success')
    editing.value = null
    await refreshInstances()
    if (store.settings) store.settings = await api.getSettings()
  } catch (e) {
    toastError(e)
    await refreshInstances()
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <section class="page">
    <header class="page-head">
      <h2>实例</h2>
      <span class="badge">{{ store.instances.length }} 个</span>
      <span class="spacer" />
      <div class="search">
        <Icon name="search" :size="16" class="faint" />
        <input v-model="keyword" class="input" placeholder="搜索实例" />
      </div>
      <button class="btn icon" title="刷新" @click="refreshInstances"><Icon name="refresh" :size="18" /></button>
      <button class="btn primary" @click="store.page = 'download'; store.downloadTab = 'versions'">
        <Icon name="plus" :size="16" /> 新建实例
      </button>
    </header>

    <div class="grid-wrap">
      <div v-if="store.instances.length === 0" class="empty">
        <Icon name="cube" :size="40" />
        <div>还没有实例。前往“下载”页选择一个版本安装。</div>
      </div>
      <TransitionGroup name="list" tag="div" class="grid">
        <article
          v-for="i in list"
          :key="i.name"
          class="inst glass"
          :class="{ current: currentInstance?.name === i.name }"
          v-light
        >
          <div class="inst-top">
            <InstanceIcon :name="i.name" :icon="i.icon" :loader="i.loader?.kind" :version-type="i.versionType" :size="52" />
            <div class="inst-info">
              <div class="inst-name ellipsis" :title="i.name">{{ i.name }}</div>
              <div class="faint inst-sub">
                {{ i.mcVersion }} · {{ VERSION_TYPE_LABEL[i.versionType] ?? i.versionType }}
                <template v-if="i.loader"> · {{ LOADER_LABEL[i.loader.kind] ?? i.loader.kind }} {{ i.loader.version }}</template>
                <template v-if="i.optifine && i.loader?.kind !== 'optifine'"> + OptiFine</template>
              </div>
              <div class="badges">
                <span v-if="currentInstance?.name === i.name" class="badge accent">当前</span>
                <span v-if="gameState(i.name)?.state === 'running'" class="badge success">运行中</span>
                <span v-if="!i.installed" class="badge warning">未安装完成</span>
                <span v-if="!i.managed" class="badge">外部版本</span>
              </div>
            </div>
          </div>
          <p v-if="i.description" class="desc muted">{{ i.description }}</p>
          <div class="faint when">最近游玩：{{ relativeTime(i.lastPlayed) }}</div>
          <div class="actions">
            <button
              class="btn sm primary"
              :disabled="isBusy(i.name)"
              @click="selectInstance(i.name); launchInstance(i.name)"
            >
              <Icon name="play" :size="14" /> 启动
            </button>
            <button v-if="isBusy(i.name)" class="btn sm" @click="openLog(i.name)"><Icon name="log" :size="14" /></button>
            <button
              v-if="currentInstance?.name !== i.name"
              class="btn sm"
              title="设为首页的当前实例"
              @click="selectInstance(i.name)"
            >
              <Icon name="check" :size="14" />
            </button>
            <span class="spacer" />
            <button class="btn sm icon" title="资源管理（模组 / 资源包 / 光影包 / 存档 / 加载器）" @click="store.detailInstance = i.name">
              <Icon name="puzzle" :size="15" />
            </button>
            <button class="btn sm icon" title="打开实例文件夹（versions/实例名）" @click="openFolder(i.name)">
              <Icon name="folder" :size="15" />
            </button>
            <button class="btn sm icon" title="实例设置" @click="openEdit(i)"><Icon name="edit" :size="15" /></button>
            <button class="btn sm icon" title="修复文件" :disabled="isBusy(i.name)" @click="repair(i)">
              <Icon name="wrench" :size="15" />
            </button>
            <button class="btn sm icon" title="删除实例" :disabled="isBusy(i.name)" @click="deleting = i">
              <Icon name="trash" :size="15" />
            </button>
          </div>
        </article>
      </TransitionGroup>
    </div>

    <!-- 删除确认 -->
    <GlassDialog :open="!!deleting" title="删除实例" :width="440" @close="deleting = null">
      <p>
        确定要删除 <b>{{ deleting?.name }}</b> 吗？<br />
        实例目录（含 mods、存档、资源包、光影包与配置）将被<b>永久删除</b>，共享的 libraries / assets 不受影响。
      </p>
      <template #footer>
        <button class="btn" @click="deleting = null">取消</button>
        <button class="btn danger" @click="confirmDelete">删除</button>
      </template>
    </GlassDialog>

    <!-- 编辑 -->
    <GlassDialog :open="!!editing" :title="`实例设置 · ${editing?.name ?? ''}`" :width="600" @close="editing = null">
      <div class="form">
        <div class="field">
          <label>名称（修改即重命名实例文件夹）</label>
          <input v-model="form.name" class="input" maxlength="64" />
        </div>
        <div class="field">
          <span class="field-label">图标</span>
          <div class="icons">
            <button
              v-for="ic in ICON_NAMES"
              :key="ic"
              class="ic"
              :class="{ active: form.icon === ic, auto: ic === 'auto' }"
              :title="ic === 'auto' ? '自动：有加载器时显示加载器图标，否则按版本类型（正式版 / 快照 / 远古版本 / 愚人节版本）' : ICON_LABEL[ic]"
              @click="form.icon = ic"
            >
              <InstanceIcon
                :name="form.name"
                :icon="ic"
                :loader="editing?.loader?.kind"
                :version-type="editing?.versionType"
                :size="38"
              />
              <span v-if="ic === 'auto'" class="auto-tag">自动</span>
            </button>
          </div>
          <span class="faint mem-hint">{{ ICON_LABEL[form.icon] ?? form.icon }}</span>
        </div>
        <div class="field">
          <label>备注</label>
          <input v-model="form.description" class="input" placeholder="例如：和朋友一起的生存服" />
        </div>
        <div class="field">
          <div class="row">
            <span class="field-label">单独设置内存</span>
            <span class="spacer" />
            <GlassSwitch v-model="form.memoryOverride" />
          </div>
          <div v-if="form.memoryOverride" class="row">
            <input
              v-model.number="form.memoryMb"
              class="slider"
              type="range"
              min="1024"
              :max="maxMem"
              step="256"
              :style="{ '--pct': `${((form.memoryMb - 1024) / (maxMem - 1024)) * 100}%` }"
            />
            <span class="mem mono">{{ (form.memoryMb / 1024).toFixed(1) }} GB</span>
          </div>
          <div v-else class="faint mem-hint">使用全局设置：{{ (effectiveMem / 1024).toFixed(1) }} GB</div>
          <MemoryBar :allocated-mb="effectiveMem" @status="liveAutoMb = $event.autoMb" />
        </div>
        <div class="field">
          <label>Java（留空则使用全局设置 / 自动选择官方运行时）</label>
          <div class="java-row">
            <input v-model="form.javaPath" class="input" placeholder="例如 C:\Program Files\Java\jdk-21\bin\javaw.exe" />
            <GlassSelect
              class="java-pick"
              :model-value="javaChoice"
              :options="javaOptions"
              :min-width="420"
              placeholder="选择已检测到的 Java"
              @update:model-value="form.javaPath = $event"
            />
          </div>
        </div>
        <div class="two">
          <div class="field">
            <label>附加 JVM 参数</label>
            <textarea v-model="form.jvmArgs" class="textarea" placeholder="-XX:+UseZGC" />
          </div>
          <div class="field">
            <label>附加游戏参数</label>
            <textarea v-model="form.gameArgs" class="textarea" placeholder="--server example.com" />
          </div>
        </div>
        <div class="two">
          <div class="field">
            <label>窗口宽度（0 = 全局设置）</label>
            <input v-model.number="form.windowWidth" class="input" type="number" min="0" />
          </div>
          <div class="field">
            <label>窗口高度（0 = 全局设置）</label>
            <input v-model.number="form.windowHeight" class="input" type="number" min="0" />
          </div>
        </div>
        <div class="faint path mono">{{ editing?.dir }}</div>
      </div>
      <template #footer>
        <button class="btn" @click="editing = null">取消</button>
        <button class="btn primary" :disabled="saving || !form.name.trim()" @click="saveEdit">
          <span v-if="saving" class="spinner" /> 保存
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
  gap: 10px;
}
.page-head h2 {
  font-size: 24px;
}
.search {
  position: relative;
  width: 220px;
}
.search :deep(.icon) {
  position: absolute;
  left: 11px;
  top: 10px;
}
.search .input {
  padding-left: 34px;
}
.grid-wrap {
  flex: 1;
  overflow: auto;
  padding: 2px 4px 24px 0;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
  gap: 14px;
}
.inst {
  --radius: 22px;
  --blur: 18px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: transform 0.5s var(--spring), box-shadow 0.3s ease;
}
.inst:hover {
  transform: translateY(-3px);
  z-index: 2;
}
.inst.current {
  box-shadow: var(--glass-shadow), inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 70%, transparent),
    inset 0 1px 0 var(--hl-top);
}
.inst-top {
  display: flex;
  gap: 14px;
  align-items: center;
}
.inst-info {
  flex: 1;
  min-width: 0;
}
.inst-name {
  font-size: 16px;
  font-weight: 650;
}
.inst-sub {
  font-size: 12.5px;
  margin-top: 2px;
}
.badges {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 6px;
}
.desc {
  margin: 0;
  font-size: 13px;
}
.when {
  font-size: 12px;
}
.actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.icons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.ic.auto {
  position: relative;
}
.auto-tag {
  position: absolute;
  right: -2px;
  bottom: -2px;
  padding: 0 5px;
  height: 16px;
  line-height: 16px;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 700;
  color: #fff;
  background: var(--accent);
  box-shadow: 0 2px 6px -2px var(--accent);
}
.java-row {
  display: flex;
  gap: 8px;
}
.java-row .input {
  flex: 1;
  min-width: 0;
}
.java-pick {
  width: 210px;
  flex: none;
}
.ic {
  padding: 4px;
  border: none;
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 5%, transparent);
  box-shadow: inset 0 0 0 1px var(--divider);
  cursor: pointer;
  transition: transform 0.4s var(--spring), box-shadow 0.2s ease, background 0.2s ease;
}
.ic:hover {
  transform: scale(1.08);
  background: color-mix(in srgb, var(--text) 9%, transparent);
}
.ic.active {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  box-shadow: 0 0 0 2px var(--accent);
}
.mem {
  width: 64px;
  text-align: right;
}
.path {
  font-size: 11.5px;
  word-break: break-all;
  user-select: text;
}
.mem-hint {
  font-size: 12px;
}
</style>
