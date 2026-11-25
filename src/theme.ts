// 主题：深色 / 浅色 / 跟随系统；窗口材质与折射开关通过 <html> 上的属性/类控制
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { BackgroundPalette, ThemeMode, WindowEffect } from './api'

const media = window.matchMedia('(prefers-color-scheme: dark)')
let mode: ThemeMode = 'system'

function resolved(): 'light' | 'dark' {
  if (mode === 'system') return media.matches ? 'dark' : 'light'
  return mode
}

export function applyTheme(m: ThemeMode) {
  mode = m
  document.documentElement.dataset.theme = resolved()
  // 同步原生窗口主题（影响 Mica 色调）；null 表示跟随系统
  try {
    const win = getCurrentWindow() as unknown as { setTheme?: (t: 'light' | 'dark' | null) => Promise<void> }
    win.setTheme?.(m === 'system' ? null : m)?.catch(() => {})
  } catch {
    /* 非 Tauri 环境（例如纯浏览器预览）忽略 */
  }
}

media.addEventListener('change', () => {
  if (mode === 'system') document.documentElement.dataset.theme = resolved()
})

export function applyEffect(effect: WindowEffect) {
  document.documentElement.dataset.effect = effect
}

export function applyRefraction(on: boolean) {
  document.documentElement.classList.toggle('refract', on)
}

/** 背景配色：html[data-palette] 控制强调色的微调（base.css），内置背景本身由 AmbientBackground 按 prop 绘制 */
export function applyPalette(p: BackgroundPalette | null | undefined) {
  document.documentElement.dataset.palette = p === 'mint' || p === 'coral' ? p : 'blue'
}
