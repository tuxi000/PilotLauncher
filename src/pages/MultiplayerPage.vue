<script setup lang="ts">
// 联机页：内置 Terracotta | 陶瓦联机。
// 本启动器只通过陶瓦提供的本机 HTTP 接口与未经修改的官方程序交互（AGPL-3.0-or-later 例外条款），
// 页面底部固定展示其版权与许可信息。
import { computed, onActivated, onDeactivated, ref, watch } from 'vue'
import Icon from '../components/Icon.vue'
import GlassSelect from '../components/GlassSelect.vue'
import TaskCard from '../components/TaskCard.vue'
import Avatar from '../components/Avatar.vue'
import { api, errMsg, formatBytes, LOADER_LABEL, type TerracottaView } from '../api'
import type { SelectOption } from '../selectOption'
import { resolveIcon } from '../instanceIcons'
import {
  store,
  currentInstance,
  currentAccount,
  taskList,
  launchInstance,
  gameState,
  openLog,
  toast,
  toastError,
} from '../store'
import {
  DIFFICULTY_TEXT,
  EXCEPTION_TEXT,
  PROFILE_KIND_LABEL,
  ROOM_STATE_LABEL,
  isGuestPhase,
  isHostPhase,
  normalizeRoomCode,
} from '../terracotta'

const t = computed(() => store.terracotta)
const room = computed(() => t.value?.room ?? null)
const phase = computed(() => room.value?.state ?? 'waiting')
const isHost = computed(() => isHostPhase(phase.value))
const isGuest = computed(() => isGuestPhase(phase.value))
const running = computed(() => t.value?.process === 'running')
const starting = computed(() => t.value?.process === 'starting')

/** 当前正在执行的操作（避免重复点击） */
const busy = ref<string | null>(null)

const installTask = computed(() => taskList.value.find((x) => x.kind === 'terracotta') ?? null)
const showInstallTask = computed(
  () => !!installTask.value && (installTask.value.status === 'running' || (!t.value?.installed && installTask.value.status === 'failed')),
)

const coreBadge = computed(() => {
  const v = t.value
  if (!v) return { text: '正在读取…', cls: '' }
  if (v.installing) return { text: '正在安装联机组件', cls: 'accent' }
  if (!v.installed) return { text: '未安装联机组件', cls: '' }
  switch (v.process) {
    case 'running':
      return { text: `联机组件已就绪${v.coreVersion ? ` · ${v.coreVersion}` : ''}`, cls: 'success' }
    case 'starting':
      return { text: '正在启动联机组件', cls: 'accent' }
    case 'crashed':
      return { text: '联机组件已停止', cls: 'danger' }
    default:
      return { text: '联机组件未启动', cls: '' }
  }
})

// ───── 实例选择（房主启动游戏 / 房客启动游戏共用） ─────
const instName = ref('')
watch(
  () => [store.instances.map((i) => i.name).join('|'), currentInstance.value?.name ?? ''] as const,
  () => {
    if (!store.instances.some((i) => i.name === instName.value)) {
      instName.value = currentInstance.value?.name ?? store.instances[0]?.name ?? ''
    }
  },
  { immediate: true },
)
const instanceOptions = computed<SelectOption<string>[]>(() =>
  store.instances.map((i) => ({
    value: i.name,
    label: i.name,
    sub: `${i.mcVersion}${i.loader ? ` · ${LOADER_LABEL[i.loader.kind] ?? i.loader.kind} ${i.loader.version}` : ' · 原版'}`,
    icon: resolveIcon(i.icon, i.loader?.kind, i.versionType),
  })),
)
const game = computed(() => gameState(instName.value))
const gameRunning = computed(() => game.value?.state === 'running')
const gamePreparing = computed(() => game.value?.state === 'preparing')

function launchGame() {
  if (!instName.value) {
    store.page = 'download'
    store.downloadTab = 'versions'
    return
  }
  if (!currentAccount.value) {
    store.accountDialog = true
    return
  }
  if (gameRunning.value || gamePreparing.value) {
    openLog(instName.value)
    return
  }
  launchInstance(instName.value)
}

// ───── 操作 ─────
async function run(name: string, fn: () => Promise<TerracottaView | string | void>) {
  if (busy.value) return
  busy.value = name
  try {
    const r = await fn()
    if (r && typeof r === 'object') store.terracotta = r
  } catch (e) {
    toastError(e)
  } finally {
    busy.value = null
  }
}

const install = () => run('install', () => api.terracottaInstall())
const startCore = () => run('start', () => api.terracottaStart())
const restartCore = () => run('restart', () => api.terracottaRestart())
const stopCore = () => run('stop', () => api.terracottaStop())
const leave = () => run('leave', () => api.terracottaLeave())

/** 最近一次操作的身份，异常后“再试一次”用 */
const lastRole = ref<'host' | 'guest' | null>(null)
function host() {
  lastRole.value = 'host'
  run('host', () => api.terracottaHost())
}

const code = ref('')
const normalized = computed(() => normalizeRoomCode(code.value))
const codeError = computed(() =>
  code.value.trim() && !normalized.value ? '邀请码格式不正确，应为 U/XXXX-XXXX-XXXX-XXXX（可以直接粘贴整段消息）' : '',
)
const lastCode = ref<string | null>(null)
function join(c: string | null = normalized.value) {
  if (!c) return
  const target: string = c
  lastRole.value = 'guest'
  lastCode.value = target
  run('join', () => api.terracottaJoin(target))
}
async function pasteCode() {
  try {
    const text = await navigator.clipboard.readText()
    if (text) code.value = normalizeRoomCode(text) ?? text.trim()
  } catch {
    toast('无法读取剪贴板，请按 Ctrl+V 粘贴', 'error')
  }
}

function retry() {
  if (lastRole.value === 'guest' && lastCode.value) join(lastCode.value)
  else if (lastRole.value === 'host') host()
  else leave()
}

async function copy(text: string | null | undefined, what: string) {
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    toast(`已复制${what}`, 'success')
  } catch {
    toast('复制失败，请手动选择文字复制', 'error')
  }
}

async function copyLog() {
  try {
    const log = await api.terracottaLog()
    await navigator.clipboard.writeText(log)
    toast('已复制联机组件日志（最后 400 行）', 'success')
  } catch (e) {
    toast(`获取日志失败：${errMsg(e)}`, 'error')
  }
}

// 房间建好后自动复制一次邀请码
let autoCopied = ''
watch(
  () => (phase.value === 'host-ok' ? room.value?.room ?? '' : ''),
  (c) => {
    if (!c || c === autoCopied) return
    autoCopied = c
    navigator.clipboard
      .writeText(c)
      .then(() => toast('房间已开启，邀请码已复制到剪贴板', 'success'))
      .catch(() => toast('房间已开启', 'success'))
  },
)

const exceptionText = computed(() => {
  const n = room.value?.exceptionType
  return n === null || n === undefined ? '联机组件报告了一个异常' : (EXCEPTION_TEXT[n] ?? `联机组件报告了异常（类型 ${n}）`)
})
const difficulty = computed(() => DIFFICULTY_TEXT[room.value?.difficulty ?? 'UNKNOWN'] ?? DIFFICULTY_TEXT.UNKNOWN)
const profiles = computed(() => room.value?.profiles ?? [])
const showPlayers = computed(() => (phase.value === 'host-ok' || phase.value === 'guest-ok') && profiles.value.length > 0)

// 只在联机页打开时让后端高频轮询；打开时若已安装且未启动则自动启动联机组件
onActivated(async () => {
  try {
    const v = await api.terracottaWatch(true)
    store.terracotta = v
    if (v.installed && !v.installing && v.process === 'stopped') startCore()
  } catch (e) {
    toastError(e)
  }
})
onDeactivated(() => {
  api.terracottaWatch(false).catch(() => {})
})

const openUrl = (url: string | undefined) => {
  if (url) api.openUrl(url).catch(toastError)
}
</script>

<template>
  <section class="page">
    <header class="page-head">
      <h2>联机</h2>
      <span class="badge" :class="coreBadge.cls">{{ coreBadge.text }}</span>
      <span class="spacer" />
      <template v-if="t?.installed">
        <button v-if="t.process === 'stopped' || starting" class="btn sm" :disabled="!!busy || starting" @click="startCore">
          <span v-if="starting || busy === 'start'" class="spinner" />
          <Icon v-else name="play" :size="14" /> {{ starting ? '正在启动…' : '启动联机组件' }}
        </button>
        <button v-if="running" class="btn sm ghost" title="复制联机组件日志，用于反馈问题" @click="copyLog">
          <Icon name="log" :size="15" /> 复制日志
        </button>
        <button
          v-if="running || t.process === 'crashed'"
          class="btn sm ghost"
          :disabled="!!busy"
          title="重启联机组件"
          @click="restartCore"
        >
          <span v-if="busy === 'restart'" class="spinner" />
          <Icon v-else name="refresh" :size="15" /> 重启
        </button>
        <button v-if="running && !t.external" class="btn sm ghost" :disabled="!!busy" title="停止联机组件" @click="stopCore">
          <Icon name="stop" :size="14" /> 停止
        </button>
      </template>
    </header>

    <div class="scroll">
      <!-- 未安装：说明 + 安装按钮 -->
      <div v-if="t && !t.installed" class="glass card intro" v-light>
        <div class="intro-ico"><Icon name="network" :size="34" /></div>
        <div class="intro-text">
          <h3>和朋友一起玩，不需要公网 IP</h3>
          <p class="muted">
            联机功能使用 Terracotta | 陶瓦联机（基于 EasyTier 的 P2P 组网）。房主把自己“对局域网开放”的世界分享出去，
            朋友输入邀请码就能加入。无需管理员权限，也不会安装虚拟网卡。首次使用需要下载联机组件。
          </p>
          <div class="facts">
            <span class="badge">版本 {{ t.version }}</span>
            <span class="badge">约 {{ formatBytes(t.packageSize) }}</span>
            <span class="badge" title="跟随 设置 → 下载源">来源：{{ t.sources.join(' → ') }}</span>
            <span class="badge success">SHA-512 校验</span>
          </div>
        </div>
        <button class="btn primary" :disabled="t.installing || !!busy" @click="install">
          <span v-if="t.installing || busy === 'install'" class="spinner" />
          <Icon v-else name="download" :size="16" />
          {{ t.installing ? '正在安装…' : '安装联机组件' }}
        </button>
      </div>
      <TaskCard v-if="showInstallTask && installTask" :task="installTask" />

      <template v-if="t?.installed">
        <!-- 联机组件异常退出 -->
        <div v-if="t.process === 'crashed'" class="glass card alert-card">
          <Icon name="alert" :size="20" />
          <div class="grow">
            <div class="strong">联机组件已停止运行</div>
            <div class="faint small pre">{{ t.error }}</div>
          </div>
          <button class="btn sm primary" :disabled="!!busy" @click="restartCore">
            <span v-if="busy === 'restart'" class="spinner" /> 重启联机组件
          </button>
          <button class="btn sm" :disabled="!!busy || t.installing" @click="install">重新安装</button>
        </div>

        <div v-if="t.external && running" class="note glass-lite">
          <Icon name="info" :size="15" />
          <span>
            正在共用其他程序（如 HMCL / PCL CE）启动的陶瓦联机{{ t.coreVersion ? ` ${t.coreVersion}` : '' }}；
            一台电脑同时只运行一个陶瓦，退出 PilotLauncher 时不会关闭它。
          </span>
        </div>

        <!-- 房间异常 -->
        <div v-if="phase === 'exception'" class="glass card alert-card">
          <Icon name="alert" :size="20" />
          <div class="grow">
            <div class="strong">{{ exceptionText }}</div>
            <div class="faint small">可以再试一次；多次失败时可尝试更换网络，或换另一位玩家当房主。</div>
          </div>
          <button class="btn sm primary" :disabled="!!busy" @click="retry">再试一次</button>
          <button class="btn sm" :disabled="!!busy" @click="leave">返回</button>
        </div>

        <div class="cards">
          <!-- ───── 创建房间（房主） ───── -->
          <div class="glass card role" :class="{ active: isHost, dim: isGuest }" v-light>
            <div class="role-head">
              <span class="role-ico host"><Icon name="home" :size="20" /></span>
              <div class="grow">
                <h3>创建房间</h3>
                <div class="faint small">我当房主：把自己的局域网世界分享给朋友</div>
              </div>
              <span v-if="isHost" class="badge" :class="phase === 'host-ok' ? 'success' : 'accent'">
                {{ ROOM_STATE_LABEL[phase] ?? phase }}
              </span>
            </div>

            <!-- 房间已开启：大号邀请码 -->
            <template v-if="phase === 'host-ok'">
              <div class="code-box">
                <div class="faint small">邀请码</div>
                <div class="code mono sel">{{ room?.room }}</div>
                <div class="row wrap">
                  <button class="btn primary" @click="copy(room?.room, '邀请码')">
                    <Icon name="copy" :size="16" /> 复制邀请码
                  </button>
                  <button class="btn danger sm" :disabled="!!busy" @click="leave">
                    <span v-if="busy === 'leave'" class="spinner" /> 关闭房间
                  </button>
                </div>
              </div>
              <p class="faint small hint">
                把邀请码发给朋友，让他们在 PilotLauncher、HMCL 或 PCL CE 的联机功能里选择“加入房间 / 房客”并输入。
                请保持游戏世界和启动器开着；退出世界或关闭房间后，邀请码立即失效。
              </p>
            </template>

            <!-- 正在等待局域网世界 / 正在创建房间 -->
            <template v-else-if="isHost">
              <div class="wait">
                <span class="spinner big" />
                <div>
                  <div class="strong">
                    {{ phase === 'host-starting' ? '已找到局域网世界，正在创建房间…' : '正在等待你开放局域网世界…' }}
                  </div>
                  <div class="faint small">进入单人世界后按 <kbd>Esc</kbd> → 对局域网开放 → 创建局域网世界，启动器会自动检测到</div>
                </div>
              </div>
              <div class="row wrap">
                <GlassSelect
                  v-model="instName"
                  class="inst"
                  :options="instanceOptions"
                  size="sm"
                  :min-width="280"
                  placeholder="（还没有实例）"
                  empty-text="还没有实例"
                />
                <button class="btn sm" :disabled="!instName" @click="launchGame">
                  <span v-if="gamePreparing" class="spinner" />
                  <Icon v-else :name="gameRunning ? 'log' : 'play'" :size="14" />
                  {{ gameRunning ? '游戏运行中' : gamePreparing ? '正在启动…' : '启动游戏' }}
                </button>
                <span class="spacer" />
                <button class="btn sm" :disabled="!!busy" @click="leave">取消</button>
              </div>
            </template>

            <!-- 空闲：步骤 + 选择实例 -->
            <template v-else>
              <ol class="steps">
                <li>选择实例并<b>启动游戏</b>，进入一个单人世界</li>
                <li>按 <kbd>Esc</kbd> → <b>对局域网开放</b> → <b>创建局域网世界</b></li>
                <li>点击<b>创建房间</b>，检测到局域网世界后会生成邀请码</li>
              </ol>
              <div class="field">
                <label>实例</label>
                <GlassSelect
                  v-model="instName"
                  :options="instanceOptions"
                  :disabled="isGuest"
                  placeholder="（还没有实例）"
                  empty-text="还没有实例"
                />
              </div>
              <div class="row">
                <button class="btn" :disabled="isGuest" @click="launchGame">
                  <span v-if="gamePreparing" class="spinner" />
                  <Icon v-else :name="!instName ? 'download' : gameRunning ? 'log' : 'play'" :size="15" />
                  {{ !instName ? '先去安装游戏' : gameRunning ? '游戏运行中' : gamePreparing ? '正在启动…' : '启动游戏' }}
                </button>
                <span class="spacer" />
                <button class="btn primary" :disabled="isGuest || !!busy || starting" @click="host">
                  <span v-if="busy === 'host'" class="spinner" />
                  <Icon v-else name="network" :size="16" />
                  创建房间
                </button>
              </div>
            </template>
          </div>

          <!-- ───── 加入房间（房客） ───── -->
          <div class="glass card role" :class="{ active: isGuest, dim: isHost }" v-light>
            <div class="role-head">
              <span class="role-ico guest"><Icon name="link" :size="20" /></span>
              <div class="grow">
                <h3>加入房间</h3>
                <div class="faint small">我当房客：输入房主发来的邀请码</div>
              </div>
              <span v-if="isGuest" class="badge" :class="phase === 'guest-ok' ? 'success' : 'accent'">
                {{ ROOM_STATE_LABEL[phase] ?? phase }}
              </span>
            </div>

            <!-- 已加入 -->
            <template v-if="phase === 'guest-ok'">
              <div class="ok-box">
                <Icon name="check" :size="22" :stroke="2.4" />
                <div>
                  <div class="strong">已加入房间</div>
                  <div class="small muted">
                    启动游戏 → <b>多人游戏</b>，局域网列表里会出现“陶瓦联机大厅”，双击进入即可（请保持启动器运行）。
                  </div>
                </div>
              </div>
              <div v-if="room?.url" class="addr">
                <span class="faint small">备用地址（添加服务器 / 直接连接）</span>
                <span class="mono sel">{{ room.url }}</span>
                <button class="btn ghost sm icon" title="复制地址" @click="copy(room?.url, '服务器地址')">
                  <Icon name="copy" :size="15" />
                </button>
              </div>
              <div class="row wrap">
                <GlassSelect
                  v-model="instName"
                  class="inst"
                  :options="instanceOptions"
                  size="sm"
                  :min-width="280"
                  placeholder="（还没有实例）"
                  empty-text="还没有实例"
                />
                <button class="btn sm" :disabled="!instName" @click="launchGame">
                  <span v-if="gamePreparing" class="spinner" />
                  <Icon v-else :name="gameRunning ? 'log' : 'play'" :size="14" />
                  {{ gameRunning ? '游戏运行中' : gamePreparing ? '正在启动…' : '启动游戏' }}
                </button>
                <span class="spacer" />
                <button class="btn danger sm" :disabled="!!busy" @click="leave">
                  <span v-if="busy === 'leave'" class="spinner" /> 退出房间
                </button>
              </div>
            </template>

            <!-- 连接中 -->
            <template v-else-if="isGuest">
              <div class="wait">
                <span class="spinner big" />
                <div>
                  <div class="strong">
                    {{ phase === 'guest-starting' ? '正在与房主建立连接…' : '正在连接房间…' }}
                  </div>
                  <div class="faint small mono">{{ room?.room }}</div>
                </div>
              </div>
              <div v-if="phase === 'guest-starting'" class="diff">
                <span class="badge" :class="difficulty.tone">{{ difficulty.text }}</span>
                <span class="faint small">根据双方 NAT 类型推算，仅供参考</span>
              </div>
              <div class="row">
                <span class="spacer" />
                <button class="btn sm" :disabled="!!busy" @click="leave">取消</button>
              </div>
            </template>

            <!-- 空闲：输入邀请码 -->
            <template v-else>
              <div class="field">
                <label>邀请码</label>
                <div class="row">
                  <input
                    v-model="code"
                    class="input mono code-input"
                    :class="{ invalid: !!codeError }"
                    placeholder="U/XXXX-XXXX-XXXX-XXXX"
                    spellcheck="false"
                    :disabled="isHost"
                    @keydown.enter="join()"
                  />
                  <button class="btn sm" :disabled="isHost" title="从剪贴板粘贴" @click="pasteCode">粘贴</button>
                </div>
                <div v-if="codeError" class="err small">{{ codeError }}</div>
                <div v-else-if="normalized && normalized !== code.trim()" class="faint small">
                  将使用 <span class="mono">{{ normalized }}</span>
                </div>
              </div>
              <p class="faint small hint">
                加入成功后打开游戏 → 多人游戏，房间会出现在局域网列表里。双方的游戏版本和模组需要一致。
              </p>
              <div class="row">
                <span class="spacer" />
                <button class="btn primary" :disabled="isHost || !normalized || !!busy || starting" @click="join()">
                  <span v-if="busy === 'join'" class="spinner" />
                  <Icon v-else name="link" :size="16" />
                  加入
                </button>
              </div>
            </template>
          </div>
        </div>

        <!-- 玩家列表（陶瓦只提供名称与身份，没有延迟信息） -->
        <div v-if="showPlayers" class="glass card players">
          <h3 class="section-title"><Icon name="users" /> 房间成员 · {{ profiles.length }}</h3>
          <div class="plist">
            <div v-for="p in profiles" :key="p.machineId" class="player glass-lite">
              <Avatar :name="p.name || '?'" :seed="p.machineId" :size="30" />
              <span class="ellipsis pname">{{ p.name || '匿名玩家' }}</span>
              <span class="badge" :class="p.kind === 'HOST' ? 'accent' : p.kind === 'LOCAL' ? 'success' : ''">
                {{ PROFILE_KIND_LABEL[p.kind] ?? p.kind }}
              </span>
              <span class="faint tiny ellipsis vendor" :title="p.vendor">{{ p.vendor }}</span>
            </div>
          </div>
        </div>

        <!-- 注意事项 -->
        <div class="glass card tips">
          <h3 class="section-title"><Icon name="info" /> 注意事项</h3>
          <ul>
            <li>
              <b>不要和房间里的人用同一个游戏名。</b>离线账户的 UUID 由用户名决定，同名玩家会互相挤下线或无法进入。
            </li>
            <li>房主需要一直开着游戏世界和启动器；房主退出世界后房间会自动关闭。双方的游戏版本、模组需要一致。</li>
            <li>
              房客通过局域网列表里的“陶瓦联机大厅”进入时会跳过正版验证；如果使用外置登录（皮肤站）账户进入时提示
              “无效的玩家档案公钥签名”，请改用离线账户再试。
            </li>
            <li>
              不需要管理员权限，也不会安装虚拟网卡。第一次联机时 Windows 防火墙可能询问是否允许联机组件（EasyTier）访问网络，请选择允许。
            </li>
            <li>连接成功率取决于双方的网络（NAT 类型）。连不上时可以换一个网络，或换另一位玩家当房主。</li>
            <li>一台电脑同时只运行一个陶瓦联机：HMCL / PCL CE 已经打开联机时，PilotLauncher 会直接共用它。</li>
            <li>请遵守所在国家或地区的法律法规。联机组件是第三方开源软件，问题可到其项目主页反馈。</li>
          </ul>
          <div v-if="t.installDir" class="faint tiny mono sel dir">组件位置：{{ t.installDir }}</div>
        </div>
      </template>
    </div>

    <!-- 版权与许可（AGPL-3.0-or-later 例外条款要求在界面明显处标注，固定在页面底部） -->
    <footer class="license glass glass-strong">
      <span class="lic-ico"><Icon name="network" :size="18" /></span>
      <div class="lic-text">
        联机功能由 <b>Terracotta | 陶瓦联机</b> 提供，© burningtnt 及贡献者，基于 <b>EasyTier</b>，以
        <b>AGPL-3.0-or-later</b> 许可发布；本启动器通过其 HTTP 接口与未修改的程序交互。
      </div>
      <div class="lic-links">
        <button class="btn sm ghost" @click="openUrl(t?.repoUrl ?? 'https://github.com/burningtnt/Terracotta')">
          <Icon name="external" :size="14" /> GitHub
        </button>
        <button class="btn sm ghost" @click="openUrl(t?.licenseUrl ?? 'https://github.com/burningtnt/Terracotta/blob/master/LICENSE')">
          许可证
        </button>
        <button class="btn sm ghost" @click="openUrl(t?.easytierUrl ?? 'https://github.com/EasyTier/EasyTier')">EasyTier</button>
      </div>
    </footer>
  </section>
</template>

<style scoped>
.page {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 22px 26px 14px;
  gap: 14px;
}
.page-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.page-head h2 {
  font-size: 24px;
  margin-right: 4px;
}
.scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 2px 4px 8px 0;
}
.scroll > * {
  flex: none;
}
.grow {
  flex: 1;
  min-width: 0;
}
.strong {
  font-weight: 650;
}
.small {
  font-size: 12.5px;
  line-height: 1.6;
}
.tiny {
  font-size: 11.5px;
}
.pre {
  white-space: pre-wrap;
  word-break: break-all;
}
.sel {
  user-select: text;
}
.wrap {
  flex-wrap: wrap;
}
kbd {
  display: inline-block;
  padding: 0 6px;
  border-radius: 6px;
  font-family: var(--mono);
  font-size: 11.5px;
  line-height: 18px;
  background: color-mix(in srgb, var(--text) 9%, transparent);
  box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--text) 18%, transparent);
}

/* 未安装 */
.intro {
  --radius: 26px;
  display: flex;
  align-items: center;
  gap: 18px;
}
.intro-ico {
  width: 64px;
  height: 64px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 22px;
  color: #fff;
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 70%, white), var(--accent));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6), 0 10px 22px -8px var(--accent);
}
.intro-text {
  flex: 1;
  min-width: 0;
}
.intro-text h3 {
  font-size: 17px;
}
.intro-text p {
  margin: 6px 0 10px;
  font-size: 13px;
  line-height: 1.7;
}
.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.alert-card {
  --radius: 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  color: var(--text);
  box-shadow: var(--glass-shadow), inset 0 0 0 1.5px color-mix(in srgb, var(--danger) 45%, transparent);
}
.alert-card > .icon {
  color: var(--danger);
}
.note {
  --radius: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 14px;
  font-size: 12.5px;
  color: var(--text-2);
}

/* 两张操作卡片 */
.cards {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}
.role {
  --radius: 26px;
  --blur: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 300px;
  transition: opacity 0.3s ease, transform 0.5s var(--spring), box-shadow 0.3s ease;
}
.role.active {
  box-shadow: var(--glass-shadow), inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 55%, transparent),
    0 14px 34px -18px var(--accent);
}
.role.dim {
  opacity: 0.55;
}
.role-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.role-head h3 {
  font-size: 16px;
}
.role-ico {
  width: 40px;
  height: 40px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 14px;
  color: #fff;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55), 0 8px 18px -8px var(--accent);
}
.role-ico.host {
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 65%, white), var(--accent));
}
.role-ico.guest {
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent-2) 65%, white), var(--accent-2));
}
.steps {
  margin: 0;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-2);
}
.steps b {
  color: var(--text);
  font-weight: 600;
}
.role .row {
  margin-top: auto;
}
.inst {
  flex: 1;
  min-width: 160px;
  max-width: 260px;
}
.hint {
  margin: 0;
}

/* 邀请码 */
.code-box {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
  padding: 16px 18px;
  border-radius: 20px;
  background: color-mix(in srgb, var(--accent) 10%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 28%, transparent);
}
.code {
  font-size: 27px;
  font-weight: 700;
  letter-spacing: 0.06em;
  color: var(--text);
  word-break: break-all;
}
.code-box .row {
  margin-top: 0;
}
.wait {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--text) 5%, transparent);
  color: var(--accent);
}
.wait > div {
  color: var(--text);
}
.spinner.big {
  width: 26px;
  height: 26px;
  border-width: 3px;
  flex: none;
}
.diff {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.ok-box {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 18px;
  color: var(--success);
  background: color-mix(in srgb, var(--success) 12%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--success) 30%, transparent);
}
.ok-box > div {
  color: var(--text);
}
.addr {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px 8px 14px;
  border-radius: 14px;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--field-border);
}
.addr .mono {
  flex: 1;
  font-size: 14px;
  font-weight: 600;
}
.code-input {
  font-size: 14px;
  letter-spacing: 0.04em;
}
.code-input.invalid {
  box-shadow: inset 0 0 0 1px var(--danger), 0 0 0 4px color-mix(in srgb, var(--danger) 16%, transparent);
}
.err {
  color: var(--danger);
}

/* 玩家列表 */
.players {
  --radius: 24px;
}
.players .section-title,
.tips .section-title {
  margin-bottom: 12px;
}
.plist {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 8px;
}
.player {
  --radius: 14px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  grid-template-rows: auto auto;
  column-gap: 10px;
  align-items: center;
  padding: 8px 12px;
}
.player :deep(.avatar),
.player > :first-child {
  grid-row: span 2;
}
.pname {
  font-weight: 600;
}
.vendor {
  grid-column: 2 / 4;
}

/* 注意事项 */
.tips {
  --radius: 24px;
}
.tips ul {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12.5px;
  line-height: 1.65;
  color: var(--text-2);
}
.tips b {
  color: var(--text);
}
.dir {
  margin-top: 10px;
  word-break: break-all;
}

/* 版权与许可：固定在页面底部 */
.license {
  --radius: 20px;
  --blur: 24px;
  flex: none;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px 10px 14px;
}
.lic-ico {
  width: 32px;
  height: 32px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 11px;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
.lic-text {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-2);
}
.lic-text b {
  color: var(--text);
  font-weight: 650;
}
.lic-links {
  display: flex;
  flex: none;
  gap: 2px;
}

@media (max-width: 1040px) {
  .cards {
    grid-template-columns: 1fr;
  }
  .role {
    min-height: 0;
  }
}
</style>
