<script setup lang="ts">
// 左侧窄图标导航（LauncherX 式），玻璃指示块以弹簧动画在项目间滑动（移动时轻微拉伸）
import { computed } from 'vue'
import Icon from './Icon.vue'
import Avatar from './Avatar.vue'
import { store, currentAccount, openAccountPanel, runningTaskCount, type Page } from '../store'
import { roomSummary } from '../terracotta'

const items: { page: Page; icon: string; label: string }[] = [
  { page: 'home', icon: 'home', label: '首页' },
  { page: 'download', icon: 'download', label: '下载' },
  { page: 'instances', icon: 'cube', label: '实例' },
  { page: 'multiplayer', icon: 'network', label: '联机' },
  { page: 'settings', icon: 'settings', label: '设置' },
]

/** 联机房间进行中：导航项上显示一个小圆点（已连通为绿色，连接中为蓝色） */
const room = computed(() => roomSummary(store.terracotta?.room))

const ITEM = 62
const index = computed(() => Math.max(0, items.findIndex((i) => i.page === store.page)))

function openPanel(e: MouseEvent) {
  if (store.accountPanel) {
    store.accountPanel = false
    return
  }
  openAccountPanel(e.currentTarget as Element | null)
}
</script>

<template>
  <nav class="sidenav">
    <div class="rail glass" v-refract="{ scale: 28 }" v-light>
      <div class="indicator" :style="{ transform: `translateY(${index * ITEM}px)` }">
        <!-- 每次切换重新挂载，播放一次“拉伸 → 回弹” -->
        <span :key="index" class="blob" />
      </div>
      <button
        v-for="item in items"
        :key="item.page"
        class="item"
        :class="{ active: store.page === item.page }"
        :title="item.label"
        @click="store.page = item.page"
      >
        <span class="ico">
          <Icon :name="item.icon" :size="22" />
          <span v-if="item.page === 'download' && runningTaskCount > 0" class="dot">{{ runningTaskCount }}</span>
          <span
            v-if="item.page === 'multiplayer' && room"
            class="live"
            :class="{ ok: room.live }"
            :title="room.text"
          />
        </span>
        <span class="label">{{ item.label }}</span>
      </button>
    </div>
    <div class="spacer" />
    <button
      class="account glass"
      :class="{ open: store.accountPanel }"
      v-light
      :title="currentAccount ? `${currentAccount.name}（账户）` : '添加账户'"
      @click="openPanel"
    >
      <Avatar :name="currentAccount?.name ?? '?'" :seed="currentAccount?.uuid ?? ''" :account="currentAccount" :size="38" />
    </button>
  </nav>
</template>

<style scoped>
.sidenav {
  width: 84px;
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 6px 0 16px;
}
.rail {
  --radius: 26px;
  --blur: 26px;
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 6px;
  width: 68px;
}
.indicator {
  position: absolute;
  left: 6px;
  top: 6px;
  width: 56px;
  height: 56px;
  margin-top: 3px;
  transition: transform 0.6s var(--spring);
}
.blob {
  position: absolute;
  inset: 0;
  border-radius: 18px;
  background: var(--glass-bg-strong);
  box-shadow: 0 6px 16px -6px rgba(0, 0, 0, 0.28), inset 0 1px 0 var(--hl-top), inset 0 0 0 1px var(--glass-edge);
  animation: nav-stretch 0.6s var(--spring) both;
}
@keyframes nav-stretch {
  0% {
    transform: scale(0.92, 1.16);
  }
  100% {
    transform: none;
  }
}
.item {
  position: relative;
  z-index: 1;
  width: 56px;
  height: 62px;
  border: none;
  background: transparent;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  color: var(--text-2);
  transition: color 0.2s ease, transform 0.45s var(--spring);
}
.item:hover {
  color: var(--text);
  transform: scale(1.06);
}
.item:active {
  transform: scale(0.9);
  transition-duration: 0.1s;
}
.item.active {
  color: var(--accent);
}
.ico {
  position: relative;
  transition: transform 0.45s var(--spring);
}
/* 图标悬停时轻轻上浮、侧一点 */
.item:hover .ico {
  transform: translateY(-2px) rotate(-6deg);
}
/* 选中时图标弹一下 */
.item.active .ico {
  animation: nav-pop 0.55s var(--spring);
}
@keyframes nav-pop {
  0% {
    transform: scale(0.7);
  }
  100% {
    transform: none;
  }
}
.dot {
  position: absolute;
  top: -6px;
  right: -10px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 700;
  line-height: 16px;
  color: #fff;
  background: var(--danger);
  box-shadow: 0 2px 6px rgba(255, 69, 58, 0.5);
  animation: nav-pop 0.5s var(--spring);
}
.live {
  position: absolute;
  top: -3px;
  right: -5px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 2px var(--glass-bg-strong), 0 0 8px var(--accent);
}
.live.ok {
  background: var(--success);
  box-shadow: 0 0 0 2px var(--glass-bg-strong), 0 0 8px var(--success);
}
.label {
  font-size: 11px;
  font-weight: 600;
}
.account {
  --radius: 50%;
  width: 52px;
  height: 52px;
  border: none;
  padding: 0;
  display: grid;
  place-items: center;
  cursor: pointer;
  transition: transform 0.45s var(--spring), box-shadow 0.25s ease;
}
.account:hover {
  transform: scale(1.08);
}
.account:active {
  transform: scale(0.92);
}
.account.open {
  box-shadow: var(--glass-shadow), 0 0 0 2px var(--accent), 0 0 18px -4px var(--accent);
}
.account :deep(.avatar) {
  transition: transform 0.45s var(--spring);
}
.account:hover :deep(.avatar) {
  transform: rotate(-4deg);
}
</style>
