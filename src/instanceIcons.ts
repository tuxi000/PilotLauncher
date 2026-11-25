// 实例 / 游戏版本 / 加载器图标（全部为 256×256 透明 PNG，见 src/assets/icons/）。
//  - 方块图标：用原版 16×16 方块贴图（版权归 Mojang）按物品栏视角重新渲染成等距方块，
//    贴图按最近邻采样，像素边缘清晰；大图缩小显示时由浏览器平滑缩放。
//  - Fabric：FabricMC 官方 32×32 像素 logo，按 8 倍最近邻放大。
//  - NeoForge：NeoForged 官方像素狐狸 logo，去掉底板后按整数倍放大。
//  - Quilt：QuiltMC/art 仓库的官方 logo（CC0）。
// 每个图标在页面上只是一个 <img>，版本列表有几百项时也不会产生大量 DOM 节点。

import grassPng from './assets/icons/grass.png'
import commandBlockPng from './assets/icons/command_block.png'
import cobblestonePng from './assets/icons/cobblestone.png'
import goldBlockPng from './assets/icons/gold_block.png'
import anvilPng from './assets/icons/anvil.png'
import fabricPng from './assets/icons/fabric.png'
import neoforgePng from './assets/icons/neoforge.png'
import quiltPng from './assets/icons/quilt.png'
import optifinePng from './assets/icons/optifine.png'
import stonePng from './assets/icons/stone.png'
import diamondPng from './assets/icons/diamond.png'
import redstonePng from './assets/icons/redstone.png'
import amethystPng from './assets/icons/amethyst.png'

/** 图片图标（图标名 → 打包后的 URL） */
export const PNG_ICONS: Record<string, string> = {
  grass_block: grassPng,
  command_block: commandBlockPng,
  cobblestone: cobblestonePng,
  gold_block: goldBlockPng,
  anvil: anvilPng,
  fabric: fabricPng,
  neoforge: neoforgePng,
  quilt: quiltPng,
  optifine: optifinePng,
  stone: stonePng,
  diamond: diamondPng,
  redstone: redstonePng,
  amethyst: amethystPng,
}

/** 旧图标名兼容：第三阶段手绘的“金块”（gold）现在显示金块图片 */
const ICON_ALIAS: Record<string, string> = {
  gold: 'gold_block',
}

type Grid = string[]
type Palette = Record<string, string>
interface CubeDef {
  kind: 'cube'
  pal: Palette
  top: Grid
  left: Grid
  right: Grid
}
interface FlatDef {
  kind: 'flat'
  pal: Palette
  grid: Grid
}
type IconDef = CubeDef | FlatDef

/** 程序绘制的像素画图标（目前全部改用 PNG，保留结构以便以后扩展） */
export const ICONS: Record<string, IconDef> = {}

export const ICON_LABEL: Record<string, string> = {
  auto: '自动',
  grass_block: '草方块（正式版）',
  command_block: '命令方块（快照）',
  cobblestone: '原石（远古版本）',
  gold_block: '金块（愚人节版本）',
  anvil: '铁砧（Forge）',
  fabric: '布卷（Fabric）',
  neoforge: '狐狸（NeoForge）',
  quilt: 'Quilt',
  optifine: '土径（OptiFine）',
  stone: '石头',
  diamond: '钻石块',
  redstone: '红石块',
  amethyst: '紫水晶块',
  gold: '金块（愚人节版本）',
}

/** 实例设置里图标选择器的顺序（第一个是“自动”） */
export const ICON_NAMES: string[] = ['auto', ...Object.keys(PNG_ICONS), ...Object.keys(ICONS)]

function canonical(icon: string): string {
  return ICON_ALIAS[icon] ?? icon
}

/** 是否为已知图标名 */
export function isKnownIcon(icon: string | null | undefined): boolean {
  if (!icon) return false
  const k = canonical(icon)
  return !!PNG_ICONS[k] || !!ICONS[k]
}

/** 旧数据兼容：空、auto、旧默认值 grass、未知图标名都按“自动”处理 */
export function isAutoIcon(icon: string | null | undefined): boolean {
  return !icon || icon === 'auto' || icon === 'grass' || !isKnownIcon(icon)
}

/** 实例设置里图标选择器的初始值：自动返回 "auto"，旧图标名换成当前的名字（gold → gold_block） */
export function editableIcon(icon: string | null | undefined): string {
  return isAutoIcon(icon) ? 'auto' : canonical(icon as string)
}

/** 按版本类型：正式版草方块、快照命令方块、远古版本原石、愚人节版本金块 */
export function versionIcon(type: string | null | undefined): string {
  switch (type) {
    case 'snapshot':
      return 'command_block'
    case 'old_beta':
    case 'old_alpha':
      return 'cobblestone'
    case 'april_fools':
      return 'gold_block'
    default:
      return 'grass_block'
  }
}

/** 加载器图标（Forge + OptiFine 的实例 loader.kind 为 forge，显示铁砧） */
export function loaderIcon(kind: string | null | undefined): string | null {
  switch (kind) {
    case 'forge':
      return 'anvil'
    case 'neoforge':
      return 'neoforge'
    case 'fabric':
      return 'fabric'
    case 'quilt':
      return 'quilt'
    case 'optifine':
      return 'optifine'
    default:
      return null
  }
}

/** 最终显示的图标：手动选过的保留；自动时有加载器显示加载器图标，否则按版本类型 */
export function resolveIcon(
  icon: string | null | undefined,
  loader: string | null | undefined,
  versionType: string | null | undefined,
): string {
  if (!isAutoIcon(icon)) return canonical(icon as string)
  return loaderIcon(loader) ?? versionIcon(versionType)
}

// ───── SVG 生成 ─────

/** 把网格按行合并为同色的水平线段，按颜色输出 <path> */
function runs(grid: Grid, pal: Palette): string {
  const byColor: Record<string, string[]> = {}
  grid.forEach((row, y) => {
    let x = 0
    while (x < row.length) {
      const c = row[x]
      if (c === '.' || c === ' ' || !pal[c]) {
        x++
        continue
      }
      const x0 = x
      while (x < row.length && row[x] === c) x++
      ;(byColor[c] ??= []).push(`M${x0} ${y}h${x - x0}v1h${x0 - x}z`)
    }
  })
  return Object.entries(byColor)
    .map(([c, d]) => `<path fill="${pal[c]}" d="${d.join('')}"/>`)
    .join('')
}

function dominant(grid: Grid, pal: Palette): string {
  const count: Record<string, number> = {}
  for (const row of grid) for (const c of row) if (pal[c]) count[c] = (count[c] ?? 0) + 1
  const best = Object.entries(count).sort((a, b) => b[1] - a[1])[0]
  return best ? pal[best[0]] : '#888'
}

// 32×32 视口中的等距立方体：每个面把 8×8 贴图仿射变换到对应的平行四边形
const FACE: Record<'top' | 'left' | 'right', { m: string; poly: string }> = {
  top: { m: 'matrix(1.5 0.8125 -1.5 0.8125 16 3)', poly: '16,3 28,9.5 16,16 4,9.5' },
  left: { m: 'matrix(1.5 0.8125 0 1.625 4 9.5)', poly: '4,9.5 16,16 16,29 4,22.5' },
  right: { m: 'matrix(1.5 -0.8125 0 1.625 16 16)', poly: '16,16 28,9.5 28,22.5 16,29' },
}

function cubeSvg(d: CubeDef): string {
  const faces = (['top', 'left', 'right'] as const)
    .map((f) => {
      const g = d[f]
      // 先铺一层主色，避免变换后相邻色块之间出现细缝
      return `<polygon points="${FACE[f].poly}" fill="${dominant(g, d.pal)}"/><g transform="${FACE[f].m}">${runs(g, d.pal)}</g>`
    })
    .join('')
  const shade =
    `<polygon points="${FACE.top.poly}" fill="#fff" opacity="0.1"/>` +
    `<polygon points="${FACE.left.poly}" fill="#000" opacity="0.16"/>` +
    `<polygon points="${FACE.right.poly}" fill="#000" opacity="0.3"/>` +
    '<path d="M4 9.5 16 16 28 9.5M16 16v13" fill="none" stroke="#fff" stroke-opacity="0.22" stroke-width="0.5"/>' +
    '<polygon points="16,3 28,9.5 28,22.5 16,29 4,22.5 4,9.5" fill="none" stroke="#000" stroke-opacity="0.45" stroke-width="0.7" stroke-linejoin="round"/>'
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" shape-rendering="crispEdges">${faces}${shade}</svg>`
}

function flatSvg(d: FlatDef): string {
  // 四周留 1 像素，使平面图标与等距方块的视觉大小接近
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1 -1 18 18" shape-rendering="crispEdges">${runs(d.grid, d.pal)}</svg>`
}

const urlCache = new Map<string, string>()

/** 图标的 URL：图片图标直接返回打包后的地址；像素画图标生成 SVG data URL（带缓存）；未知图标显示草方块 */
export function iconUrl(key: string): string {
  const k = canonical(key)
  const png = PNG_ICONS[k]
  if (png) return png
  const hit = urlCache.get(k)
  if (hit) return hit
  const def = ICONS[k]
  if (!def) return PNG_ICONS.grass_block
  const svg = def.kind === 'cube' ? cubeSvg(def) : flatSvg(def)
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
  urlCache.set(k, url)
  return url
}
