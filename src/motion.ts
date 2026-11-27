// 界面动画开关与小工具
//  - html.no-motion：设置 → 外观 → 界面动画 关闭时加上，base.css 据此去掉非必要的过渡和动画
//  - 系统开启“减少动画”（prefers-reduced-motion）时同样视为关闭
//  - lastPointer：记录最近一次按下的位置，对话框 / 账户面板从触发位置弹出
import { reactive } from 'vue'

const reducedQuery = window.matchMedia('(prefers-reduced-motion: reduce)')

export const motion = reactive({
  /** 设置里的“界面动画” */
  enabled: true,
  /** 系统“减少动画” */
  reduced: reducedQuery.matches,
})

reducedQuery.addEventListener('change', () => {
  motion.reduced = reducedQuery.matches
})

/** 是否播放非必要动画（JS 驱动的动效在开始前检查） */
export function motionOn(): boolean {
  return motion.enabled && !motion.reduced
}

export function applyMotion(on: boolean) {
  motion.enabled = on
  document.documentElement.classList.toggle('no-motion', !on)
}

const pointer = { x: -1, y: -1, t: 0 }
document.addEventListener(
  'pointerdown',
  (e) => {
    pointer.x = e.clientX
    pointer.y = e.clientY
    pointer.t = performance.now()
  },
  true,
)

/** 最近 maxAge 毫秒内的按下位置（键盘触发等情况返回 null） */
export function recentPointer(maxAge = 1200): { x: number; y: number } | null {
  if (pointer.x < 0 || performance.now() - pointer.t > maxAge) return null
  return { x: pointer.x, y: pointer.y }
}
