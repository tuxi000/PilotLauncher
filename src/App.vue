<script setup lang="ts">
import { computed, onMounted, type Component } from 'vue'
import TitleBar from './components/TitleBar.vue'
import SideNav from './components/SideNav.vue'
import ToastHost from './components/ToastHost.vue'
import AccountDialog from './components/AccountDialog.vue'
import LogDrawer from './components/LogDrawer.vue'
import CrashDialog from './components/CrashDialog.vue'
import InstanceDetail from './components/InstanceDetail.vue'
import ImportDialog from './components/ImportDialog.vue'
import MissingFilesDialog from './components/MissingFilesDialog.vue'
import Icon from './components/Icon.vue'
import AmbientBackground from './components/AmbientBackground.vue'
import HomePage from './pages/HomePage.vue'
import DownloadPage from './pages/DownloadPage.vue'
import InstancesPage from './pages/InstancesPage.vue'
import SettingsPage from './pages/SettingsPage.vue'
import MultiplayerPage from './pages/MultiplayerPage.vue'
import { store, init, type Page } from './store'

const pages: Record<Page, Component> = {
  home: HomePage,
  download: DownloadPage,
  instances: InstancesPage,
  multiplayer: MultiplayerPage,
  settings: SettingsPage,
}
const current = computed(() => pages[store.page])

onMounted(init)
</script>

<template>
  <div class="shell" :class="{ home: store.page === 'home', 'custom-bg': !!store.backgroundUrl }">
    <TitleBar />
    <div class="body">
      <SideNav />
      <main class="sheet">
        <!-- 自定义背景图：首页清晰全显，其它页面虚化淡出，让窗口的 Mica 透出来 -->
        <div v-if="store.backgroundUrl" class="backdrop" :style="{ backgroundImage: `url(${store.backgroundUrl})` }" />
        <!-- 内置背景：本身已经很柔和，其它页面只降低不透明度（不加模糊，避免对动画层做逐帧滤镜） -->
        <div v-else class="ambient-host">
          <AmbientBackground
            :palette="store.settings?.backgroundPalette ?? 'blue'"
            :animated="store.settings?.backgroundAnimation ?? true"
          />
        </div>
        <div class="veil" />
        <div class="page-host">
          <Transition name="page" mode="out-in">
            <KeepAlive>
              <component :is="current" />
            </KeepAlive>
          </Transition>
        </div>
      </main>
    </div>
    <AccountDialog />
    <LogDrawer />
    <CrashDialog />
    <InstanceDetail />
    <ImportDialog />
    <MissingFilesDialog />
    <ToastHost />
    <!-- 拖放导入提示 -->
    <Transition name="drop">
      <div v-if="store.dragOver" class="drop-overlay">
        <div class="drop-card glass glass-strong" v-refract="{ scale: 30 }">
          <Icon name="upload" :size="40" />
          <div class="drop-title">松开以导入</div>
          <div class="muted">地图 zip（解压到实例的 saves）· 整合包 .mrpack / CurseForge zip（创建新实例）</div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.body {
  flex: 1;
  min-height: 0;
  display: flex;
}
.sheet {
  position: relative;
  flex: 1;
  min-width: 0;
  margin: 0 12px 12px 0;
  border-radius: 24px;
  overflow: hidden;
  background: var(--sheet-bg);
  box-shadow: inset 0 0 0 1px var(--glass-edge), inset 0 1px 0 var(--hl-top), 0 10px 40px -20px rgba(0, 0, 0, 0.4);
  isolation: isolate;
}
.backdrop {
  position: absolute;
  inset: -40px;
  z-index: -2;
  background-size: cover;
  background-position: center;
  opacity: 0.32;
  filter: blur(36px) saturate(1.4);
  transform: scale(1.08);
  transition: opacity 0.6s var(--ease-out), filter 0.7s var(--ease-out), transform 0.9s var(--spring);
}
.home .backdrop {
  opacity: 1;
  filter: none;
  transform: scale(1);
}
.ambient-host {
  position: absolute;
  inset: 0;
  z-index: -2;
  opacity: 0.55;
  transition: opacity 0.6s var(--ease-out);
}
.home .ambient-host {
  opacity: 1;
}
/* 其它页面有大量 backdrop-filter 玻璃卡片：背景暂停（保留当前位置，回到首页继续流动），
   避免背景每帧变化导致所有玻璃层每帧重新模糊 */
.shell:not(.home) .ambient-host :deep(.anim) {
  animation-play-state: paused;
}
.veil {
  position: absolute;
  inset: 0;
  z-index: -1;
  pointer-events: none;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0.18), transparent 30%, transparent 62%, rgba(0, 0, 0, 0.28));
  opacity: 0;
  transition: opacity 0.5s ease;
}
/* 暗角只用于自定义背景图（照片上的白色问候语需要它），内置背景不需要 */
.home.custom-bg .veil {
  opacity: 1;
}
.page-host {
  position: relative;
  height: 100%;
}
.page-host > * {
  height: 100%;
}
.drop-overlay {
  position: fixed;
  inset: 0;
  z-index: 300;
  display: grid;
  place-items: center;
  pointer-events: none;
  background: var(--scrim);
}
.drop-card {
  --radius: 30px;
  --blur: 30px;
  padding: 34px 46px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  color: var(--accent);
  text-align: center;
  box-shadow: var(--glass-shadow), inset 0 0 0 2px color-mix(in srgb, var(--accent) 60%, transparent);
}
.drop-title {
  font-size: 20px;
  font-weight: 650;
  color: var(--text);
}
.drop-enter-active {
  transition: opacity 0.2s ease;
}
.drop-enter-active .drop-card {
  transition: transform 0.55s var(--spring);
}
.drop-leave-active {
  transition: opacity 0.15s ease;
}
.drop-enter-from,
.drop-leave-to {
  opacity: 0;
}
.drop-enter-from .drop-card {
  transform: scale(0.88);
}
</style>
