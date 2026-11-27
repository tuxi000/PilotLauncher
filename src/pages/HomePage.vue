<script setup lang="ts">
// 首页：大背景 + 底部玻璃信息条 + 右下角大号玻璃启动按钮（内置背景时问候语使用主题文字颜色，自定义背景图时用白字 + 阴影）
//  启动按钮的动效：按下回弹；播放图标收起变成进度环；状态文字按“准备中 → 下载文件 → 启动中 → 运行中”平滑切换；
//  游戏启动成功时一次光晕扩散和粒子。账户、实例都可以在信息条上直接切换（弹出玻璃小列表）。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import Icon from '../components/Icon.vue'
import Avatar from '../components/Avatar.vue'
import InstanceIcon from '../components/InstanceIcon.vue'
import { relativeTime, VERSION_TYPE_LABEL, type AccountView } from '../api'
import {
  store,
  currentInstance,
  currentAccount,
  gameState,
  launchInstance,
  killInstance,
  openAccountPanel,
  openLog,
  selectAccount,
  selectInstance,
  taskList,
} from '../store'
import { roomSummary } from '../terracotta'
import { motionOn } from '../motion'

const picker = ref(false)
const accPicker = ref(false)
/** 联机房间进行中时在问候语下方显示一个小胶囊，点击进入联机页 */
const room = computed(() => roomSummary(store.terracotta?.room))
const inst = currentInstance
/** 没有自定义背景图：显示内置的浅 / 深色渐变背景 */
const ambient = computed(() => !store.backgroundUrl)
const state = computed(() => gameState(inst.value?.name))
const preparing = computed(() => state.value?.state === 'preparing')
const running = computed(() => state.value?.state === 'running')

// 启动过程中出现的补全文件 / Java 下载任务
const launchTask = computed(() =>
  taskList.value.find((t) => t.status === 'running' && (t.kind === 'launch' || t.kind === 'java' || t.kind === 'install')),
)

type Phase = 'idle' | 'prepare' | 'download' | 'start' | 'running'
/** 启动阶段：后端的 preparing 消息依次是 检查账户 → 检查游戏文件 → 准备 Java → 验证账户 → 解压本地库 → 启动游戏进程 */
const phase = computed<Phase>(() => {
  if (running.value) return 'running'
  if (!preparing.value) return 'idle'
  if (launchTask.value) return 'download'
  const m = state.value?.message ?? ''
  if (/解压|启动游戏进程/.test(m)) return 'start'
  return 'prepare'
})
const busy = computed(() => phase.value !== 'idle' && phase.value !== 'running')

const mainText = computed(() => {
  if (!inst.value) return '安装游戏'
  switch (phase.value) {
    case 'prepare':
      return '准备中'
    case 'download':
      return '下载文件'
    case 'start':
      return '启动中'
    case 'running':
      return '运行中'
    default:
      return '启动游戏'
  }
})
const subText = computed(() => {
  if (!inst.value) return '选择一个版本开始'
  switch (phase.value) {
    case 'running':
      return '点击查看日志'
    case 'download':
      return launchTask.value?.stage || '正在下载'
    case 'prepare':
    case 'start':
      return state.value?.message || '请稍候'
    default:
      return `${inst.value.name} · ${inst.value.mcVersion}`
  }
})

// 进度环：下载阶段按任务进度画弧，其余阶段是一段旋转的弧
const RING_C = 2 * Math.PI * 14
const determinate = computed(() => phase.value === 'download' && (launchTask.value?.progress ?? 0) > 0)
const arcStyle = computed(() => {
  const p = determinate.value ? Math.max(0.04, Math.min(1, launchTask.value?.progress ?? 0)) : 0.28
  return { strokeDasharray: `${RING_C}`, strokeDashoffset: `${RING_C * (1 - p)}` }
})

// 启动成功：一次光晕 + 粒子
const burst = ref(0)
let burstTimer: number | undefined
watch(phase, (now, prev) => {
  if (now === 'running' && prev !== 'running' && prev !== 'idle' && motionOn()) {
    burst.value++
    window.clearTimeout(burstTimer)
    burstTimer = window.setTimeout(() => (burst.value = 0), 1500)
  }
})
const SPARKS = 12
function sparkStyle(n: number) {
  const a = (n / SPARKS) * Math.PI * 2 + (n % 2 ? 0.18 : -0.12)
  const d = 46 + ((n * 37) % 26)
  return {
    '--dx': `${Math.cos(a) * d}px`,
    '--dy': `${Math.sin(a) * d * 0.8}px`,
    '--delay': `${(n % 4) * 25}ms`,
    '--size': `${4 + (n % 3)}px`,
  }
}

// 点击启动时图标“踢”一下
const kick = ref(false)
let kickTimer: number | undefined
function doKick() {
  if (!motionOn()) return
  kick.value = false
  window.clearTimeout(kickTimer)
  requestAnimationFrame(() => {
    kick.value = true
    kickTimer = window.setTimeout(() => (kick.value = false), 650)
  })
}
onBeforeUnmount(() => {
  window.clearTimeout(burstTimer)
  window.clearTimeout(kickTimer)
})

const greeting = computed(() => {
  const h = new Date().getHours()
  const part = h < 6 ? '夜深了' : h < 11 ? '早上好' : h < 14 ? '中午好' : h < 18 ? '下午好' : '晚上好'
  return currentAccount.value ? `${part}，${currentAccount.value.name}` : `${part}，欢迎使用 PilotLauncher`
})

function primary() {
  if (!inst.value) {
    store.page = 'download'
    store.downloadTab = 'versions'
    return
  }
  if (!currentAccount.value) {
    openAccountPanel()
    return
  }
  if (running.value || preparing.value) {
    openLog(inst.value.name)
    return
  }
  doKick()
  launchInstance(inst.value.name)
}

function closePops() {
  picker.value = false
  accPicker.value = false
}

function toggleInstances() {
  accPicker.value = false
  picker.value = !picker.value
}

async function choose(name: string) {
  picker.value = false
  await selectInstance(name)
}

// 账户小列表：只切换，不做管理（管理在账户面板里）
function toggleAccounts(e: MouseEvent) {
  picker.value = false
  if (!store.accounts.length) {
    openAccountPanel(e.currentTarget as Element | null)
    return
  }
  accPicker.value = !accPicker.value
}

function chooseAccount(a: AccountView) {
  accPicker.value = false
  selectAccount(a.id)
}

function manageAccounts(e: MouseEvent) {
  accPicker.value = false
  openAccountPanel(e.currentTarget as Element | null)
}

function accountKind(a: AccountView | null | undefined) {
  if (!a) return '离线 / 外置登录'
  return a.kind === 'offline' ? '离线账户' : (a.serverName ?? '外置登录')
}
</script>

<template>
  <section class="home" :class="{ ambient }">
    <div class="hero stagger">
      <h1 class="greet">{{ greeting }}</h1>
      <p class="tag">在方块世界里，开启一段清透的冒险。</p>
      <Transition name="pop">
        <button v-if="room" class="mp-pill glass" :title="'查看联机房间'" @click="store.page = 'multiplayer'">
          <span class="mp-dot" :class="{ ok: room.live }" />
          <Icon name="network" :size="15" />
          <span class="ellipsis">{{ room.text }}</span>
        </button>
      </Transition>
    </div>

    <div class="dock stagger">
      <!-- 账户与实例信息 -->
      <div class="info glass" v-refract v-light>
        <div class="picker-wrap acc-wrap">
          <button class="chip" :class="{ open: accPicker }" @click="toggleAccounts">
            <Avatar :name="currentAccount?.name ?? '?'" :seed="currentAccount?.uuid ?? ''" :account="currentAccount" :size="40" />
            <span class="chip-text">
              <span class="k">账户</span>
              <span class="v ellipsis">{{ currentAccount?.name ?? (store.accounts.length ? '选择账户' : '添加账户') }}</span>
              <span class="s faint ellipsis">{{ accountKind(currentAccount) }}</span>
            </span>
            <Icon v-if="store.accounts.length" name="chevronDown" :size="16" class="faint chev" />
          </button>
          <Transition name="pop">
            <div v-if="accPicker" class="popover glass glass-strong" v-light>
              <button
                v-for="(a, i) in store.accounts"
                :key="a.id"
                class="opt acc-opt"
                :class="{ active: a.id === currentAccount?.id }"
                :style="{ '--i': i }"
                @click="chooseAccount(a)"
              >
                <Avatar :name="a.name" :seed="a.uuid" :account="a" :size="30" />
                <span class="opt-text">
                  <span class="ellipsis">{{ a.name }}</span>
                  <span class="faint ellipsis opt-sub">{{ accountKind(a) }}</span>
                </span>
                <Icon v-if="a.id === currentAccount?.id" name="check" :size="15" :stroke="2.4" class="opt-check" />
              </button>
              <div class="pop-sep" />
              <button class="opt manage" :style="{ '--i': store.accounts.length }" @click="manageAccounts">
                <span class="manage-ico"><Icon name="settings" :size="15" /></span>
                <span>管理账户</span>
                <Icon name="chevron" :size="14" class="faint" />
              </button>
            </div>
          </Transition>
        </div>
        <span class="vsep" />
        <div class="picker-wrap">
          <button class="chip" :class="{ open: picker }" :disabled="store.instances.length === 0" @click="toggleInstances">
            <InstanceIcon :name="inst?.name ?? ''" :icon="inst?.icon" :loader="inst?.loader?.kind" :version-type="inst?.versionType" :size="44" />
            <span class="chip-text">
              <span class="k">实例</span>
              <span class="v ellipsis">{{ inst?.name ?? '尚未选择实例' }}</span>
              <span class="s faint ellipsis">
                {{ inst ? `${inst.mcVersion} · ${VERSION_TYPE_LABEL[inst.versionType] ?? inst.versionType} · ${relativeTime(inst.lastPlayed)}` : '前往下载页安装游戏' }}
              </span>
            </span>
            <Icon v-if="store.instances.length" name="chevronDown" :size="16" class="faint chev" />
          </button>
          <Transition name="pop">
            <div v-if="picker" class="popover glass glass-strong" v-light>
              <button
                v-for="(i, idx) in store.instances"
                :key="i.name"
                class="opt"
                :class="{ active: i.name === inst?.name }"
                :style="{ '--i': idx }"
                @click="choose(i.name)"
              >
                <InstanceIcon :name="i.name" :icon="i.icon" :loader="i.loader?.kind" :version-type="i.versionType" :size="30" />
                <span class="ellipsis">{{ i.name }}</span>
                <span class="faint">{{ i.mcVersion }}</span>
              </button>
            </div>
          </Transition>
        </div>
      </div>

      <!-- 启动按钮 -->
      <div class="launch-wrap">
        <Transition name="lp">
          <div v-if="launchTask" class="launch-progress glass">
            <span class="ellipsis">{{ launchTask.stage }}</span>
            <div class="progress flowing"><i :style="{ width: `${Math.round(launchTask.progress * 100)}%` }" /></div>
          </div>
        </Transition>
        <button
          class="launch glass"
          :class="[`ph-${phase}`, { running, preparing, kick }]"
          v-refract="{ scale: 44, edge: 22 }"
          v-light
          @click="primary"
        >
          <span class="launch-icon">
            <span class="li li-play"><Icon :name="inst ? 'play' : 'download'" :size="26" :stroke="2" /></span>
            <span class="li li-ring">
              <svg class="ring-svg" :class="{ 'keep-spin': !determinate }" viewBox="0 0 36 36" aria-hidden="true">
                <circle class="ring-track" cx="18" cy="18" r="14" />
                <circle class="ring-arc" cx="18" cy="18" r="14" :style="arcStyle" />
              </svg>
            </span>
            <span class="li li-run"><Icon name="log" :size="24" :stroke="2" /></span>
            <span class="heartbeat" />
          </span>
          <span class="launch-text">
            <Transition name="swap" mode="out-in">
              <span :key="mainText" class="main">{{ mainText }}</span>
            </Transition>
            <Transition name="swap-sub" mode="out-in">
              <span :key="subText" class="sub ellipsis">{{ subText }}</span>
            </Transition>
          </span>
          <span v-if="burst" :key="burst" class="burst" aria-hidden="true">
            <i class="halo" />
            <i class="halo late" />
            <i v-for="n in SPARKS" :key="n" class="spark" :class="{ alt: n % 3 === 0 }" :style="sparkStyle(n)" />
          </span>
        </button>
        <Transition name="pop">
          <button v-if="running && inst" class="stop btn danger sm" @click="killInstance(inst.name)">
            <Icon name="stop" :size="14" /> 结束游戏
          </button>
        </Transition>
      </div>
    </div>
    <div v-if="picker || accPicker" class="click-away" @click="closePops" />
    <span v-if="busy" class="sr-only" aria-live="polite">{{ mainText }}</span>
  </section>
</template>

<style scoped>
.home {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 36px 34px 30px;
  color: #fff;
}
.hero {
  text-shadow: 0 2px 18px rgba(0, 0, 0, 0.35);
}
.home.ambient {
  color: var(--text);
}
.home.ambient .hero {
  text-shadow: none;
}
.home.ambient .tag {
  color: var(--text-2);
  opacity: 1;
}
.greet {
  font-size: 34px;
  font-weight: 700;
}
.tag {
  margin: 8px 0 0;
  font-size: 15px;
  opacity: 0.86;
}
.mp-pill {
  --radius: 16px;
  --blur: 18px;
  margin-top: 14px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 420px;
  height: 32px;
  padding: 0 14px 0 12px;
  border: none;
  cursor: pointer;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
  text-shadow: none;
  transition: transform 0.45s var(--spring);
}
.mp-pill:hover {
  transform: translateY(-1px) scale(1.03);
}
.mp-pill:active {
  transform: scale(0.96);
}
.mp-dot {
  width: 8px;
  height: 8px;
  flex: none;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 8px var(--accent);
}
.mp-dot.ok {
  background: var(--success);
  box-shadow: 0 0 8px var(--success);
}
.dock {
  display: flex;
  align-items: flex-end;
  gap: 18px;
}
.info {
  --radius: 26px;
  flex: 1;
  min-width: 0;
  max-width: 640px;
  display: flex;
  align-items: center;
  padding: 10px;
  color: var(--text);
}
.chip {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1;
  padding: 8px 12px;
  border: none;
  border-radius: 18px;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.2s ease, transform 0.45s var(--spring);
}
.chip:hover:not(:disabled),
.chip.open {
  background: var(--glass-bg-hover);
}
.chip:active:not(:disabled) {
  transform: scale(0.97);
}
.chip:disabled {
  cursor: default;
}
.chev {
  transition: transform 0.45s var(--spring);
}
.chip.open .chev {
  transform: rotate(180deg);
}
.chip-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.k {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-3);
}
.v {
  font-size: 15px;
  font-weight: 650;
}
.s {
  font-size: 12px;
}
.vsep {
  width: 1px;
  align-self: stretch;
  margin: 8px 4px;
  background: var(--divider);
}
.picker-wrap {
  position: relative;
  flex: 1.3;
  min-width: 0;
  display: flex;
}
.picker-wrap.acc-wrap {
  flex: 1;
}
.popover {
  --radius: 20px;
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 14px);
  z-index: 20;
  min-width: 240px;
  max-height: 320px;
  overflow: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  transform-origin: 50% 100%;
}
.opt {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border: none;
  border-radius: 14px;
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease, transform 0.4s var(--spring);
  animation: rise-in 0.32s var(--ease-out) both;
  animation-delay: calc(min(var(--i, 0), 8) * 22ms);
}
.opt:hover {
  background: var(--glass-bg-hover);
}
.opt:active {
  transform: scale(0.98);
}
.opt.active {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
}
.opt-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.3;
}
.opt-sub {
  font-size: 11px;
}
.opt-check {
  color: var(--accent);
}
.pop-sep {
  height: 1px;
  margin: 4px 8px;
  background: var(--divider);
  flex: none;
}
.opt.manage {
  font-size: 13px;
  font-weight: 600;
  color: var(--accent);
}
.manage-ico {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
.click-away {
  position: fixed;
  inset: 0;
  z-index: 10;
}
.info,
.launch-wrap {
  position: relative;
  z-index: 11;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}

.launch-wrap {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 10px;
}
.launch-progress {
  --radius: 14px;
  width: 300px;
  padding: 8px 12px;
  font-size: 12px;
  color: var(--text);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.launch {
  --radius: 30px;
  --blur: 26px;
  width: 300px;
  height: 92px;
  border: none;
  padding: 0 22px;
  display: flex;
  align-items: center;
  gap: 16px;
  cursor: pointer;
  color: var(--text);
  text-align: left;
  background: linear-gradient(140deg, rgba(255, 255, 255, 0.35), rgba(255, 255, 255, 0.08)),
    color-mix(in srgb, var(--accent) 26%, var(--glass-bg));
  transition: transform 0.6s var(--spring), box-shadow 0.3s ease, background 0.3s ease;
  box-shadow: 0 18px 40px -14px color-mix(in srgb, var(--accent) 70%, rgba(0, 0, 0, 0.4)),
    inset 0 1px 0 var(--hl-top), inset 0 -1px 0 var(--hl-bottom);
}
.launch:hover {
  transform: translateY(-3px) scale(1.035);
}
.launch:active {
  transform: scale(0.95);
  transition-duration: 0.15s;
}
.launch.running {
  background: linear-gradient(140deg, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.06)),
    color-mix(in srgb, var(--success) 30%, var(--glass-bg));
  box-shadow: 0 18px 40px -14px color-mix(in srgb, var(--success) 70%, transparent), inset 0 1px 0 var(--hl-top);
}
.launch-icon {
  position: relative;
  width: 54px;
  height: 54px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 20px;
  color: #fff;
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 70%, white), var(--accent));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6), 0 8px 18px -6px var(--accent);
  transition: transform 0.6s var(--spring);
}
.launch.running .launch-icon {
  background: linear-gradient(160deg, color-mix(in srgb, var(--success) 70%, white), var(--success));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6), 0 8px 18px -6px var(--success);
}
.launch:hover .launch-icon {
  transform: rotate(-6deg) scale(1.06);
}
.launch.kick .launch-icon {
  animation: launch-kick 0.6s var(--spring);
}
@keyframes launch-kick {
  0% {
    transform: scale(0.82) rotate(8deg);
  }
  100% {
    transform: none;
  }
}

/* 图标的三层：播放 → 进度环 → 运行中（只改 transform / opacity） */
.li {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  transition: transform 0.55s var(--spring), opacity 0.22s ease;
}
.li-play {
  transform: none;
  opacity: 1;
}
.launch:hover .li-play :deep(.icon) {
  transform: translateX(2px);
}
.li-play :deep(.icon) {
  transition: transform 0.45s var(--spring);
}
.li-ring,
.li-run {
  opacity: 0;
  transform: scale(0.4) rotate(-90deg);
}
.launch.preparing .li-play,
.launch.running .li-play {
  opacity: 0;
  transform: scale(0.3) rotate(90deg);
}
.launch.preparing .li-ring {
  opacity: 1;
  transform: none;
}
.launch.running .li-run {
  opacity: 1;
  transform: none;
}
.ring-svg {
  width: 34px;
  height: 34px;
  transform: rotate(-90deg);
}
.ring-svg.keep-spin {
  animation: ring-spin 1s linear infinite;
}
@keyframes ring-spin {
  from {
    transform: rotate(-90deg);
  }
  to {
    transform: rotate(270deg);
  }
}
.ring-track {
  fill: none;
  stroke: rgba(255, 255, 255, 0.32);
  stroke-width: 3.5;
}
.ring-arc {
  fill: none;
  stroke: #fff;
  stroke-width: 3.5;
  stroke-linecap: round;
}
/* 运行中：图标外一圈缓慢的“心跳” */
.heartbeat {
  position: absolute;
  inset: -1px;
  border-radius: inherit;
  pointer-events: none;
  box-shadow: 0 0 0 2px var(--success);
  opacity: 0;
}
.launch.running .heartbeat {
  animation: heartbeat 2.4s var(--ease-out) infinite;
}
@keyframes heartbeat {
  0% {
    opacity: 0.6;
    transform: scale(1);
  }
  70%,
  100% {
    opacity: 0;
    transform: scale(1.45);
  }
}

.launch-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.main {
  font-size: 21px;
  font-weight: 700;
  letter-spacing: 0.02em;
}
.sub {
  font-size: 12.5px;
  color: var(--text-2);
  margin-top: 2px;
}
.stop {
  margin-right: 6px;
}

/* 启动成功：光晕 + 粒子（从图标中心发出） */
.burst {
  position: absolute;
  left: 49px;
  top: 50%;
  width: 0;
  height: 0;
  pointer-events: none;
  z-index: 2;
}
.halo {
  position: absolute;
  left: -30px;
  top: -30px;
  width: 60px;
  height: 60px;
  border-radius: 50%;
  background: radial-gradient(closest-side, color-mix(in srgb, var(--success) 55%, transparent), transparent);
  opacity: 0;
  animation: halo 0.9s var(--ease-out) both;
}
.halo.late {
  background: none;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--success) 70%, white);
  animation-delay: 0.08s;
}
@keyframes halo {
  0% {
    opacity: 0.85;
    transform: scale(0.6);
  }
  100% {
    opacity: 0;
    transform: scale(3.2);
  }
}
.spark {
  position: absolute;
  left: calc(var(--size) / -2);
  top: calc(var(--size) / -2);
  width: var(--size);
  height: var(--size);
  border-radius: 50%;
  background: var(--success);
  box-shadow: 0 0 6px var(--success);
  opacity: 0;
  animation: spark 0.85s var(--ease-out) both;
  animation-delay: var(--delay);
}
.spark.alt {
  background: #fff;
  box-shadow: 0 0 6px rgba(255, 255, 255, 0.9);
}
@keyframes spark {
  0% {
    opacity: 1;
    transform: translate(0, 0) scale(1);
  }
  100% {
    opacity: 0;
    transform: translate(var(--dx), var(--dy)) scale(0.2);
  }
}

/* 主文字：上下滑动切换；副文字：只淡入淡出 + 很小的位移 */
.swap-enter-active {
  transition: opacity 0.22s ease, transform 0.45s var(--spring);
}
.swap-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.swap-enter-from {
  opacity: 0;
  transform: translateY(10px);
}
.swap-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
.swap-sub-enter-active {
  transition: opacity 0.2s ease, transform 0.3s var(--ease-out);
}
.swap-sub-leave-active {
  transition: opacity 0.1s ease;
}
.swap-sub-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.swap-sub-leave-to {
  opacity: 0;
}

.lp-enter-active {
  transition: transform 0.5s var(--spring), opacity 0.2s ease;
}
.lp-leave-active {
  transition: transform 0.18s ease, opacity 0.18s ease;
}
.lp-enter-from,
.lp-leave-to {
  opacity: 0;
  transform: translateY(10px) scale(0.96);
}

.pop-enter-active {
  transition: transform 0.5s var(--spring), opacity 0.2s ease;
}
.pop-leave-active {
  transition: transform 0.15s ease, opacity 0.15s ease;
}
.pop-enter-from,
.pop-leave-to {
  transform: translateY(10px) scale(0.95);
  opacity: 0;
}
</style>
