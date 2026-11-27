<script setup lang="ts">
// 3D 皮肤预览（skinview3d，MIT）：可拖动旋转；待机动画，点一下挥手。
//  - 背景透明，由外层的玻璃面板决定颜色，深浅色都协调
//  - 卸载时 dispose 并主动释放 WebGL 上下文（浏览器同时能保留的上下文数量有限）
//  - 关闭“界面动画”或系统开启“减少动画”时不播放动作，只保留静止姿势
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { IdleAnimation, SkinViewer, WalkingAnimation, WaveAnimation } from 'skinview3d'

const props = withDefaults(defineProps<{ skin: string; slim?: boolean; cape?: string | null; animated?: boolean }>(), {
  slim: false,
  cape: null,
  animated: true,
})
const emit = defineEmits<{ (e: 'failed'): void }>()

const host = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const ready = ref(false)

let viewer: SkinViewer | null = null
let ro: ResizeObserver | null = null
let actionTimer: number | undefined
let loadSeq = 0

function idle() {
  if (!viewer) return
  window.clearTimeout(actionTimer)
  viewer.animation = props.animated ? new IdleAnimation() : null
}

/** 播放一段动作，结束后回到待机 */
function play(kind: 'wave' | 'walk', ms: number) {
  if (!viewer || !props.animated) return
  window.clearTimeout(actionTimer)
  viewer.animation = kind === 'wave' ? new WaveAnimation('right') : new WalkingAnimation()
  actionTimer = window.setTimeout(idle, ms)
}

function wave() {
  play('wave', 2600)
}
function walk() {
  play('walk', 3200)
}
defineExpose({ wave, walk })

async function loadSkin() {
  const v = viewer
  if (!v) return
  const my = ++loadSeq
  try {
    await v.loadSkin(props.skin, { model: props.slim ? 'slim' : 'default' })
    if (my === loadSeq && viewer === v) ready.value = true
  } catch (e) {
    console.warn('皮肤贴图加载失败', e)
  }
}

function loadCape() {
  const v = viewer
  if (!v) return
  const cape = props.cape
  if (cape) {
    v.loadCape(cape).catch((e: unknown) => console.warn('披风贴图加载失败', e))
  } else {
    v.loadCape(null)
  }
}

// 区分“点一下”和“拖动旋转”：按下到松开移动不超过 5 像素算点击
let downAt: { x: number; y: number } | null = null
function onDown(e: PointerEvent) {
  downAt = { x: e.clientX, y: e.clientY }
}
function onUp(e: PointerEvent) {
  if (downAt && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) < 5) wave()
  downAt = null
}

onMounted(() => {
  const el = host.value
  const cv = canvas.value
  if (!el || !cv) return
  try {
    viewer = new SkinViewer({
      canvas: cv,
      width: Math.max(1, el.clientWidth),
      height: Math.max(1, el.clientHeight),
      zoom: 0.84,
      fov: 40,
    })
  } catch (e) {
    console.warn('3D 皮肤预览初始化失败', e)
    viewer = null
    emit('failed')
    return
  }
  viewer.controls.enableZoom = false
  viewer.controls.enablePan = false
  // 稍微侧一点，比正对着更有立体感
  viewer.playerWrapper.rotation.y = 0.45
  idle()
  loadSkin()
  loadCape()
  ro = new ResizeObserver(() => {
    if (viewer && el.clientWidth > 0 && el.clientHeight > 0) viewer.setSize(el.clientWidth, el.clientHeight)
  })
  ro.observe(el)
})

watch(
  () => [props.skin, props.slim] as const,
  () => loadSkin(),
)
watch(
  () => props.cape,
  () => loadCape(),
)
watch(
  () => props.animated,
  () => idle(),
)

onBeforeUnmount(() => {
  window.clearTimeout(actionTimer)
  ro?.disconnect()
  ro = null
  const v = viewer
  viewer = null
  if (v) {
    const renderer = v.renderer
    v.dispose()
    try {
      renderer.forceContextLoss()
    } catch {
      /* 已经释放 */
    }
  }
})
</script>

<template>
  <div ref="host" class="sv3d" :class="{ ready }">
    <canvas ref="canvas" class="sv-canvas" title="拖动旋转，点一下打招呼" @pointerdown="onDown" @pointerup="onUp" />
  </div>
</template>

<style scoped>
.sv3d {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
}
.sv-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
  cursor: grab;
  opacity: 0;
  transform: translateY(8px) scale(0.96);
  transition: opacity 0.4s var(--ease-out), transform 0.7s var(--spring);
}
.sv-canvas:active {
  cursor: grabbing;
}
.sv3d.ready .sv-canvas {
  opacity: 1;
  transform: none;
}
</style>
