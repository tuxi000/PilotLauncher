// 全局状态（不引入 Pinia，保持依赖精简）
import { computed, reactive, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  api,
  errMsg,
  EVENTS,
  type AccountView,
  type CrashInfo,
  type GameLogEvent,
  type GameStateEvent,
  type InstanceView,
  type MissingEvent,
  type ResourceKind,
  type Settings,
  type SystemInfo,
  type TaskSnapshot,
  type TerracottaView,
} from './api'
import { applyEffect, applyPalette, applyRefraction, applyTheme } from './theme'
import { applyMotion, recentPointer } from './motion'
import { loadSkins } from './skin'

export type Page = 'home' | 'download' | 'instances' | 'multiplayer' | 'settings'
export type DownloadTab = 'versions' | 'tasks' | ResourceKind

/** 本地导入请求：文件路径 + 期望的类型（拖放或文件选择） */
export interface ImportRequest {
  path: string
  hint: 'world' | 'modpack' | null
  /** 地图导入的默认目标实例 */
  instance?: string | null
}

export interface Toast {
  id: number
  text: string
  kind: 'info' | 'success' | 'error'
}

const LOG_LIMIT = 5000

export const store = reactive({
  ready: false,
  page: 'home' as Page,
  downloadTab: 'versions' as DownloadTab,
  /** 社区资源安装的目标实例（为空时使用当前实例） */
  communityTarget: null as string | null,
  /** 打开资源管理面板的实例 */
  detailInstance: null as string | null,
  importRequest: null as ImportRequest | null,
  /** 窗口上有文件拖动经过 */
  dragOver: false,
  /** CurseForge 整合包中需要手动下载的文件 */
  missing: null as MissingEvent | null,
  settings: null as Settings | null,
  sys: null as SystemInfo | null,
  instances: [] as InstanceView[],
  accounts: [] as AccountView[],
  tasks: {} as Record<string, TaskSnapshot>,
  games: {} as Record<string, GameStateEvent>,
  logs: {} as Record<string, string[]>,
  toasts: [] as Toast[],
  /** 账户面板（点侧边栏底部的头像弹出的玻璃面板，见 AccountPanel.vue） */
  accountPanel: false,
  /** 账户面板弹出的起点（屏幕坐标），面板从这里缩放展开 */
  accountPanelOrigin: null as { x: number; y: number } | null,
  logInstance: null as string | null,
  crash: null as { instance: string; info: CrashInfo } | null,
  /** 自定义背景图的 data URL；为空时显示内置的 AmbientBackground（配色见 settings.backgroundPalette） */
  backgroundUrl: null as string | null,
  /** 联机组件（Terracotta | 陶瓦联机）的状态，由 terracotta-state 事件更新 */
  terracotta: null as TerracottaView | null,
})

let toastSeq = 0
export function toast(text: string, kind: Toast['kind'] = 'info', ms = 3600) {
  const id = ++toastSeq
  store.toasts.push({ id, text, kind })
  window.setTimeout(() => {
    const i = store.toasts.findIndex((t) => t.id === id)
    if (i >= 0) store.toasts.splice(i, 1)
  }, ms)
}

export function toastError(e: unknown) {
  toast(errMsg(e), 'error', 6000)
}

export const currentInstance = computed<InstanceView | null>(() => {
  const name = store.settings?.selectedInstance
  return store.instances.find((i) => i.name === name) ?? null
})

export const currentAccount = computed<AccountView | null>(() => {
  const id = store.settings?.selectedAccount
  return store.accounts.find((a) => a.id === id) ?? null
})

export const taskList = computed(() =>
  Object.values(store.tasks).sort((a, b) => b.createdAt - a.createdAt),
)

/** 社区资源 / 本地资源发生变化的计数器（实例资源面板据此刷新） */
export const resourceVersion = ref(0)

export const runningTaskCount = computed(() => taskList.value.filter((t) => t.status === 'running').length)

export async function refreshInstances() {
  try {
    store.instances = await api.listInstances()
  } catch (e) {
    toastError(e)
  }
}

export async function refreshTerracotta() {
  try {
    store.terracotta = await api.terracottaStatus()
  } catch (e) {
    console.warn('获取联机状态失败', e)
  }
}

export async function refreshAccounts() {
  try {
    const list = await api.listAccounts()
    store.accounts = list.accounts
    if (store.settings) store.settings.selectedAccount = list.selected
    loadSkins(list.accounts)
  } catch (e) {
    toastError(e)
  }
}

/**
 * 打开账户面板。from：触发的元素（面板从它的中心弹出）；不传时用最近一次点击的位置
 */
export function openAccountPanel(from?: Element | null) {
  if (from) {
    const r = from.getBoundingClientRect()
    store.accountPanelOrigin = { x: r.left + r.width / 2, y: r.top + r.height / 2 }
  } else {
    store.accountPanelOrigin = recentPointer()
  }
  store.accountPanel = true
}

export function closeAccountPanel() {
  store.accountPanel = false
}

/** 切换当前账户 */
export async function selectAccount(id: string) {
  if (store.settings?.selectedAccount === id) return
  const before = store.settings?.selectedAccount ?? null
  // 先在界面上切过去，失败再回滚
  if (store.settings) store.settings.selectedAccount = id
  try {
    store.settings = await api.selectAccount(id)
  } catch (e) {
    if (store.settings) store.settings.selectedAccount = before
    toastError(e)
  }
}

export async function removeAccount(id: string) {
  try {
    const list = await api.removeAccount(id)
    store.accounts = list.accounts
    if (store.settings) store.settings.selectedAccount = list.selected
    loadSkins(list.accounts)
  } catch (e) {
    toastError(e)
  }
}

async function applyBackground(path: string | null) {
  if (!path) {
    store.backgroundUrl = null
    return
  }
  try {
    store.backgroundUrl = await api.readImage(path)
  } catch (e) {
    store.backgroundUrl = null
    toast(`背景图加载失败：${errMsg(e)}`, 'error')
  }
}

function applyAppearance(s: Settings) {
  applyTheme(s.theme)
  applyEffect(s.windowEffect)
  applyRefraction(s.glassRefraction)
  applyPalette(s.backgroundPalette)
  applyMotion(s.uiAnimation ?? true)
}

/** 合并保存设置（失败时回滚） */
export async function saveSettings(patch: Partial<Settings>) {
  if (!store.settings) return
  const before = { ...store.settings }
  const next = { ...store.settings, ...patch }
  store.settings = next
  applyAppearance(next)
  try {
    const saved = await api.saveSettings(next)
    store.settings = saved
    if (patch.backgroundImage !== undefined) await applyBackground(saved.backgroundImage)
    if (patch.gameDir !== undefined) {
      store.sys = await api.systemInfo()
      await refreshInstances()
    }
  } catch (e) {
    store.settings = before
    applyAppearance(before)
    toastError(e)
  }
}

export async function selectInstance(name: string) {
  try {
    store.settings = await api.selectInstance(name)
  } catch (e) {
    toastError(e)
  }
}

export function gameState(name: string | null | undefined): GameStateEvent | null {
  if (!name) return null
  return store.games[name] ?? null
}

export function isBusy(name: string | null | undefined): boolean {
  const s = gameState(name)
  return !!s && (s.state === 'preparing' || s.state === 'running')
}

export async function launchInstance(name: string) {
  if (isBusy(name)) return
  store.games[name] = {
    instance: name,
    state: 'preparing',
    message: '准备启动',
    pid: null,
    exitCode: null,
    killed: false,
    crash: null,
  }
  store.logs[name] = []
  try {
    await api.launch(name)
  } catch (e) {
    // 后端也会推送 error 事件，这里兜底
    store.games[name] = { ...store.games[name], state: 'error', message: errMsg(e) }
    toastError(e)
  }
}

export async function killInstance(name: string) {
  try {
    await api.kill(name)
  } catch (e) {
    toastError(e)
  }
}

export async function openLog(name: string) {
  store.logInstance = name
  try {
    const lines = await api.gameLog(name)
    if (lines.length >= (store.logs[name]?.length ?? 0)) store.logs[name] = lines
  } catch {
    /* 没有日志 */
  }
}

/** 用系统对话框选择本地 zip / mrpack */
export async function pickArchive(kind: 'world' | 'modpack'): Promise<string | null> {
  try {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      title: kind === 'world' ? '选择地图压缩包' : '选择整合包',
      filters:
        kind === 'world'
          ? [{ name: '地图压缩包', extensions: ['zip'] }]
          : [{ name: '整合包', extensions: ['mrpack', 'zip'] }],
    })
    if (!picked) return null
    return Array.isArray(picked) ? (picked[0] ?? null) : picked
  } catch (e) {
    toastError(e)
    return null
  }
}

export async function requestImport(kind: 'world' | 'modpack', instance?: string | null) {
  const path = await pickArchive(kind)
  if (path) store.importRequest = { path, hint: kind, instance: instance ?? null }
}

let initialized = false

export async function init() {
  if (initialized) return
  initialized = true

  await listen<TaskSnapshot>(EVENTS.task, (e) => {
    const prev = store.tasks[e.payload.id]
    store.tasks[e.payload.id] = e.payload
    if (prev?.status === 'running' && e.payload.status !== 'running') {
      const notable = ['install', 'repair', 'community', 'world'].includes(e.payload.kind)
      if (e.payload.status === 'success' && notable) {
        toast(`${e.payload.title} 完成`, 'success')
      } else if (e.payload.status === 'failed') {
        toast(`${e.payload.title} 失败：${e.payload.error ?? ''}`, 'error', 8000)
      }
      if (notable) {
        refreshInstances()
        resourceVersion.value++
      }
      if (e.payload.kind === 'terracotta') {
        if (e.payload.status === 'success') toast('联机组件安装完成', 'success')
        refreshTerracotta()
      }
    }
  })

  await listen<TerracottaView>(EVENTS.terracotta, (e) => {
    store.terracotta = e.payload
  })

  await listen<GameStateEvent>(EVENTS.gameState, (e) => {
    const ev = e.payload
    store.games[ev.instance] = ev
    if (ev.state === 'running') {
      api.gameLog(ev.instance).then((lines) => {
        // 后端缓冲区包含启动信息头与已产生的日志，以它为准
        if (lines.length >= (store.logs[ev.instance]?.length ?? 0)) store.logs[ev.instance] = lines
      }).catch(() => {})
      refreshInstances()
    } else if (ev.state === 'exited') {
      if (ev.crash) store.crash = { instance: ev.instance, info: ev.crash }
      else toast(ev.message ?? '游戏已退出')
      refreshInstances()
    } else if (ev.state === 'error') {
      toast(ev.message ?? '启动失败', 'error', 8000)
    }
  })

  await listen<MissingEvent>(EVENTS.missing, (e) => {
    store.missing = e.payload
  })

  // 拖放导入：把 .zip / .mrpack 拖进窗口
  try {
    await getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload
      if (p.type === 'enter' || p.type === 'over') {
        store.dragOver = true
      } else if (p.type === 'leave') {
        store.dragOver = false
      } else if (p.type === 'drop') {
        store.dragOver = false
        const file = p.paths.find((x) => /\.(zip|mrpack)$/i.test(x))
        if (!file) {
          if (p.paths.length) toast('只支持拖入 .zip（地图 / CurseForge 整合包）或 .mrpack 文件', 'error')
          return
        }
        const tab = store.page === 'download' ? store.downloadTab : null
        const hint = tab === 'world' ? 'world' : tab === 'modpack' ? 'modpack' : null
        store.importRequest = { path: file, hint, instance: store.detailInstance }
      }
    })
  } catch (e) {
    console.warn('拖放监听失败', e)
  }

  await listen<GameLogEvent>(EVENTS.gameLog, (e) => {
    const arr = store.logs[e.payload.instance] ?? (store.logs[e.payload.instance] = [])
    arr.push(...e.payload.lines)
    if (arr.length > LOG_LIMIT) arr.splice(0, arr.length - LOG_LIMIT)
  })

  try {
    const [settings, sys, tasks, running] = await Promise.all([
      api.getSettings(),
      api.systemInfo(),
      api.listTasks(),
      api.runningGames(),
    ])
    store.settings = settings
    store.sys = sys
    for (const t of tasks) store.tasks[t.id] = t
    for (const name of running) {
      store.games[name] = {
        instance: name,
        state: 'running',
        message: '游戏运行中',
        pid: null,
        exitCode: null,
        killed: false,
        crash: null,
      }
    }
    applyAppearance(settings)
    await Promise.all([
      refreshInstances(),
      refreshAccounts(),
      applyBackground(settings.backgroundImage),
      refreshTerracotta(),
    ])
  } catch (e) {
    toastError(e)
  } finally {
    store.ready = true
  }
}
