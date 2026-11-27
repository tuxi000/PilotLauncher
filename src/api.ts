// 后端命令与事件的类型定义及封装（Tauri 2：@tauri-apps/api/core 的 invoke）
import { invoke } from '@tauri-apps/api/core'

export type DownloadSource = 'official' | 'bmclapi' | 'official_first'
/** 社区资源（Modrinth / CurseForge）源策略，镜像为 MCIM */
export type CommunitySource = 'official_first' | 'mirror_first' | 'official'
export type ThemeMode = 'system' | 'light' | 'dark'
export type WindowEffect = 'mica' | 'acrylic' | 'tabbed' | 'none'
/** 内置背景配色：蓝紫 / 薄荷 / 珊瑚 */
export type BackgroundPalette = 'blue' | 'mint' | 'coral'

export interface Settings {
  downloadSource: DownloadSource
  communitySource: CommunitySource
  concurrency: number
  retries: number
  memoryMode: 'auto' | 'manual'
  memoryMb: number
  javaPath: string | null
  theme: ThemeMode
  windowEffect: WindowEffect
  glassRefraction: boolean
  gameDir: string | null
  selectedInstance: string | null
  selectedAccount: string | null
  jvmArgs: string
  gameArgs: string
  windowWidth: number
  windowHeight: number
  backgroundImage: string | null
  /** 未设置自定义背景图时内置背景的配色（默认 blue） */
  backgroundPalette: BackgroundPalette
  /** 内置背景是否缓慢流动（默认 true；系统开启“减少动画”时自动停止） */
  backgroundAnimation: boolean
  /** 界面动画（默认 true）：关闭后去掉页面切换、列表入场、启动按钮等非必要动效，只保留加载指示 */
  uiAnimation: boolean
  launcherVisibility: 'keep' | 'minimize'
}

export interface SystemInfo {
  version: string
  dataDir: string
  gameDir: string
  totalMemoryMb: number
  availableMemoryMb: number
  autoMemoryMb: number
  curseforgeConfigured: boolean
  localConfigPath: string | null
  /** 旧默认游戏目录（%APPDATA% 下）仍有版本时给出，用于提示 */
  legacyGameDir: string | null
}

/** 实时内存状态（MB） */
export interface MemoryStatus {
  totalMb: number
  availableMb: number
  autoMb: number
}

export interface ManifestVersion {
  id: string
  type: 'release' | 'snapshot' | 'old_beta' | 'old_alpha' | string
  url: string
  time: string
  releaseTime: string
  sha1?: string | null
  /** 后端计算：是否为愚人节版本（type 仍是 snapshot） */
  aprilFools?: boolean
}

export interface VersionManifest {
  latest: { release: string; snapshot: string }
  versions: ManifestVersion[]
}

export interface LoaderInfo {
  kind: string
  version: string
}

export interface InstanceView {
  name: string
  mcVersion: string
  loader: LoaderInfo | null
  /** 与 Forge 共存、以 mod 形式安装的 OptiFine 版本 */
  optifine: string | null
  createdAt: number
  lastPlayed: number | null
  description: string
  icon: string
  javaPath: string | null
  memoryMb: number | null
  jvmArgs: string
  gameArgs: string
  windowWidth: number | null
  windowHeight: number | null
  installed: boolean
  dir: string
  versionType: string
  managed: boolean
  hasJson: boolean
  hasJar: boolean
  running: boolean
}

export interface InstanceEdit {
  description: string
  icon: string
  javaPath: string | null
  memoryMb: number | null
  jvmArgs: string
  gameArgs: string
  windowWidth: number | null
  windowHeight: number | null
}

export interface TaskSnapshot {
  id: string
  title: string
  kind: string
  stage: string
  stageIndex: number
  stageCount: number
  status: 'running' | 'success' | 'failed' | 'cancelled'
  filesDone: number
  filesTotal: number
  bytesDone: number
  bytesTotal: number
  speed: number
  progress: number
  error: string | null
  createdAt: number
  finishedAt: number | null
}

export interface AccountView {
  id: string
  kind: 'offline' | 'yggdrasil'
  name: string
  uuid: string
  serverUrl: string | null
  serverName: string | null
  username: string | null
}

export interface AccountList {
  accounts: AccountView[]
  selected: string | null
}

export interface GameProfile {
  id: string
  name: string
}

export interface YggLoginResult {
  status: 'ok' | 'choose_profile'
  account: AccountView | null
  profiles: GameProfile[]
}

/** 账户皮肤：外置登录账户从认证服务器获取（data URL）；为空时显示默认皮肤（见 skin.ts） */
export interface SkinView {
  skin: string | null
  cape: string | null
  slim: boolean
  source: 'yggdrasil' | 'default'
  error: string | null
}

// ───── 清理 ─────

export type CleanupCategoryId = 'cache' | 'logs' | 'crash' | 'components'

export interface CleanupDetail {
  label: string
  files: number
  bytes: number
}

export interface CleanupCategory {
  id: CleanupCategoryId
  label: string
  description: string
  files: number
  bytes: number
  defaultSelected: boolean
  details: CleanupDetail[]
  notes: string[]
}

export interface CleanupScan {
  categories: CleanupCategory[]
  totalBytes: number
  /** 有下载 / 安装任务进行中，这时不能清理 */
  busy: boolean
}

export interface CleanupOutcome {
  id: CleanupCategoryId
  files: number
  bytes: number
  failed: number
}

export interface CleanupResult {
  categories: CleanupOutcome[]
  freedBytes: number
  deletedFiles: number
  failedFiles: number
}

export interface ServerInfo {
  apiRoot: string
  serverName: string | null
}

export interface JavaInfo {
  path: string
  version: string
  major: number
  source: string
}

export interface CrashInfo {
  title: string
  suggestions: string[]
  reportPath: string | null
  reportExcerpt: string | null
  logTail: string[]
}

export interface GameStateEvent {
  instance: string
  state: 'preparing' | 'running' | 'exited' | 'error'
  message: string | null
  pid: number | null
  exitCode: number | null
  killed: boolean
  crash: CrashInfo | null
}

export interface GameLogEvent {
  instance: string
  lines: string[]
}

// ───── 模组加载器 ─────

export type LoaderKind = 'forge' | 'neoforge' | 'fabric' | 'quilt' | 'optifine'

export interface LoaderVersion {
  kind: LoaderKind
  /** 安装标识（Forge 为 1.20.1-47.2.0，OptiFine 为 HD_U/I6） */
  id: string
  version: string
  mcVersion: string
  stable: boolean
  recommended: boolean
  latest: boolean
  note: string | null
}

export interface LoaderSpec {
  kind: LoaderKind
  version: string
}

export const LOADER_LABEL: Record<string, string> = {
  forge: 'Forge',
  neoforge: 'NeoForge',
  fabric: 'Fabric',
  quilt: 'Quilt',
  optifine: 'OptiFine',
}

/** 随加载器一起安装的前置模组（Modrinth 项目） */
export interface LoaderAddon {
  /** Modrinth 项目 ID */
  projectId: string
  title: string
  /** 版本过滤用的 loaders 参数 */
  loader: string
  /** 用于识别实例里已经装过的同一模组（fabric.mod.json / quilt.mod.json 的 id） */
  modIds: string[]
}

/** Fabric 实例自动安装 Fabric API（P7dR8mSH），Quilt 实例自动安装 QFAPI / QSL（slug qsl，ID qvIfYCYJ） */
export const LOADER_ADDONS: Partial<Record<LoaderKind, LoaderAddon>> = {
  fabric: { projectId: 'P7dR8mSH', title: 'Fabric API', loader: 'fabric', modIds: ['fabric-api', 'fabric'] },
  quilt: {
    projectId: 'qvIfYCYJ',
    title: 'Quilted Fabric API（QFAPI）',
    loader: 'quilt',
    modIds: ['quilted_fabric_api', 'qsl'],
  },
}

// ───── 社区资源 ─────

export type ResourceKind = 'mod' | 'modpack' | 'datapack' | 'resourcepack' | 'shaderpack' | 'world'
export type Platform = 'modrinth' | 'curseforge'
export type SearchPlatform = Platform | 'all'
export type SortKey = 'relevance' | 'downloads' | 'updated' | 'newest' | 'follows'

export const KIND_LABEL: Record<ResourceKind, string> = {
  mod: '模组',
  modpack: '整合包',
  datapack: '数据包',
  resourcepack: '资源包',
  shaderpack: '光影包',
  world: '地图',
}

export interface SearchQuery {
  platform: SearchPlatform
  kind: ResourceKind
  keyword: string
  mcVersion: string | null
  loader: string | null
  category: string | null
  sort: SortKey
  page: number
  pageSize: number
}

export interface ResourceSummary {
  platform: Platform
  id: string
  slug: string
  title: string
  description: string
  iconUrl: string | null
  downloads: number
  follows: number
  kind: ResourceKind
  author: string
  categories: string[]
  mcVersions: string[]
  updated: string | null
  pageUrl: string
}

export interface SearchPage {
  hits: ResourceSummary[]
  total: number
  page: number
  hasMore: boolean
  notices: string[]
}

export interface Category {
  id: string
  name: string
  icon: string | null
}

export interface ProjectDetail {
  summary: ResourceSummary
  body: string
  bodyFormat: 'markdown' | 'html'
  gallery: { url: string; title: string | null }[]
  links: { label: string; url: string }[]
  gameVersions: string[]
  loaders: string[]
}

export interface ResourceFile {
  url: string | null
  filename: string
  size: number
  sha1: string | null
  sha512: string | null
  primary: boolean
}

export interface Dependency {
  projectId: string
  versionId: string | null
  kind: 'required' | 'optional' | 'incompatible' | 'embedded' | string
}

export interface ResourceVersion {
  platform: Platform
  projectId: string
  id: string
  name: string
  versionNumber: string
  gameVersions: string[]
  loaders: string[]
  releaseType: 'release' | 'beta' | 'alpha' | string
  date: string
  downloads: number
  files: ResourceFile[]
  dependencies: Dependency[]
}

export interface PlanItem {
  platform: Platform
  projectId: string
  title: string
  iconUrl: string | null
  version: ResourceVersion | null
  root: boolean
  installed: boolean
  note: string | null
}

export interface InstallItem {
  title: string
  iconUrl: string | null
  version: ResourceVersion
}

export interface ArchiveInfo {
  kind: 'mrpack' | 'curseforge' | 'world' | 'unknown'
  name: string | null
  version: string | null
  mcVersion: string | null
  loaders: string[]
  worlds: string[]
  fileCount: number
  summary: string | null
}

export interface MissingFile {
  name: string
  fileName: string
  url: string
  target: string
}

export interface MissingEvent {
  instance: string
  files: MissingFile[]
}

export interface LocalResource {
  file: string
  name: string
  version: string | null
  description: string | null
  authors: string | null
  modId: string | null
  loader: string | null
  icon: string | null
  iconUrl: string | null
  enabled: boolean
  isDir: boolean
  size: number
  modified: number
  platform: Platform | null
  projectId: string | null
}

/** 实例中可管理的资源类型 */
export type LocalKind = 'mod' | 'resourcepack' | 'shaderpack' | 'world'

/** 实例可用的 mod 加载器名（Quilt 兼容 Fabric mod） */
export function instanceModLoaders(inst: InstanceView | null | undefined): string[] {
  switch (inst?.loader?.kind) {
    case 'fabric':
      return ['fabric']
    case 'quilt':
      return ['quilt', 'fabric']
    case 'forge':
      return ['forge']
    case 'neoforge':
      return ['neoforge']
    default:
      return []
  }
}

/** 按资源类型决定版本列表的加载器过滤条件 */
export function versionLoaders(kind: ResourceKind, inst: InstanceView | null | undefined): string[] {
  if (kind === 'mod') return instanceModLoaders(inst)
  if (kind === 'datapack') return ['datapack']
  return []
}


// ───── 联机（Terracotta | 陶瓦联机） ─────

/** Terracotta 的状态机（GET /state 的 state 字段） */
export type RoomPhase =
  | 'waiting'
  | 'host-scanning'
  | 'host-starting'
  | 'host-ok'
  | 'guest-connecting'
  | 'guest-starting'
  | 'guest-ok'
  | 'exception'

export interface TerracottaProfile {
  machineId: string
  name: string
  vendor: string
  /** HOST（房主）/ LOCAL（本机）/ GUEST（其他房客） */
  kind: 'HOST' | 'LOCAL' | 'GUEST' | string
}

export interface TerracottaRoom {
  state: RoomPhase | string
  index: number
  /** 邀请码 U/XXXX-XXXX-XXXX-XXXX */
  room: string | null
  /** 房客的直连地址（guest-ok） */
  url: string | null
  /** NAT 连接难度（guest-starting）：UNKNOWN / EASIEST / SIMPLE / MEDIUM / TOUGH */
  difficulty: string | null
  /** 异常类型（exception）：0~5 */
  exceptionType: number | null
  profiles: TerracottaProfile[]
}

export interface TerracottaView {
  installed: boolean
  installing: boolean
  /** 固定使用的陶瓦版本 */
  version: string
  packageSize: number
  /** 按下载源设置排列的下载渠道 */
  sources: string[]
  installDir: string
  process: 'stopped' | 'starting' | 'running' | 'crashed'
  error: string | null
  /** 复用其他程序（HMCL / PCL CE）启动的陶瓦，退出时不关闭 */
  external: boolean
  port: number | null
  coreVersion: string | null
  easytierVersion: string | null
  room: TerracottaRoom | null
  repoUrl: string
  licenseUrl: string
  easytierUrl: string
  docsUrl: string
}

export const EVENTS = {
  task: 'task-update',
  gameState: 'game-state',
  gameLog: 'game-log',
  missing: 'modpack-missing',
  terracotta: 'terracotta-state',
} as const

/** 把 invoke 抛出的错误统一转为字符串 */
export function errMsg(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  try {
    return JSON.stringify(e)
  } catch {
    return String(e)
  }
}

export const api = {
  getSettings: () => invoke<Settings>('get_settings'),
  saveSettings: (settings: Settings) => invoke<Settings>('save_settings', { settings }),
  systemInfo: () => invoke<SystemInfo>('get_system_info'),
  memoryStatus: () => invoke<MemoryStatus>('get_memory_status'),

  manifest: (force = false) => invoke<VersionManifest>('get_version_manifest', { force }),

  listInstances: () => invoke<InstanceView[]>('list_instances'),
  createInstance: (name: string, mcVersion: string, loaders: LoaderSpec[] = [], addons: InstallItem[] = []) =>
    invoke<string>('create_instance', { name, mcVersion, loaders, addons }),
  renameInstance: (oldName: string, newName: string) => invoke<void>('rename_instance', { oldName, newName }),
  deleteInstance: (name: string) => invoke<void>('delete_instance', { name }),
  updateInstance: (name: string, edit: InstanceEdit) => invoke<InstanceView>('update_instance', { name, edit }),
  repairInstance: (name: string) => invoke<string>('repair_instance', { name }),
  selectInstance: (name: string | null) => invoke<Settings>('select_instance', { name }),
  listSaves: (name: string) => invoke<string[]>('list_saves', { name }),
  openInstanceDir: (name: string, sub?: string) => invoke<void>('open_instance_dir', { name, sub: sub ?? null }),

  listTasks: () => invoke<TaskSnapshot[]>('list_tasks'),
  cancelTask: (id: string) => invoke<void>('cancel_task', { id }),
  clearFinishedTasks: () => invoke<TaskSnapshot[]>('clear_finished_tasks'),

  listAccounts: () => invoke<AccountList>('list_accounts'),
  addOfflineAccount: (name: string) => invoke<AccountView>('add_offline_account', { name }),
  checkAuthServer: (url: string) => invoke<ServerInfo>('check_auth_server', { url }),
  addYggdrasilAccount: (server: string, username: string, password: string, profileId?: string) =>
    invoke<YggLoginResult>('add_yggdrasil_account', { server, username, password, profileId: profileId ?? null }),
  removeAccount: (id: string) => invoke<AccountList>('remove_account', { id }),
  selectAccount: (id: string) => invoke<Settings>('select_account', { id }),
  accountSkin: (id: string, force = false) => invoke<SkinView>('get_account_skin', { id, force }),

  // 清理
  scanCleanup: () => invoke<CleanupScan>('scan_cleanup'),
  runCleanup: (categories: CleanupCategoryId[]) => invoke<CleanupResult>('run_cleanup', { categories }),

  listJava: () => invoke<JavaInfo[]>('list_java'),
  inspectJava: (path: string) => invoke<JavaInfo>('inspect_java', { path }),

  launch: (name: string) => invoke<void>('launch_game', { name }),
  kill: (name: string) => invoke<void>('kill_game', { name }),
  gameLog: (name: string) => invoke<string[]>('get_game_log', { name }),
  runningGames: () => invoke<string[]>('running_games'),

  // 加载器
  listLoaderVersions: (kind: LoaderKind, mcVersion: string, force = false) =>
    invoke<LoaderVersion[]>('list_loader_versions', { kind, mcVersion, force }),
  /** addons：随加载器安装到 mods 的前置模组（Fabric API / QFAPI），加载器装好后在同一个任务里下载 */
  installLoader: (name: string, loaders: LoaderSpec[], addons: InstallItem[] = []) =>
    invoke<string>('install_loader', { name, loaders, addons }),

  // 社区资源
  communitySearch: (query: SearchQuery) => invoke<SearchPage>('community_search', { query }),
  communityCategories: (platform: Platform, kind: ResourceKind) =>
    invoke<Category[]>('community_categories', { platform, kind }),
  communityProject: (platform: Platform, id: string, kind: ResourceKind) =>
    invoke<ProjectDetail>('community_project', { platform, id, kind }),
  communityVersions: (platform: Platform, projectId: string, mcVersion: string | null, loaders: string[]) =>
    invoke<ResourceVersion[]>('community_versions', { platform, projectId, mcVersion, loaders }),
  communityPlan: (instance: string, kind: ResourceKind, title: string, iconUrl: string | null, version: ResourceVersion) =>
    invoke<PlanItem[]>('community_plan', { instance, kind, title, iconUrl, version }),
  communityInstall: (instance: string, kind: ResourceKind, save: string | null, items: InstallItem[]) =>
    invoke<string>('community_install', { instance, kind, save, items }),
  communityInstallModpack: (version: ResourceVersion, name: string, title: string) =>
    invoke<string>('community_install_modpack', { version, name, title }),
  inspectArchive: (path: string) => invoke<ArchiveInfo>('inspect_archive', { path }),
  importModpack: (path: string, name: string) => invoke<string>('import_modpack', { path, name }),
  importWorld: (path: string, instance: string) => invoke<string>('import_world', { path, instance }),
  getMissingFiles: (name: string) => invoke<MissingFile[]>('get_missing_files', { name }),
  dismissMissingFiles: (name: string) => invoke<void>('dismiss_missing_files', { name }),

  // 实例内资源
  listResources: (name: string, kind: LocalKind) => invoke<LocalResource[]>('list_instance_resources', { name, kind }),
  setResourceEnabled: (name: string, kind: LocalKind, file: string, enabled: boolean) =>
    invoke<string>('set_resource_enabled', { name, kind, file, enabled }),
  deleteResource: (name: string, kind: LocalKind, file: string) =>
    invoke<void>('delete_instance_resource', { name, kind, file }),
  revealResource: (name: string, kind: LocalKind, file: string) =>
    invoke<void>('reveal_instance_resource', { name, kind, file }),

  // 联机（Terracotta | 陶瓦联机）
  terracottaStatus: () => invoke<TerracottaView>('terracotta_status'),
  terracottaWatch: (watching: boolean) => invoke<TerracottaView>('terracotta_watch', { watching }),
  terracottaInstall: () => invoke<string>('terracotta_install'),
  terracottaStart: () => invoke<TerracottaView>('terracotta_start'),
  terracottaStop: () => invoke<TerracottaView>('terracotta_stop'),
  terracottaRestart: () => invoke<TerracottaView>('terracotta_restart'),
  terracottaHost: () => invoke<TerracottaView>('terracotta_host'),
  terracottaJoin: (code: string) => invoke<TerracottaView>('terracotta_join', { code }),
  terracottaLeave: () => invoke<TerracottaView>('terracotta_leave'),
  terracottaLog: () => invoke<string>('terracotta_log'),

  openPath: (path: string) => invoke<void>('open_path', { path }),
  openUrl: (url: string) => invoke<void>('open_url', { url }),
  readImage: (path: string) => invoke<string>('read_image_data_url', { path }),
}

// ───── 格式化工具 ─────

export function formatBytes(n: number): string {
  if (!n || n < 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  let i = 0
  let v = n
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function formatDate(iso: string | number | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return '—'
  return d.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' })
}

export function relativeTime(ms: number | null | undefined): string {
  if (!ms) return '从未启动'
  const diff = Date.now() - ms
  const min = 60_000
  if (diff < min) return '刚刚'
  if (diff < 60 * min) return `${Math.floor(diff / min)} 分钟前`
  if (diff < 24 * 60 * min) return `${Math.floor(diff / (60 * min))} 小时前`
  if (diff < 30 * 24 * 60 * min) return `${Math.floor(diff / (24 * 60 * min))} 天前`
  return formatDate(ms)
}

export const VERSION_TYPE_LABEL: Record<string, string> = {
  release: '正式版',
  snapshot: '快照',
  old_beta: '远古 Beta',
  old_alpha: '远古 Alpha',
  /** 实例的 versionType（后端识别）；下载页的愚人节版本也用这个标签 */
  april_fools: '愚人节版本',
}

export function formatCount(n: number): string {
  if (!n) return '0'
  if (n >= 100_000_000) return `${(n / 100_000_000).toFixed(1)} 亿`
  if (n >= 10_000) return `${(n / 10_000).toFixed(1)} 万`
  return String(n)
}

/** 不重名的实例名 */
export function uniqueInstanceName(base: string, existing: { name: string }[]): string {
  const clean = base.replace(/[\\/:*?"<>|;!]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 60).replace(/\.+$/, '') || '新实例'
  const names = new Set(existing.map((i) => i.name.toLowerCase()))
  if (!names.has(clean.toLowerCase())) return clean
  let n = 2
  while (names.has(`${clean}-${n}`.toLowerCase())) n++
  return `${clean}-${n}`
}
