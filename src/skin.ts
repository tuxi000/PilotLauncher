// 皮肤：默认皮肤规则、外置登录皮肤缓存、头像（皮肤的头部 + 帽子层）
import { reactive } from 'vue'
import steveUrl from './assets/skins/steve.png'
import alexUrl from './assets/skins/alex.png'
import { api, type AccountView, type SkinView } from './api'

/**
 * 默认皮肤（离线账户、没有设置皮肤的外置账户）按 Java 版 1.19.2 及以前的规则选择：
 * UUID.hashCode() 为奇数显示 Alex（纤细手臂），偶数显示 Steve。
 * Java 的 UUID.hashCode() = (int)(hilo >> 32) ^ (int)hilo，其中 hilo = 高 64 位 ^ 低 64 位，
 * 等价于把 32 位十六进制 UUID 切成 4 段 32 位整数后全部异或。
 * （1.19.3 起游戏改成 9 种默认皮肤，这里沿用经典的两种，和多数启动器、皮肤站一致。）
 */
export function isSlimByUuid(uuid: string): boolean {
  const hex = uuid.replace(/-/g, '')
  if (!/^[0-9a-fA-F]{32}$/.test(hex)) return false
  let h = 0
  for (let i = 0; i < 32; i += 8) h ^= parseInt(hex.slice(i, i + 8), 16) | 0
  return (h & 1) === 1
}

export interface ResolvedSkin {
  /** 皮肤贴图地址（data URL 或打包进程序的默认皮肤） */
  url: string
  slim: boolean
  cape: string | null
  /** 是否为账户自己的皮肤（false 表示默认皮肤） */
  custom: boolean
}

const state = reactive({
  skins: {} as Record<string, SkinView>,
  loading: {} as Record<string, boolean>,
})

export function defaultSkin(uuid: string | null | undefined): ResolvedSkin {
  const slim = !!uuid && isSlimByUuid(uuid)
  return { url: slim ? alexUrl : steveUrl, slim, cape: null, custom: false }
}

/** 账户当前应显示的皮肤（外置皮肤还没取到时先显示默认皮肤） */
export function resolveSkin(acc: AccountView | null | undefined): ResolvedSkin {
  if (!acc) return defaultSkin(null)
  const s = state.skins[acc.id]
  if (s?.skin) return { url: s.skin, slim: s.slim, cape: s.cape, custom: true }
  return defaultSkin(acc.uuid)
}

export function skinLoading(id: string | null | undefined): boolean {
  return !!id && !!state.loading[id]
}

export function skinError(id: string | null | undefined): string | null {
  if (!id) return null
  return state.skins[id]?.error ?? null
}

/** 读取外置登录账户的皮肤（后端会缓存到 cache/skins/）；离线账户不需要请求 */
export async function loadSkin(acc: AccountView, force = false) {
  if (acc.kind !== 'yggdrasil') return
  if (state.loading[acc.id]) return
  if (!force && state.skins[acc.id]) return
  state.loading[acc.id] = true
  try {
    state.skins[acc.id] = await api.accountSkin(acc.id, force)
  } catch (e) {
    console.warn('读取皮肤失败', e)
  } finally {
    delete state.loading[acc.id]
  }
}

export function loadSkins(list: AccountView[]) {
  for (const a of list) void loadSkin(a)
  // 已删除账户的缓存一并清掉
  const ids = new Set(list.map((a) => a.id))
  for (const id of Object.keys(state.skins)) if (!ids.has(id)) delete state.skins[id]
}

// ───── 头像：取皮肤的 8×8 头部，叠加帽子层（第二层），按像素放大显示 ─────

const heads = new Map<string, Promise<string>>()

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.decoding = 'async'
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error('皮肤图片加载失败'))
    img.src = src
  })
}

async function renderHead(src: string): Promise<string> {
  const img = await loadImage(src)
  const w = img.naturalWidth
  const h = img.naturalHeight
  if (w < 64 || h < 32 || (h !== w && h * 2 !== w)) throw new Error('不是有效的皮肤贴图')
  // 高清皮肤（128×128 等）按比例取
  const k = w / 64
  const s = Math.round(8 * k)
  const sheet = document.createElement('canvas')
  sheet.width = w
  sheet.height = h
  const sctx = sheet.getContext('2d', { willReadFrequently: true })
  if (!sctx) throw new Error('无法创建画布')
  sctx.drawImage(img, 0, 0)

  const out = document.createElement('canvas')
  out.width = s
  out.height = s
  const ctx = out.getContext('2d')
  if (!ctx) throw new Error('无法创建画布')
  ctx.imageSmoothingEnabled = false
  ctx.drawImage(sheet, 8 * k, 8 * k, s, s, 0, 0, s, s)

  // 帽子层：64×32 的旧格式皮肤如果这一块完全不透明，游戏会当作没有帽子（否则常见的黑色底会盖住脸）
  const hat = sctx.getImageData(40 * k, 8 * k, s, s).data
  let opaque = true
  let visible = false
  for (let i = 3; i < hat.length; i += 4) {
    if (hat[i] < 255) opaque = false
    if (hat[i] > 0) visible = true
  }
  const legacy = h * 2 === w
  if (visible && !(legacy && opaque)) ctx.drawImage(sheet, 40 * k, 8 * k, s, s, 0, 0, s, s)
  return out.toDataURL('image/png')
}

/** 皮肤贴图 → 头像 data URL（结果缓存） */
export function headUrl(skinUrl: string): Promise<string> {
  let p = heads.get(skinUrl)
  if (!p) {
    p = renderHead(skinUrl)
    heads.set(skinUrl, p)
    p.catch(() => heads.delete(skinUrl))
    if (heads.size > 64) {
      const oldest = heads.keys().next().value
      if (oldest !== undefined && oldest !== skinUrl) heads.delete(oldest)
    }
  }
  return p
}
