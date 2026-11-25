<script setup lang="ts">
// 首页：大背景 + 底部玻璃信息条 + 右下角大号玻璃启动按钮（内置背景时问候语使用主题文字颜色，自定义背景图时用白字 + 阴影）
import { computed, ref } from 'vue'
import Icon from '../components/Icon.vue'
import Avatar from '../components/Avatar.vue'
import InstanceIcon from '../components/InstanceIcon.vue'
import { relativeTime, VERSION_TYPE_LABEL } from '../api'
import {
  store,
  currentInstance,
  currentAccount,
  gameState,
  launchInstance,
  killInstance,
  openLog,
  selectInstance,
  taskList,
} from '../store'
import { roomSummary } from '../terracotta'

const picker = ref(false)
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
    store.accountDialog = true
    return
  }
  if (running.value || preparing.value) {
    openLog(inst.value.name)
    return
  }
  launchInstance(inst.value.name)
}

async function choose(name: string) {
  picker.value = false
  await selectInstance(name)
}
</script>

<template>
  <section class="home" :class="{ ambient }">
    <div class="hero">
      <h1 class="greet">{{ greeting }}</h1>
      <p class="tag">在方块世界里，开启一段清透的冒险。</p>
      <button v-if="room" class="mp-pill glass" :title="'查看联机房间'" @click="store.page = 'multiplayer'">
        <span class="mp-dot" :class="{ ok: room.live }" />
        <Icon name="network" :size="15" />
        <span class="ellipsis">{{ room.text }}</span>
      </button>
    </div>

    <div class="dock">
      <!-- 账户与实例信息 -->
      <div class="info glass" v-refract v-light>
        <button class="chip" @click="store.accountDialog = true">
          <Avatar :name="currentAccount?.name ?? '?'" :seed="currentAccount?.uuid ?? ''" :size="40" />
          <span class="chip-text">
            <span class="k">账户</span>
            <span class="v ellipsis">{{ currentAccount?.name ?? '点击添加账户' }}</span>
            <span class="s faint ellipsis">
              {{ currentAccount ? (currentAccount.kind === 'offline' ? '离线账户' : currentAccount.serverName ?? '外置登录') : '离线 / 外置登录' }}
            </span>
          </span>
        </button>
        <span class="vsep" />
        <div class="picker-wrap">
          <button class="chip" :disabled="store.instances.length === 0" @click="picker = !picker">
            <InstanceIcon :name="inst?.name ?? ''" :icon="inst?.icon" :loader="inst?.loader?.kind" :version-type="inst?.versionType" :size="44" />
            <span class="chip-text">
              <span class="k">实例</span>
              <span class="v ellipsis">{{ inst?.name ?? '尚未选择实例' }}</span>
              <span class="s faint ellipsis">
                {{ inst ? `${inst.mcVersion} · ${VERSION_TYPE_LABEL[inst.versionType] ?? inst.versionType} · ${relativeTime(inst.lastPlayed)}` : '前往下载页安装游戏' }}
              </span>
            </span>
            <Icon v-if="store.instances.length" name="chevronDown" :size="16" class="faint" />
          </button>
          <Transition name="pop">
            <div v-if="picker" class="popover glass glass-strong" v-light>
              <button
                v-for="i in store.instances"
                :key="i.name"
                class="opt"
                :class="{ active: i.name === inst?.name }"
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
        <div v-if="launchTask" class="launch-progress glass">
          <span class="ellipsis">{{ launchTask.stage }}</span>
          <div class="progress"><i :style="{ width: `${Math.round(launchTask.progress * 100)}%` }" /></div>
        </div>
        <button
          class="launch glass"
          :class="{ running, preparing }"
          v-refract="{ scale: 44, edge: 22 }"
          v-light
          @click="primary"
        >
          <span class="launch-icon">
            <span v-if="preparing" class="spinner" />
            <Icon v-else :name="running ? 'log' : inst ? 'play' : 'download'" :size="26" :stroke="2" />
          </span>
          <span class="launch-text">
            <span class="main">
              {{ !inst ? '安装游戏' : running ? '游戏运行中' : preparing ? '正在启动…' : '启动游戏' }}
            </span>
            <span class="sub ellipsis">
              {{ preparing ? state?.message : running ? '点击查看日志' : inst ? `${inst.name} · ${inst.mcVersion}` : '选择一个版本开始' }}
            </span>
          </span>
        </button>
        <button v-if="running && inst" class="stop btn danger sm" @click="killInstance(inst.name)">
          <Icon name="stop" :size="14" /> 结束游戏
        </button>
      </div>
    </div>
    <div v-if="picker" class="click-away" @click="picker = false" />
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
.chip:hover:not(:disabled) {
  background: var(--glass-bg-hover);
}
.chip:active:not(:disabled) {
  transform: scale(0.97);
}
.chip:disabled {
  cursor: default;
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
.popover {
  --radius: 20px;
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 14px);
  z-index: 20;
  max-height: 320px;
  overflow: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
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
  transition: background 0.15s ease;
}
.opt:hover {
  background: var(--glass-bg-hover);
}
.opt.active {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
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
.launch-icon .spinner {
  width: 24px;
  height: 24px;
  border-width: 3px;
}
.launch-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
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
