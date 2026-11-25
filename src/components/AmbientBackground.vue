<script setup lang="ts">
// 内置背景：现代简约风格，不含任何 Minecraft 元素。
//  - 组成：底色渐变 + 三团柔和的彩色光晕 + 两道半透明渐变波浪 + 三条细曲线 + 少量几何形状（圆、圆环、圆角方块）
//  - 全部用 CSS / SVG 绘制，矢量的，任意窗口尺寸都清晰；SVG 用 preserveAspectRatio="xMidYMid slice" 铺满，
//    细线用 vector-effect="non-scaling-stroke" 保持 1px 左右的粗细
//  - 三套配色（blue / mint / coral），每套有浅色、深色两个版本，跟随 html[data-theme] 自动切换
//  - 动画：每一层是独立的 HTML 元素，只对 transform / opacity 做 40～80 秒的往复动画（不改 SVG path），
//    在 WebView2 中由合成器处理，不会每帧重绘；animated = false 或系统开启“减少动画”时完全静止
//  - 尺寸都用百分比，所以也可以缩小放进设置页的色卡里当预览
import type { BackgroundPalette } from '../api'

withDefaults(defineProps<{ palette?: BackgroundPalette; animated?: boolean }>(), {
  palette: 'blue',
  animated: true,
})

// 每个实例的 SVG 渐变 id 必须唯一（设置页的三张色卡也会各渲染一份）
const uid = `amb-${Math.random().toString(36).slice(2, 9)}`
</script>

<template>
  <div class="ambient" :data-palette="palette" :class="{ still: !animated }" aria-hidden="true">
    <div class="base" />
    <div class="blob b1 anim" />
    <div class="blob b2 anim" />
    <div class="blob b3 anim" />

    <!-- 渐变波浪 -->
    <div class="layer waves anim">
      <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
        <defs>
          <linearGradient :id="`${uid}-wa`" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" class="wa0" />
            <stop offset="1" class="wa1" />
          </linearGradient>
          <linearGradient :id="`${uid}-wb`" x1="1" y1="0" x2="0" y2="1">
            <stop offset="0" class="wb0" />
            <stop offset="1" class="wb1" />
          </linearGradient>
        </defs>
        <path
          :fill="`url(#${uid}-wa)`"
          d="M0 640 C 300 560 560 760 900 690 S 1400 500 1600 560 L 1600 1000 L 0 1000 Z"
        />
        <path
          :fill="`url(#${uid}-wb)`"
          d="M0 770 C 380 690 700 900 1080 800 S 1460 660 1600 700 L 1600 1000 L 0 1000 Z"
        />
      </svg>
    </div>

    <!-- 细曲线 -->
    <div class="layer lines anim">
      <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" fill="none">
        <path
          class="ln"
          vector-effect="non-scaling-stroke"
          d="M-40 600 C 280 520 560 720 900 640 S 1380 470 1640 520"
        />
        <path
          class="ln ln-2"
          vector-effect="non-scaling-stroke"
          d="M-40 660 C 300 590 600 790 940 700 S 1400 540 1640 590"
        />
        <path
          class="ln ln-3"
          vector-effect="non-scaling-stroke"
          d="M-40 250 C 260 170 520 330 860 230 S 1300 90 1640 150"
        />
      </svg>
    </div>

    <!-- 几何形状 -->
    <div class="layer shapes anim">
      <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
        <circle class="disc" cx="1250" cy="250" r="165" />
        <circle class="ring" vector-effect="non-scaling-stroke" cx="1370" cy="370" r="78" />
        <rect
          class="tile"
          vector-effect="non-scaling-stroke"
          x="140"
          y="330"
          width="150"
          height="150"
          rx="38"
          transform="rotate(16 215 405)"
        />
        <circle class="dot" cx="420" cy="470" r="9" />
      </svg>
    </div>
  </div>
</template>

<style scoped>
.ambient {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  contain: strict;
}
.base {
  position: absolute;
  inset: 0;
  background: linear-gradient(135deg, var(--bg-1) 0%, var(--bg-2) 52%, var(--bg-3) 100%);
}
.blob {
  position: absolute;
  aspect-ratio: 1 / 1;
  border-radius: 50%;
  will-change: transform;
}
.b1 {
  width: 68%;
  left: -18%;
  top: -34%;
  background: radial-gradient(closest-side, var(--blob-1), transparent);
  animation: amb-drift-1 58s ease-in-out infinite alternate;
}
.b2 {
  width: 58%;
  right: -16%;
  top: -6%;
  background: radial-gradient(closest-side, var(--blob-2), transparent);
  animation: amb-drift-2 70s ease-in-out infinite alternate;
}
.b3 {
  width: 64%;
  left: 26%;
  bottom: -52%;
  background: radial-gradient(closest-side, var(--blob-3), transparent);
  animation: amb-drift-3 82s ease-in-out infinite alternate;
}
.layer {
  position: absolute;
  inset: -5%;
  will-change: transform;
}
.layer svg {
  display: block;
  width: 100%;
  height: 100%;
}
.waves {
  animation: amb-sway 64s ease-in-out infinite alternate;
}
.lines {
  animation: amb-float 48s ease-in-out infinite alternate;
}
.shapes {
  animation: amb-bob 54s ease-in-out infinite alternate;
}
.wa0 {
  stop-color: var(--wave-a);
}
.wa1 {
  stop-color: var(--wave-a);
  stop-opacity: 0.15;
}
.wb0 {
  stop-color: var(--wave-b);
}
.wb1 {
  stop-color: var(--wave-b);
  stop-opacity: 0.2;
}
.ln {
  stroke: var(--line);
  stroke-width: 1.25;
  stroke-linecap: round;
}
.ln-2 {
  opacity: 0.55;
}
.ln-3 {
  opacity: 0.45;
  stroke-dasharray: 2 7;
}
.disc {
  fill: var(--disc);
}
.ring {
  fill: none;
  stroke: var(--ring);
  stroke-width: 1.25;
}
.tile {
  fill: var(--tile);
  stroke: var(--ring);
  stroke-width: 1;
}
.dot {
  fill: var(--ring);
}

/* 动画只用 transform；幅度很小、周期 48～82 秒，几乎察觉不到 */
@keyframes amb-drift-1 {
  from {
    transform: translate3d(0, 0, 0) scale(1);
  }
  to {
    transform: translate3d(9%, 7%, 0) scale(1.08);
  }
}
@keyframes amb-drift-2 {
  from {
    transform: translate3d(0, 0, 0) scale(1.04);
  }
  to {
    transform: translate3d(-10%, 9%, 0) scale(0.95);
  }
}
@keyframes amb-drift-3 {
  from {
    transform: translate3d(0, 0, 0) scale(0.96);
  }
  to {
    transform: translate3d(-8%, -6%, 0) scale(1.06);
  }
}
@keyframes amb-sway {
  from {
    transform: translate3d(-1.5%, 0.6%, 0);
  }
  to {
    transform: translate3d(1.5%, -0.6%, 0);
  }
}
@keyframes amb-float {
  from {
    transform: translate3d(1%, -1.2%, 0);
  }
  to {
    transform: translate3d(-1%, 1.2%, 0);
  }
}
@keyframes amb-bob {
  from {
    transform: translate3d(0, 1.4%, 0) rotate(-0.6deg);
  }
  to {
    transform: translate3d(-0.8%, -1.4%, 0) rotate(0.6deg);
  }
}

/* 关闭动画 / 系统开启“减少动画”：完全静止（base.css 的全局规则只把时长压到 0.01ms，
   对无限往复动画会变成闪烁，所以这里必须显式 animation: none） */
.ambient.still .anim {
  animation: none !important;
}
@media (prefers-reduced-motion: reduce) {
  .ambient .anim {
    animation: none !important;
  }
}

/* ───── 配色：蓝紫（冷静、现代） ───── */
.ambient[data-palette='blue'] {
  --bg-1: #eef1ff;
  --bg-2: #ebe8ff;
  --bg-3: #f5f1ff;
  --blob-1: rgba(129, 140, 248, 0.5);
  --blob-2: rgba(192, 132, 252, 0.36);
  --blob-3: rgba(96, 165, 250, 0.4);
  --wave-a: rgba(129, 140, 248, 0.26);
  --wave-b: rgba(167, 139, 250, 0.2);
  --line: rgba(99, 102, 241, 0.34);
  --disc: rgba(165, 180, 252, 0.34);
  --ring: rgba(99, 102, 241, 0.3);
  --tile: rgba(255, 255, 255, 0.4);
}
html[data-theme='dark'] .ambient[data-palette='blue'] {
  --bg-1: #0f1226;
  --bg-2: #15142f;
  --bg-3: #0c1424;
  --blob-1: rgba(79, 70, 229, 0.32);
  --blob-2: rgba(147, 51, 234, 0.22);
  --blob-3: rgba(37, 99, 235, 0.24);
  --wave-a: rgba(99, 102, 241, 0.18);
  --wave-b: rgba(139, 92, 246, 0.13);
  --line: rgba(165, 180, 252, 0.2);
  --disc: rgba(99, 102, 241, 0.14);
  --ring: rgba(165, 180, 252, 0.2);
  --tile: rgba(255, 255, 255, 0.035);
}

/* ───── 配色：青绿 / 薄荷（清新） ───── */
.ambient[data-palette='mint'] {
  --bg-1: #eafaf4;
  --bg-2: #e4f6f6;
  --bg-3: #f1fbef;
  --blob-1: rgba(52, 211, 153, 0.42);
  --blob-2: rgba(56, 189, 248, 0.3);
  --blob-3: rgba(45, 212, 191, 0.38);
  --wave-a: rgba(45, 212, 191, 0.26);
  --wave-b: rgba(56, 189, 248, 0.18);
  --line: rgba(13, 148, 136, 0.32);
  --disc: rgba(110, 231, 183, 0.34);
  --ring: rgba(13, 148, 136, 0.28);
  --tile: rgba(255, 255, 255, 0.42);
}
html[data-theme='dark'] .ambient[data-palette='mint'] {
  --bg-1: #08171a;
  --bg-2: #0a1d1e;
  --bg-3: #0a1820;
  --blob-1: rgba(16, 185, 129, 0.24);
  --blob-2: rgba(14, 116, 144, 0.3);
  --blob-3: rgba(20, 184, 166, 0.2);
  --wave-a: rgba(20, 184, 166, 0.16);
  --wave-b: rgba(14, 165, 233, 0.12);
  --line: rgba(94, 234, 212, 0.18);
  --disc: rgba(20, 184, 166, 0.12);
  --ring: rgba(94, 234, 212, 0.18);
  --tile: rgba(255, 255, 255, 0.035);
}

/* ───── 配色：暖橙 / 珊瑚粉（温暖） ───── */
.ambient[data-palette='coral'] {
  --bg-1: #fff3ec;
  --bg-2: #ffedf0;
  --bg-3: #fff6e8;
  --blob-1: rgba(251, 146, 60, 0.4);
  --blob-2: rgba(251, 113, 133, 0.36);
  --blob-3: rgba(251, 191, 36, 0.3);
  --wave-a: rgba(251, 146, 60, 0.24);
  --wave-b: rgba(244, 114, 182, 0.18);
  --line: rgba(234, 88, 12, 0.28);
  --disc: rgba(253, 164, 175, 0.36);
  --ring: rgba(225, 29, 72, 0.22);
  --tile: rgba(255, 255, 255, 0.42);
}
html[data-theme='dark'] .ambient[data-palette='coral'] {
  --bg-1: #1b1012;
  --bg-2: #211219;
  --bg-3: #1c140e;
  --blob-1: rgba(234, 88, 12, 0.22);
  --blob-2: rgba(219, 39, 119, 0.2);
  --blob-3: rgba(217, 119, 6, 0.16);
  --wave-a: rgba(249, 115, 22, 0.14);
  --wave-b: rgba(236, 72, 153, 0.11);
  --line: rgba(253, 186, 116, 0.18);
  --disc: rgba(244, 63, 94, 0.12);
  --ring: rgba(253, 186, 116, 0.18);
  --tile: rgba(255, 255, 255, 0.035);
}
</style>
