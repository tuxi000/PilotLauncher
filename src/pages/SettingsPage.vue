<script setup lang="ts">
// 设置页：下载源、并发、内存、Java、外观、清理、关于
import { computed, onMounted, reactive, ref, watch } from 'vue'
import Icon from '../components/Icon.vue'
import Segmented from '../components/Segmented.vue'
import GlassSwitch from '../components/GlassSwitch.vue'
import GlassCheck from '../components/GlassCheck.vue'
import GlassDialog from '../components/GlassDialog.vue'
import AnimatedNumber from '../components/AnimatedNumber.vue'
import MemoryBar from '../components/MemoryBar.vue'
import AmbientBackground from '../components/AmbientBackground.vue'
import {
  api,
  errMsg,
  formatBytes,
  type BackgroundPalette,
  type CleanupCategoryId,
  type CleanupResult,
  type CleanupScan,
  type JavaInfo,
  type MemoryStatus,
  type Settings,
} from '../api'
import { store, saveSettings, toast, toastError } from '../store'

const s = computed(() => store.settings)
const sys = computed(() => store.sys)

function set<K extends keyof Settings>(key: K, value: Settings[K]) {
  saveSettings({ [key]: value } as Partial<Settings>)
}

/** 内置背景的三套配色（色卡里直接渲染缩小的静态背景，所见即所得，自动跟随深浅色） */
const PALETTES: { value: BackgroundPalette; label: string; desc: string }[] = [
  { value: 'blue', label: '蓝紫', desc: '冷静、现代' },
  { value: 'mint', label: '薄荷', desc: '清新' },
  { value: 'coral', label: '珊瑚', desc: '温暖' },
]

// 滑块拖动时先本地更新，松手再保存，避免频繁写盘
const concurrency = ref(32)
const memory = ref(4096)
watch(
  s,
  (v) => {
    if (!v) return
    concurrency.value = v.concurrency
    memory.value = v.memoryMb
  },
  { immediate: true },
)

// 实时内存（由 MemoryBar 每秒刷新）
const memStatus = ref<MemoryStatus | null>(null)
const autoMb = computed(() => memStatus.value?.autoMb ?? sys.value?.autoMemoryMb ?? 0)
const gameMb = computed(() => (s.value?.memoryMode === 'manual' ? memory.value : autoMb.value))

const maxMem = computed(() => Math.max(2048, Math.floor((sys.value?.totalMemoryMb ?? 8192) / 512) * 512))
const pct = (v: number, min: number, max: number) => `${((v - min) / Math.max(1, max - min)) * 100}%`

// 文本类设置：失焦保存
const jvmArgs = ref('')
const gameArgs = ref('')
const gameDir = ref('')
const javaPath = ref('')
const bgPath = ref('')
const winW = ref(0)
const winH = ref(0)
watch(
  s,
  (v) => {
    if (!v) return
    jvmArgs.value = v.jvmArgs
    gameArgs.value = v.gameArgs
    gameDir.value = v.gameDir ?? ''
    javaPath.value = v.javaPath ?? ''
    bgPath.value = v.backgroundImage ?? ''
    winW.value = v.windowWidth
    winH.value = v.windowHeight
  },
  { immediate: true },
)

// Java 检测
const javas = ref<JavaInfo[]>([])
const scanning = ref(false)
async function scan() {
  scanning.value = true
  try {
    javas.value = await api.listJava()
  } catch (e) {
    toastError(e)
  } finally {
    scanning.value = false
  }
}
onMounted(scan)

async function applyJava(path: string | null) {
  if (path) {
    try {
      const info = await api.inspectJava(path)
      toast(`已选择 Java ${info.major || '?'}（${info.version}）`, 'success')
      javaPath.value = info.path
      set('javaPath', info.path)
    } catch (e) {
      toast(errMsg(e), 'error')
    }
  } else {
    javaPath.value = ''
    set('javaPath', null)
    toast('已恢复为自动选择官方 Java 运行时', 'success')
  }
}

function open(path?: string | null) {
  if (path) api.openPath(path).catch(toastError)
}

// ───── 清理 ─────
const cleanScan = ref<CleanupScan | null>(null)
const cleanScanning = ref(false)
const cleaning = ref(false)
const picked = reactive<Record<string, boolean>>({})
const openDetail = ref<string | null>(null)
const lastClean = ref<CleanupResult | null>(null)
const confirmComponents = ref(false)

async function scanClean() {
  cleanScanning.value = true
  try {
    const r = await api.scanCleanup()
    for (const c of r.categories) {
      if (!(c.id in picked)) picked[c.id] = c.defaultSelected
    }
    cleanScan.value = r
  } catch (e) {
    toastError(e)
  } finally {
    cleanScanning.value = false
  }
}

const pickedIds = computed<CleanupCategoryId[]>(() =>
  (cleanScan.value?.categories ?? []).filter((c) => picked[c.id] && c.files > 0).map((c) => c.id),
)
const pickedBytes = computed(() =>
  (cleanScan.value?.categories ?? []).filter((c) => picked[c.id]).reduce((sum, c) => sum + c.bytes, 0),
)

function togglePick(id: string, files: number) {
  if (files > 0) picked[id] = !picked[id]
}

function toggleDetail(id: string) {
  openDetail.value = openDetail.value === id ? null : id
}

function requestClean() {
  if (!pickedIds.value.length) return
  if (pickedIds.value.includes('components')) {
    confirmComponents.value = true
    return
  }
  doClean()
}

async function doClean() {
  confirmComponents.value = false
  const ids = pickedIds.value
  if (!ids.length) return
  cleaning.value = true
  try {
    const r = await api.runCleanup(ids)
    lastClean.value = r
    const base = `已清理 ${r.deletedFiles} 个文件，释放 ${formatBytes(r.freedBytes)}`
    toast(r.failedFiles ? `${base}。另有 ${r.failedFiles} 个文件正在使用，已跳过` : base, 'success')
    await scanClean()
  } catch (e) {
    toastError(e)
  } finally {
    cleaning.value = false
  }
}

const componentsDetails = computed(
  () => cleanScan.value?.categories.find((c) => c.id === 'components')?.details ?? [],
)
</script>

<template>
  <section v-if="s" class="page">
    <header class="page-head">
      <h2>设置</h2>
    </header>

    <div class="scroll stagger">
      <!-- 下载 -->
      <div class="glass card group">
        <h3 class="section-title"><Icon name="download" /> 下载</h3>
        <div class="item">
          <div class="label">
            <div>下载源</div>
            <div class="faint">
              游戏文件、Java、加载器。BMCLAPI 由 bangbang93 提供，国内更快；“官方优先”指官方失败时自动换 BMCLAPI（OptiFine 固定使用 BMCLAPI）
            </div>
          </div>
          <Segmented
            :model-value="s.downloadSource"
            :options="[
              { value: 'official', label: '官方' },
              { value: 'bmclapi', label: 'BMCLAPI' },
              { value: 'official_first', label: '官方优先' },
            ]"
            @update:model-value="set('downloadSource', $event)"
          />
        </div>
        <div class="item">
          <div class="label">
            <div>社区资源源</div>
            <div class="faint">
              Modrinth / CurseForge 的接口与文件，镜像为 MCIM（mod.mcimirror.top）；“官方优先”指官方失败时自动换 MCIM
            </div>
          </div>
          <Segmented
            :model-value="s.communitySource"
            :options="[
              { value: 'official_first', label: '官方优先' },
              { value: 'mirror_first', label: 'MCIM 优先' },
              { value: 'official', label: '仅官方' },
            ]"
            @update:model-value="set('communitySource', $event)"
          />
        </div>
        <div class="item">
          <div class="label">
            <div>并发下载数</div>
            <div class="faint">同时下载的文件数量</div>
          </div>
          <div class="slider-row">
            <input
              v-model.number="concurrency"
              type="range"
              class="slider"
              min="1"
              max="128"
              :style="{ '--pct': pct(concurrency, 1, 128) }"
              @change="set('concurrency', concurrency)"
            />
            <span class="val mono">{{ concurrency }}</span>
          </div>
        </div>
        <div class="item">
          <div class="label">
            <div>失败重试次数</div>
          </div>
          <Segmented
            :model-value="s.retries"
            :options="[0, 1, 3, 5, 8].map((n) => ({ value: n, label: String(n) }))"
            @update:model-value="set('retries', $event)"
          />
        </div>
      </div>

      <!-- 游戏 -->
      <div class="glass card group">
        <h3 class="section-title"><Icon name="cube" /> 游戏</h3>
        <div class="item">
          <div class="label">
            <div>游戏内存</div>
            <div class="faint">自动模式会参考当前可用内存，至少给系统留 0.5 GB</div>
          </div>
          <Segmented
            :model-value="s.memoryMode"
            :options="[
              { value: 'auto', label: `自动（${(autoMb / 1024).toFixed(1)} GB）` },
              { value: 'manual', label: '手动' },
            ]"
            @update:model-value="set('memoryMode', $event)"
          />
        </div>
        <div v-if="s.memoryMode === 'manual'" class="item">
          <div class="label"><div>最大内存</div></div>
          <div class="slider-row">
            <input
              v-model.number="memory"
              type="range"
              class="slider"
              min="1024"
              :max="maxMem"
              step="256"
              :style="{ '--pct': pct(memory, 1024, maxMem) }"
              @change="set('memoryMb', memory)"
            />
            <span class="val mono">{{ (memory / 1024).toFixed(1) }} GB</span>
          </div>
        </div>
        <MemoryBar :allocated-mb="gameMb" @status="memStatus = $event" />

        <div class="divider" />
        <div class="item column">
          <div class="label">
            <div>Java</div>
            <div class="faint">
              默认根据版本自动下载 Mojang 官方运行时（Java 8 / 17 / 21 …）。在此指定后，将覆盖所有实例（实例设置可单独覆盖）。
            </div>
          </div>
          <div class="row full">
            <input
              v-model="javaPath"
              class="input"
              placeholder="自动（推荐）"
              @keydown.enter="applyJava(javaPath || null)"
            />
            <button class="btn sm" @click="applyJava(javaPath || null)">应用</button>
            <button class="btn sm" :disabled="!s.javaPath" @click="applyJava(null)">恢复自动</button>
            <button class="btn sm icon" title="重新扫描" :disabled="scanning" @click="scan">
              <span v-if="scanning" class="spinner" />
              <Icon v-else name="refresh" :size="16" />
            </button>
          </div>
          <div v-if="javas.length" class="javas">
            <button
              v-for="j in javas"
              :key="j.path"
              class="java glass-lite"
              :class="{ active: s.javaPath === j.path }"
              @click="applyJava(j.path)"
            >
              <span class="badge accent">Java {{ j.major || '?' }}</span>
              <span class="ellipsis mono jp" :title="j.path">{{ j.path }}</span>
              <span class="faint">{{ j.source }}</span>
            </button>
          </div>
        </div>

        <div class="divider" />
        <div class="two">
          <div class="field">
            <label>附加 JVM 参数</label>
            <textarea v-model="jvmArgs" class="textarea" placeholder="-XX:+UnlockExperimentalVMOptions" @blur="set('jvmArgs', jvmArgs)" />
          </div>
          <div class="field">
            <label>附加游戏参数</label>
            <textarea v-model="gameArgs" class="textarea" placeholder="--fullscreen" @blur="set('gameArgs', gameArgs)" />
          </div>
        </div>
        <div class="two">
          <div class="field">
            <label>游戏窗口宽度（0 = 默认）</label>
            <input v-model.number="winW" class="input" type="number" min="0" @change="set('windowWidth', winW || 0)" />
          </div>
          <div class="field">
            <label>游戏窗口高度（0 = 默认）</label>
            <input v-model.number="winH" class="input" type="number" min="0" @change="set('windowHeight', winH || 0)" />
          </div>
        </div>
        <div class="field">
          <label>游戏根目录（留空则使用启动器所在文件夹下的 .minecraft；建议路径不含中文和空格）</label>
          <div v-if="sys?.legacyGameDir" class="legacy">
            <Icon name="info" :size="15" />
            <span>旧版默认目录里还有已安装的游戏：<span class="mono">{{ sys.legacyGameDir }}</span></span>
            <button class="btn sm" @click="gameDir = sys.legacyGameDir!; set('gameDir', sys.legacyGameDir)">继续使用旧目录</button>
            <button class="btn sm ghost" @click="open(sys.legacyGameDir)">打开</button>
          </div>
          <div class="row">
            <input v-model="gameDir" class="input" :placeholder="sys?.gameDir" @keydown.enter="set('gameDir', gameDir.trim() || null)" />
            <button class="btn sm" @click="set('gameDir', gameDir.trim() || null)">应用</button>
            <button class="btn sm icon" title="打开" @click="open(sys?.gameDir)"><Icon name="folder" :size="16" /></button>
          </div>
        </div>
        <div class="item">
          <div class="label"><div>启动游戏后</div></div>
          <Segmented
            :model-value="s.launcherVisibility"
            :options="[
              { value: 'keep', label: '保持启动器' },
              { value: 'minimize', label: '最小化启动器' },
            ]"
            @update:model-value="set('launcherVisibility', $event)"
          />
        </div>
      </div>

      <!-- 外观 -->
      <div class="glass card group">
        <h3 class="section-title"><Icon name="palette" /> 外观</h3>
        <div class="item">
          <div class="label"><div>主题</div></div>
          <Segmented
            :model-value="s.theme"
            :options="[
              { value: 'system', label: '跟随系统' },
              { value: 'light', label: '浅色' },
              { value: 'dark', label: '深色' },
            ]"
            @update:model-value="set('theme', $event)"
          />
        </div>
        <div class="item">
          <div class="label">
            <div>窗口材质</div>
            <div class="faint">Windows 11 的 Mica / Acrylic 背景</div>
          </div>
          <Segmented
            :model-value="s.windowEffect"
            :options="[
              { value: 'mica', label: 'Mica' },
              { value: 'tabbed', label: 'Mica Alt' },
              { value: 'acrylic', label: 'Acrylic' },
              { value: 'none', label: '无' },
            ]"
            @update:model-value="set('windowEffect', $event)"
          />
        </div>
        <div class="item">
          <div class="label">
            <div>液态玻璃折射</div>
            <div class="faint">SVG 位移贴图模拟边缘折射；性能较弱的设备可关闭</div>
          </div>
          <GlassSwitch :model-value="s.glassRefraction" @update:model-value="set('glassRefraction', $event)" />
        </div>
        <div class="item column">
          <div class="label">
            <div>背景配色</div>
            <div class="faint">
              内置的简约渐变背景，浅色 / 深色版本跟随主题自动切换；强调色也会随之微调。
              <template v-if="s.backgroundImage">当前使用自定义背景图，清空后才会显示内置背景。</template>
            </div>
          </div>
          <div class="swatches" role="radiogroup" aria-label="背景配色">
            <button
              v-for="p in PALETTES"
              :key="p.value"
              class="swatch glass-lite"
              :class="{ active: s.backgroundPalette === p.value }"
              role="radio"
              :aria-checked="s.backgroundPalette === p.value"
              @click="set('backgroundPalette', p.value)"
            >
              <span class="sw-prev">
                <AmbientBackground :palette="p.value" :animated="false" />
              </span>
              <span class="sw-text">
                <span class="sw-name">{{ p.label }}</span>
                <span class="faint sw-desc">{{ p.desc }}</span>
              </span>
              <span class="sw-check"><Icon v-if="s.backgroundPalette === p.value" name="check" :size="13" :stroke="2.6" /></span>
            </button>
          </div>
        </div>
        <div class="item">
          <div class="label">
            <div>背景动画</div>
            <div class="faint">内置背景轻微、缓慢地流动（一个周期约 50 秒，只在首页播放）；系统开启“减少动画”时自动停止</div>
          </div>
          <GlassSwitch :model-value="s.backgroundAnimation" @update:model-value="set('backgroundAnimation', $event)" />
        </div>
        <div class="item">
          <div class="label">
            <div>界面动画</div>
            <div class="faint">页面切换、列表入场、启动按钮、弹窗弹出等动效。关闭后只保留加载指示；系统开启“减少动画”时也会自动关闭</div>
          </div>
          <GlassSwitch :model-value="s.uiAnimation ?? true" @update:model-value="set('uiAnimation', $event)" />
        </div>
        <div class="field">
          <label>自定义背景图（本地图片路径，优先于内置背景；留空使用内置背景）</label>
          <div class="row">
            <input v-model="bgPath" class="input" placeholder="D:\Pictures\wallpaper.jpg" @keydown.enter="set('backgroundImage', bgPath.trim() || null)" />
            <button class="btn sm" @click="set('backgroundImage', bgPath.trim() || null)">应用</button>
            <button class="btn sm" :disabled="!s.backgroundImage" @click="bgPath = ''; set('backgroundImage', null)">使用内置背景</button>
          </div>
        </div>
      </div>

      <!-- 清理 -->
      <div class="glass card group">
        <h3 class="section-title"><Icon name="sparkle" /> 清理</h3>
        <div class="item">
          <div class="label">
            <div>清理游戏垃圾</div>
            <div class="faint">缓存、日志、崩溃报告和不再使用的旧组件。存档、模组、配置、资源包、光影包和截图不会被删除</div>
          </div>
          <button class="btn sm" :disabled="cleanScanning || cleaning" @click="scanClean">
            <span v-if="cleanScanning" class="spinner" />
            <Icon v-else name="search" :size="15" />
            {{ cleanScan ? '重新扫描' : '扫描' }}
          </button>
        </div>
        <Transition name="expand">
          <div v-if="cleanScan" class="clean">
            <div
              v-for="(c, i) in cleanScan.categories"
              :key="c.id"
              class="cl-row glass-lite"
              :class="{ on: picked[c.id] && c.files > 0, none: c.files === 0 }"
              :style="{ '--i': i }"
            >
              <GlassCheck v-model="picked[c.id]" :disabled="c.files === 0" />
              <div class="cl-text" @click="togglePick(c.id, c.files)">
                <div class="cl-name">
                  {{ c.label }}
                  <span v-if="!c.defaultSelected" class="badge warning">谨慎</span>
                </div>
                <div class="faint cl-desc">{{ c.description }}</div>
                <div v-for="n in c.notes" :key="n" class="cl-note"><Icon name="info" :size="13" /> {{ n }}</div>
              </div>
              <div class="cl-size">
                <b class="mono"><AnimatedNumber :value="c.bytes" :format="formatBytes" /></b>
                <span class="faint">{{ c.files ? `${c.files} 个文件` : '没有可清理的' }}</span>
              </div>
              <button
                class="btn ghost sm icon cl-more"
                :class="{ flip: openDetail === c.id }"
                :disabled="!c.details.length"
                title="查看明细"
                @click="toggleDetail(c.id)"
              >
                <Icon name="chevronDown" :size="16" />
              </button>
              <Transition name="expand">
                <div v-if="openDetail === c.id && c.details.length" class="cl-details">
                  <div v-for="(d, k) in c.details" :key="`${k}-${d.label}`" class="cl-d">
                    <span class="ellipsis" :title="d.label">{{ d.label }}</span>
                    <span class="faint mono">{{ d.files }} 个 · {{ formatBytes(d.bytes) }}</span>
                  </div>
                </div>
              </Transition>
            </div>
            <div class="cl-foot">
              <span v-if="cleanScan.busy" class="cl-warn"><Icon name="alert" :size="14" /> 有下载或安装任务正在进行，结束后才能清理</span>
              <Transition name="swap" mode="out-in">
                <span v-if="lastClean && !cleanScan.busy" :key="lastClean.freedBytes" class="freed">
                  <span class="freed-ico"><Icon name="check" :size="13" :stroke="2.8" /></span>
                  上次释放了 <b class="mono"><AnimatedNumber :value="lastClean.freedBytes" :format="formatBytes" :duration="800" /></b>
                  <template v-if="lastClean.failedFiles">，{{ lastClean.failedFiles }} 个文件正在使用已跳过</template>
                </span>
              </Transition>
              <span class="spacer" />
              <span class="faint cl-sum">已选 <b class="mono"><AnimatedNumber :value="pickedBytes" :format="formatBytes" /></b></span>
              <button
                class="btn primary sm"
                :disabled="cleaning || cleanScanning || cleanScan.busy || !pickedIds.length"
                @click="requestClean"
              >
                <span v-if="cleaning" class="spinner" />
                <Icon v-else name="trash" :size="15" />
                清理
              </button>
            </div>
          </div>
        </Transition>
      </div>

      <!-- 关于 -->
      <div class="glass card group">
        <h3 class="section-title"><Icon name="info" /> 关于</h3>
        <div class="about">
          <div><span class="faint">版本</span> PilotLauncher v{{ sys?.version }}</div>
          <div class="row">
            <span class="faint">数据目录</span>
            <span class="mono ellipsis sel">{{ sys?.dataDir }}</span>
            <button class="btn ghost sm icon" @click="open(sys?.dataDir)"><Icon name="folder" :size="15" /></button>
          </div>
          <div class="row">
            <span class="faint">游戏目录</span>
            <span class="mono ellipsis sel">{{ sys?.gameDir }}</span>
            <button class="btn ghost sm icon" @click="open(sys?.gameDir)"><Icon name="folder" :size="15" /></button>
          </div>
          <div class="row">
            <span class="faint">CurseForge Key</span>
            <span :class="sys?.curseforgeConfigured ? 'badge success' : 'badge'">
              {{ sys?.curseforgeConfigured ? `已从 ${sys?.localConfigPath} 读取` : '未配置：只能使用 Modrinth（见 config.local.example.json）' }}
            </span>
          </div>
          <p class="faint credit">
            设计思路参考 PCL（Plain Craft Launcher）、HMCL 与 LauncherX；下载镜像由 BMCLAPI 提供，社区资源镜像由 MCIM 提供；外置登录基于 authlib-injector。
            联机功能由 Terracotta | 陶瓦联机（© burningtnt 及贡献者，AGPL-3.0-or-later）提供，基于 EasyTier；本启动器只通过其 HTTP 接口与未修改的程序交互。
            本项目与 Mojang / Microsoft 无关。
          </p>
          <div class="row links">
            <button class="btn sm" @click="api.openUrl('https://github.com/Meloong-Git/PCL')"><Icon name="globe" :size="15" /> PCL</button>
            <button class="btn sm" @click="api.openUrl('https://bmclapidoc.bangbang93.com/')"><Icon name="globe" :size="15" /> BMCLAPI</button>
            <button class="btn sm" @click="api.openUrl('https://github.com/yushijinhun/authlib-injector')"><Icon name="globe" :size="15" /> authlib-injector</button>
            <button class="btn sm" @click="api.openUrl('https://www.mcimirror.top')"><Icon name="globe" :size="15" /> MCIM</button>
            <button class="btn sm" @click="api.openUrl('https://github.com/burningtnt/Terracotta')"><Icon name="network" :size="15" /> Terracotta | 陶瓦联机</button>
            <button class="btn sm" @click="api.openUrl('https://github.com/EasyTier/EasyTier')"><Icon name="globe" :size="15" /> EasyTier</button>
          </div>
        </div>
      </div>
    </div>

    <GlassDialog :open="confirmComponents" title="清理旧组件" :width="480" @close="confirmComponents = false">
      <p class="muted confirm-text">
        下面这些组件目前没有实例在用，删除后如果以后需要，启动游戏或打开联机时会重新下载：
      </p>
      <div class="confirm-list">
        <div v-for="(d, k) in componentsDetails" :key="`${k}-${d.label}`" class="cl-d">
          <span class="ellipsis">{{ d.label }}</span>
          <span class="faint mono">{{ formatBytes(d.bytes) }}</span>
        </div>
      </div>
      <template #footer>
        <button class="btn" @click="confirmComponents = false">取消</button>
        <button class="btn danger" :disabled="cleaning" @click="doClean">清理</button>
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
.page-head h2 {
  font-size: 24px;
}
.scroll {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 2px 4px 26px 0;
}
.group {
  --radius: 24px;
  --blur: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  flex: none;
}
.item {
  display: flex;
  align-items: center;
  gap: 18px;
  justify-content: space-between;
}
.item.column {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
}
.label {
  min-width: 0;
  flex: 1;
}
.legacy {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 12px;
  font-size: 12.5px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent);
}
.label > div:first-child {
  font-weight: 600;
}
.label .faint {
  font-size: 12px;
  margin-top: 2px;
  line-height: 1.5;
}
.slider-row {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 340px;
  flex: none;
}
.val {
  width: 64px;
  text-align: right;
}
.full {
  width: 100%;
}
.javas {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 220px;
  overflow: auto;
}
.java {
  --radius: 13px;
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: none;
  cursor: pointer;
  text-align: left;
  font-size: 12.5px;
}
.java.active {
  box-shadow: inset 0 0 0 1.5px var(--accent);
}
.jp {
  font-size: 12px;
}
.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.about {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.about .faint {
  min-width: 96px;
}
.sel {
  user-select: text;
  flex: 1;
}
.credit {
  line-height: 1.7;
  margin: 4px 0 0;
}
.links {
  flex-wrap: wrap;
  gap: 8px;
}

/* 背景配色色卡 */
.swatches {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  max-width: 560px;
}
.swatch {
  --radius: 16px;
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 6px 6px 9px;
  border: none;
  cursor: pointer;
  text-align: left;
  color: var(--text);
  transition: transform 0.45s var(--spring), box-shadow 0.2s ease, background 0.2s ease;
}
.swatch:hover {
  transform: translateY(-2px);
}
.swatch:active {
  transform: scale(0.97);
  transition-duration: 0.12s;
}
.swatch.active {
  box-shadow: inset 0 0 0 1.5px var(--accent), 0 8px 20px -12px var(--accent), inset 0 1px 0 var(--hl-top);
}
.sw-prev {
  position: relative;
  display: block;
  height: 64px;
  border-radius: 11px;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px var(--divider);
  isolation: isolate;
}
.sw-text {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 0 4px;
  padding-right: 26px;
}
.sw-name {
  font-weight: 650;
  font-size: 13px;
}
.sw-desc {
  font-size: 11.5px;
}
.sw-check {
  position: absolute;
  right: 9px;
  bottom: 8px;
  width: 18px;
  height: 18px;
  border-radius: 6px;
  display: grid;
  place-items: center;
  color: #fff;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  transition: background 0.2s ease;
}
.swatch.active .sw-check {
  background: var(--accent);
}
/* 清理 */
.clean {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.cl-row {
  --radius: 15px;
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  align-items: center;
  gap: 4px 12px;
  padding: 10px 8px 10px 14px;
  transition: box-shadow 0.2s ease, background 0.2s ease, opacity 0.2s ease;
  animation: rise-in 0.36s var(--ease-out) both;
  animation-delay: calc(var(--i, 0) * 40ms);
}
.cl-row.on {
  box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 70%, transparent), inset 0 1px 0 var(--hl-top);
}
.cl-row.none {
  opacity: 0.6;
}
.cl-text {
  min-width: 0;
  cursor: pointer;
}
.cl-row.none .cl-text {
  cursor: default;
}
.cl-name {
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}
.cl-desc {
  font-size: 12px;
  margin-top: 2px;
  line-height: 1.5;
}
.cl-note {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  margin-top: 4px;
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
}
.cl-size {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  font-size: 12px;
  white-space: nowrap;
}
.cl-size b {
  font-size: 14px;
  color: var(--text);
}
.cl-more :deep(.icon) {
  transition: transform 0.45s var(--spring);
}
.cl-more.flip :deep(.icon) {
  transform: rotate(180deg);
}
.cl-details {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 6px 6px 2px 30px;
  padding-top: 8px;
  border-top: 1px solid var(--divider);
}
.cl-d {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 12px;
  min-width: 0;
}
.cl-foot {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 2px 2px 0;
  font-size: 12.5px;
}
.cl-warn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
}
.freed {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.freed b {
  color: var(--success);
}
.freed-ico {
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  color: #fff;
  background: var(--success);
  box-shadow: 0 3px 10px -3px var(--success);
  animation: pop-in 0.55s var(--spring) both;
}
.cl-sum b {
  color: var(--text);
}
.confirm-text {
  margin: 0 0 10px;
  font-size: 13px;
  line-height: 1.6;
}
.confirm-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 40vh;
  overflow: auto;
}
</style>
