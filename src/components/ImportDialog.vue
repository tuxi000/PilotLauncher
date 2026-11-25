<script setup lang="ts">
// 本地导入：整合包（.mrpack / CurseForge zip）与地图 zip。由文件选择对话框或拖放触发。
import { computed, ref, watch } from 'vue'
import Icon from './Icon.vue'
import GlassDialog from './GlassDialog.vue'
import GlassSelect from './GlassSelect.vue'
import { resolveIcon } from '../instanceIcons'
import type { SelectOption } from '../selectOption'
import { api, errMsg, uniqueInstanceName, type ArchiveInfo } from '../api'
import { store, currentInstance, toast, toastError } from '../store'

const info = ref<ArchiveInfo | null>(null)
const inspecting = ref(false)
const error = ref('')
const instance = ref('')
const name = ref('')
const submitting = ref(false)

const instanceOptions = computed<SelectOption<string>[]>(() =>
  store.instances.map((i) => ({
    value: i.name,
    label: i.name,
    sub: i.mcVersion,
    icon: resolveIcon(i.icon, i.loader?.kind, i.versionType),
  })),
)
const fileName = computed(() => store.importRequest?.path.split(/[\\/]/).pop() ?? '')
const mode = computed<'world' | 'modpack' | null>(() => {
  const k = info.value?.kind
  if (k === 'world') return 'world'
  if (k === 'mrpack' || k === 'curseforge') return 'modpack'
  return null
})
const mismatch = computed(() => {
  const h = store.importRequest?.hint
  if (!h || !mode.value || h === mode.value) return ''
  return mode.value === 'world' ? '这个文件是地图存档（不是整合包），将作为地图导入。' : '这个文件是整合包（不是地图），将创建新的实例。'
})

watch(
  () => store.importRequest,
  async (r) => {
    info.value = null
    error.value = ''
    if (!r) return
    inspecting.value = true
    try {
      const i = await api.inspectArchive(r.path)
      info.value = i
      instance.value = r.instance ?? store.communityTarget ?? currentInstance.value?.name ?? store.instances[0]?.name ?? ''
      const stem = fileName.value.replace(/\.(zip|mrpack)$/i, '')
      name.value = uniqueInstanceName(i.name || stem, store.instances)
      if (i.kind === 'unknown') error.value = '无法识别这个文件：里面既没有整合包清单（modrinth.index.json / manifest.json），也没有存档（level.dat）。'
    } catch (e) {
      error.value = errMsg(e)
    } finally {
      inspecting.value = false
    }
  },
)

function close() {
  store.importRequest = null
}

async function submit() {
  const r = store.importRequest
  if (!r || !mode.value) return
  submitting.value = true
  try {
    if (mode.value === 'world') {
      if (!instance.value) return toast('请选择目标实例', 'error')
      await api.importWorld(r.path, instance.value)
      toast(`已开始导入地图到 ${instance.value}，进度见“下载 → 任务”`, 'success')
    } else {
      if (!name.value.trim()) return toast('请填写实例名称', 'error')
      await api.importModpack(r.path, name.value.trim())
      toast(`已开始导入整合包 ${name.value}，进度见“下载 → 任务”`, 'success')
    }
    close()
  } catch (e) {
    toastError(e)
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <GlassDialog :open="!!store.importRequest" title="导入本地文件" :width="520" @close="close">
    <div class="imp">
      <div class="file glass-lite">
        <Icon :name="mode === 'world' ? 'map' : mode === 'modpack' ? 'package' : 'upload'" :size="22" />
        <div class="ellipsis">
          <div class="fname ellipsis" :title="store.importRequest?.path">{{ fileName }}</div>
          <div class="faint small ellipsis">{{ store.importRequest?.path }}</div>
        </div>
      </div>

      <div v-if="inspecting" class="row faint"><span class="spinner" /> 正在识别…</div>
      <div v-else-if="error" class="err">{{ error }}</div>
      <template v-else-if="info && mode">
        <div v-if="mismatch" class="warn small">{{ mismatch }}</div>
        <template v-if="mode === 'world'">
          <div class="faint small">
            找到 {{ info.worlds.length }} 个存档：{{ info.worlds.join('、') }}。会自动定位 level.dat 所在目录，重名时自动改名。
          </div>
          <div class="field">
            <label>目标实例（解压到 实例/saves/）</label>
            <GlassSelect v-model="instance" :options="instanceOptions" placeholder="（还没有实例）" empty-text="还没有实例" />
          </div>
        </template>
        <template v-else>
          <div class="pack small">
            <div><span class="faint">类型</span> {{ info.kind === 'mrpack' ? 'Modrinth 整合包（.mrpack）' : 'CurseForge 整合包' }}</div>
            <div v-if="info.name"><span class="faint">名称</span> {{ info.name }} {{ info.version ?? '' }}</div>
            <div><span class="faint">游戏版本</span> {{ info.mcVersion }}</div>
            <div><span class="faint">加载器</span> {{ info.loaders.join(' + ') || '原版' }}</div>
            <div><span class="faint">文件</span> {{ info.fileCount }} 个</div>
            <div v-if="info.summary" class="faint">{{ info.summary }}</div>
          </div>
          <div class="field">
            <label>新实例名称</label>
            <input v-model="name" class="input" maxlength="64" @keydown.enter="submit" />
          </div>
          <div v-if="info.kind === 'curseforge' && !store.sys?.curseforgeConfigured" class="warn small">
            CurseForge 整合包需要 API Key 才能获取文件下载地址（见 config.local.example.json）。
          </div>
        </template>
      </template>
    </div>
    <template #footer>
      <button class="btn" @click="close">取消</button>
      <button class="btn primary" :disabled="!mode || inspecting || submitting" @click="submit">
        <span v-if="submitting" class="spinner" /> {{ mode === 'world' ? '导入地图' : '导入整合包' }}
      </button>
    </template>
  </GlassDialog>
</template>

<style scoped>
.imp {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.file {
  --radius: 14px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  color: var(--accent);
  min-width: 0;
}
.file > div {
  min-width: 0;
  color: var(--text);
}
.fname {
  font-weight: 600;
}
.small {
  font-size: 12px;
  line-height: 1.6;
}
.err {
  color: var(--danger);
  white-space: pre-wrap;
}
.warn {
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
}
.pack {
  display: grid;
  gap: 4px;
}
.pack .faint {
  display: inline-block;
  min-width: 64px;
}
</style>
