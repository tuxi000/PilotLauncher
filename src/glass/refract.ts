// 液态玻璃的两个指令：
//  v-refract：为元素生成专属的 SVG 位移滤镜（feDisplacementMap），在边缘产生折射，
//             通过 CSS 变量 --refract 提供给 backdrop-filter 使用（WebView2/Chromium 支持）。
//  v-light：  记录指针位置（像素）到 --mx / --my，在鼠标附近显示小光斑。
import type { Directive } from 'vue'

const NS = 'http://www.w3.org/2000/svg'
let host: SVGSVGElement | null = null
let seq = 0

function ensureDefs(): SVGDefsElement {
  if (!host) {
    host = document.createElementNS(NS, 'svg')
    host.setAttribute('width', '0')
    host.setAttribute('height', '0')
    host.setAttribute('aria-hidden', 'true')
    host.style.position = 'fixed'
    host.style.left = '-10px'
    host.style.top = '-10px'
    host.style.pointerEvents = 'none'
    host.appendChild(document.createElementNS(NS, 'defs'))
    document.body.appendChild(host)
  }
  return host.querySelector('defs') as SVGDefsElement
}

/**
 * 位移贴图：R 通道左→右渐变控制 x 位移，B 通道上→下渐变控制 y 位移，
 * 中间是一块模糊的 50% 灰圆角矩形（不位移），因此只有边缘一圈产生“透镜”折射。
 */
function displacementMap(w: number, h: number, radius: number, edge: number): string {
  const inner = Math.max(0, radius - edge / 2)
  const svg =
    `<svg xmlns="${NS}" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">` +
    `<defs>` +
    `<linearGradient id="r" x1="0" x2="1" y1="0" y2="0"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#f00"/></linearGradient>` +
    `<linearGradient id="b" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#00f"/></linearGradient>` +
    `<filter id="s" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="${(edge / 2.4).toFixed(2)}"/></filter>` +
    `</defs>` +
    `<rect width="${w}" height="${h}" fill="#000"/>` +
    `<rect width="${w}" height="${h}" fill="url(#r)"/>` +
    `<rect width="${w}" height="${h}" fill="url(#b)" style="mix-blend-mode:screen"/>` +
    `<rect x="${edge}" y="${edge}" width="${Math.max(1, w - edge * 2)}" height="${Math.max(1, h - edge * 2)}" rx="${inner}" fill="#808080" filter="url(#s)"/>` +
    `</svg>`
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg)
}

export interface RefractOptions {
  /** 位移强度（像素），默认 36 */
  scale?: number
  /** 折射边缘宽度（像素），默认取尺寸的 1/4，最大 18 */
  edge?: number
}

type RefractEl = HTMLElement & {
  __refract?: { filter: SVGFilterElement; ro: ResizeObserver }
}

export const vRefract: Directive<RefractEl, RefractOptions | undefined> = {
  mounted(el, binding) {
    const id = `lg-refract-${++seq}`
    const filter = document.createElementNS(NS, 'filter') as SVGFilterElement
    filter.setAttribute('id', id)
    filter.setAttribute('filterUnits', 'userSpaceOnUse')
    filter.setAttribute('primitiveUnits', 'userSpaceOnUse')
    filter.setAttribute('color-interpolation-filters', 'sRGB')
    const image = document.createElementNS(NS, 'feImage')
    image.setAttribute('result', 'map')
    image.setAttribute('preserveAspectRatio', 'none')
    const disp = document.createElementNS(NS, 'feDisplacementMap')
    disp.setAttribute('in', 'SourceGraphic')
    disp.setAttribute('in2', 'map')
    disp.setAttribute('xChannelSelector', 'R')
    disp.setAttribute('yChannelSelector', 'B')
    // 负值：边缘处从内侧取样，产生向内“放大”的透镜感，同时避免取到元素外的透明像素
    disp.setAttribute('scale', String(-(binding.value?.scale ?? 36)))
    filter.append(image, disp)
    ensureDefs().appendChild(filter)

    let lastKey = ''
    const update = () => {
      const w = Math.max(1, Math.round(el.offsetWidth))
      const h = Math.max(1, Math.round(el.offsetHeight))
      const radius = parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0
      const edge = binding.value?.edge ?? Math.min(18, Math.max(4, Math.min(w, h) / 4))
      const key = `${w}x${h}r${radius}e${edge}`
      if (key === lastKey) return
      lastKey = key
      for (const node of [filter, image]) {
        node.setAttribute('x', '0')
        node.setAttribute('y', '0')
        node.setAttribute('width', String(w))
        node.setAttribute('height', String(h))
      }
      image.setAttribute('href', displacementMap(w, h, radius, edge))
    }
    update()
    const ro = new ResizeObserver(update)
    ro.observe(el)
    el.style.setProperty('--refract', `url(#${id})`)
    el.dataset.refract = ''
    el.__refract = { filter, ro }
  },
  unmounted(el) {
    el.__refract?.ro.disconnect()
    el.__refract?.filter.remove()
    delete el.__refract
  },
}

type LightEl = HTMLElement & { __light?: (e: PointerEvent) => void; __leave?: () => void }

export const vLight: Directive<LightEl> = {
  mounted(el) {
    const move = (e: PointerEvent) => {
      const r = el.getBoundingClientRect()
      // 用像素坐标，光斑大小不随元素尺寸变化
      el.style.setProperty('--mx', `${(e.clientX - r.left).toFixed(0)}px`)
      el.style.setProperty('--my', `${(e.clientY - r.top).toFixed(0)}px`)
      el.style.setProperty('--light', '1')
    }
    const leave = () => el.style.setProperty('--light', '0')
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerleave', leave)
    el.__light = move
    el.__leave = leave
  },
  unmounted(el) {
    if (el.__light) el.removeEventListener('pointermove', el.__light)
    if (el.__leave) el.removeEventListener('pointerleave', el.__leave)
  },
}
