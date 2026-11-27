<script setup lang="ts">
// 账户面板：点侧边栏底部的头像弹出，贴着侧边栏停在窗口底部，大小约为窗口的四分之一，从头像的位置弹性展开。
//  左侧：当前账户的 3D 皮肤（skinview3d）和账户信息；右侧：账户列表（切换 / 删除）与“添加账户”表单。
//  点面板外面或按 Esc 关闭；关闭时 3D 预览随组件卸载，WebGL 上下文一并释放。
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, reactive, ref, watch } from 'vue'
import Avatar from './Avatar.vue'
import Icon from './Icon.vue'
import Segmented from './Segmented.vue'
import { api, errMsg, type AccountView, type GameProfile, type ServerInfo } from '../api'
import {
  store,
  currentAccount,
  closeAccountPanel,
  refreshAccounts,
  removeAccount,
  selectAccount,
  toast,
  toastError,
} from '../store'
import { loadSkin, resolveSkin, skinError, skinLoading } from '../skin'
import { motion } from '../motion'

// three.js 体积较大，打开面板时才加载
const SkinViewer3D = defineAsyncComponent(() => import('./SkinViewer3D.vue'))

const current = currentAccount
const skin = computed(() => resolveSkin(current.value))
const animated = computed(() => motion.enabled && !motion.reduced)

// ───── 位置：贴着侧边栏，底部与内容区对齐；从触发点缩放展开 ─────
const SIDE = 92
const BOTTOM = 12
const geo = reactive({ width: 560, height: 420, origin: '0px 100%' })

function layout() {
  const vw = window.innerWidth
  const vh = window.innerHeight
  geo.width = Math.round(Math.min(Math.max(vw * 0.5, 540), 760, vw - SIDE - 16))
  geo.height = Math.round(Math.min(Math.max(vh * 0.58, 420), 600, vh - 56))
  const top = vh - BOTTOM - geo.height
  const o = store.accountPanelOrigin ?? { x: 42, y: vh - 40 }
  geo.origin = `${Math.round(o.x - SIDE)}px ${Math.round(o.y - top)}px`
}

const panelStyle = computed(() => ({
  left: `${SIDE}px`,
  bottom: `${BOTTOM}px`,
  width: `${geo.width}px`,
  height: `${geo.height}px`,
  transformOrigin: geo.origin,
}))

// 3D 预览在展开动画结束后再创建，避免创建 WebGL 时卡住动画
const showViewer = ref(false)
const viewerFailed = ref(false)
const viewer = ref<{ wave: () => void; walk: () => void } | null>(null)

function onKey(e: KeyboardEvent) {
  if (e.key !== 'Escape') return
  if (adding.value) {
    adding.value = false
    return
  }
  close()
}

watch(
  () => store.accountPanel,
  (open) => {
    if (open) {
      layout()
      viewerFailed.value = false
      window.addEventListener('keydown', onKey)
      window.addEventListener('resize', layout)
      if (current.value) loadSkin(current.value)
      // 没有账户时直接打开添加表单
      adding.value = store.accounts.length === 0
    } else {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', layout)
      showViewer.value = false
      confirmId.value = null
    }
  },
)
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('resize', layout)
})

function afterEnter() {
  showViewer.value = true
}

function close() {
  closeAccountPanel()
}

function kindLabel(a: AccountView) {
  return a.kind === 'offline' ? '离线账户' : `外置登录 · ${a.serverName ?? a.serverUrl ?? ''}`
}

async function refreshSkin() {
  if (!current.value) return
  await loadSkin(current.value, true)
  const err = skinError(current.value.id)
  if (err) toast(`皮肤获取失败：${err}`, 'error')
}

// ───── 账户列表 ─────
const confirmId = ref<string | null>(null)
let confirmTimer: number | undefined

function choose(a: AccountView) {
  confirmId.value = null
  selectAccount(a.id)
}

async function askRemove(a: AccountView) {
  if (confirmId.value !== a.id) {
    confirmId.value = a.id
    window.clearTimeout(confirmTimer)
    confirmTimer = window.setTimeout(() => (confirmId.value = null), 3000)
    return
  }
  confirmId.value = null
  await removeAccount(a.id)
  toast(`已删除账户 ${a.name}`)
}

// ───── 添加账户 ─────
const adding = ref(false)
const mode = ref<'offline' | 'yggdrasil'>('offline')
const busy = ref(false)

function startAdd() {
  adding.value = true
  // 列表先淡出再换成表单（out-in），等表单插入后再聚焦
  window.setTimeout(() => {
    const el = document.querySelector<HTMLInputElement>('.ap-panel .add-pane input')
    el?.focus()
  }, 200)
}

// 离线
const offlineName = ref('')
const nameWarning = computed(() =>
  offlineName.value && !/^[A-Za-z0-9_]{3,16}$/.test(offlineName.value)
    ? '建议使用 3~16 位英文、数字或下划线，否则部分服务器可能进不去'
    : '',
)

async function addOffline() {
  if (!offlineName.value || busy.value) return
  busy.value = true
  try {
    const acc = await api.addOfflineAccount(offlineName.value)
    await refreshAccounts()
    toast(`已添加离线账户 ${acc.name}`, 'success')
    offlineName.value = ''
    adding.value = false
  } catch (e) {
    toastError(e)
  } finally {
    busy.value = false
  }
}

// 外置登录
const server = ref('')
const serverInfo = ref<ServerInfo | null>(null)
const serverError = ref('')
const username = ref('')
const password = ref('')
const profiles = ref<GameProfile[]>([])
const presets = [{ label: 'LittleSkin', url: 'https://littleskin.cn/api/yggdrasil' }]

watch(server, () => {
  serverInfo.value = null
  serverError.value = ''
  profiles.value = []
})

async function checkServer() {
  if (!server.value.trim()) return
  busy.value = true
  serverError.value = ''
  try {
    serverInfo.value = await api.checkAuthServer(server.value)
  } catch (e) {
    serverError.value = errMsg(e)
  } finally {
    busy.value = false
  }
}

function usePreset(url: string) {
  server.value = url
  checkServer()
}

async function login(profileId?: string) {
  if (busy.value || !server.value || !username.value || !password.value) return
  busy.value = true
  try {
    const r = await api.addYggdrasilAccount(server.value, username.value, password.value, profileId)
    if (r.status === 'choose_profile') {
      profiles.value = r.profiles
      toast('这个账号下有多个角色，选一个', 'info')
      return
    }
    await refreshAccounts()
    toast(`登录成功：${r.account?.name}`, 'success')
    password.value = ''
    profiles.value = []
    adding.value = false
  } catch (e) {
    toastError(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="ap-fade">
      <div v-if="store.accountPanel" class="ap-backdrop" @click.self="close" @contextmenu.prevent />
    </Transition>
    <Transition name="ap" @after-enter="afterEnter">
      <section
        v-if="store.accountPanel"
        class="ap-panel glass glass-strong"
        :style="panelStyle"
        role="dialog"
        aria-label="账户"
        v-light
      >
        <!-- 左：皮肤与当前账户 -->
        <div class="stage">
          <div class="stage-glow" />
          <div class="stage-floor" />
          <div class="viewer-box">
            <SkinViewer3D
              v-if="showViewer && !viewerFailed"
              ref="viewer"
              :skin="skin.url"
              :slim="skin.slim"
              :cape="skin.cape"
              :animated="animated"
              @failed="viewerFailed = true"
            />
            <div v-else class="viewer-ph">
              <Avatar :name="current?.name ?? '?'" :seed="current?.uuid ?? ''" :account="current" :size="96" />
              <span v-if="viewerFailed" class="faint small">这台电脑不支持 3D 预览</span>
            </div>
          </div>
          <div class="stage-info">
            <Transition name="swap" mode="out-in">
              <div :key="current?.id ?? 'none'" class="who">
                <div class="who-name ellipsis">{{ current?.name ?? '还没有账户' }}</div>
                <div class="faint small ellipsis">
                  {{ current ? kindLabel(current) : '添加一个账户后就能启动游戏' }}
                </div>
              </div>
            </Transition>
            <div class="stage-actions">
              <span class="faint small skin-src">
                <template v-if="skinLoading(current?.id)"><span class="spinner tiny" /> 正在获取皮肤</template>
                <template v-else>{{ skin.custom ? '皮肤来自认证服务器' : `默认皮肤（${skin.slim ? 'Alex' : 'Steve'}）` }}</template>
              </span>
              <span class="spacer" />
              <button v-if="animated && showViewer && !viewerFailed" class="btn ghost sm" title="打个招呼" @click="viewer?.wave()">挥手</button>
              <button v-if="animated && showViewer && !viewerFailed" class="btn ghost sm" title="走两步" @click="viewer?.walk()">走两步</button>
              <button
                v-if="current?.kind === 'yggdrasil'"
                class="btn ghost sm icon"
                title="重新获取皮肤"
                :disabled="skinLoading(current?.id)"
                @click="refreshSkin"
              >
                <Icon name="refresh" :size="15" />
              </button>
            </div>
          </div>
        </div>

        <!-- 右：账户列表 / 添加 -->
        <div class="side">
          <header class="side-head">
            <h3>{{ adding ? '添加账户' : '账户' }}</h3>
            <span v-if="!adding" class="badge">{{ store.accounts.length }}</span>
            <span class="spacer" />
            <button class="btn ghost sm icon" title="关闭（Esc）" @click="close"><Icon name="close" :size="16" /></button>
          </header>

          <Transition name="pane" mode="out-in">
            <div v-if="!adding" key="list" class="pane list-pane">
              <div class="acc-scroll">
                <TransitionGroup name="list" tag="div" class="acc-list">
                  <div
                    v-for="(a, i) in store.accounts"
                    :key="a.id"
                    class="acc"
                    :class="{ active: store.settings?.selectedAccount === a.id }"
                    :style="{ '--i': i }"
                    role="button"
                    tabindex="0"
                    @click="choose(a)"
                    @keydown.enter="choose(a)"
                  >
                    <Avatar :name="a.name" :seed="a.uuid" :account="a" :size="36" />
                    <div class="acc-info">
                      <div class="acc-name ellipsis">{{ a.name }}</div>
                      <div class="acc-sub faint ellipsis">{{ kindLabel(a) }}</div>
                    </div>
                    <span v-if="store.settings?.selectedAccount === a.id" class="cur" title="当前账户">
                      <Icon name="check" :size="14" :stroke="2.6" />
                    </span>
                    <button
                      class="del btn ghost sm"
                      :class="{ confirm: confirmId === a.id }"
                      :title="confirmId === a.id ? '再点一次删除' : '删除账户'"
                      @click.stop="askRemove(a)"
                    >
                      <Icon name="trash" :size="15" />
                      <span v-if="confirmId === a.id" class="del-text">删除？</span>
                    </button>
                  </div>
                </TransitionGroup>
                <div v-if="!store.accounts.length" class="no-acc faint">
                  <Icon name="user" :size="28" />
                  <span>还没有账户</span>
                </div>
              </div>
              <button class="btn add-btn" @click="startAdd"><Icon name="plus" :size="16" /> 添加账户</button>
            </div>

            <div v-else key="add" class="pane add-pane">
              <Segmented
                v-model="mode"
                :options="[
                  { value: 'offline', label: '离线账户' },
                  { value: 'yggdrasil', label: '外置登录' },
                ]"
              />
              <Transition name="pane" mode="out-in">
                <div v-if="mode === 'offline'" key="offline" class="form">
                  <div class="field">
                    <label>玩家名</label>
                    <input
                      v-model.trim="offlineName"
                      class="input"
                      maxlength="16"
                      placeholder="例如 Steve"
                      spellcheck="false"
                      @keydown.enter="addOffline"
                    />
                    <span v-if="nameWarning" class="hint warn">{{ nameWarning }}</span>
                    <span class="hint faint">UUID 按 “OfflinePlayer:玩家名” 计算，和服务器的离线模式一致。</span>
                  </div>
                </div>
                <div v-else key="ygg" class="form">
                  <div class="field">
                    <label>认证服务器</label>
                    <div class="row">
                      <input
                        v-model.trim="server"
                        class="input"
                        placeholder="https://example.com/api/yggdrasil"
                        spellcheck="false"
                        @blur="checkServer"
                      />
                      <button class="btn sm" :disabled="busy || !server" @click="checkServer">检测</button>
                    </div>
                    <div class="row presets">
                      <span class="faint">常用</span>
                      <button v-for="p in presets" :key="p.url" class="btn ghost sm" @click="usePreset(p.url)">{{ p.label }}</button>
                    </div>
                    <span v-if="serverInfo" class="hint ok">
                      <Icon name="check" :size="13" /> {{ serverInfo.serverName ?? '认证服务器' }}
                    </span>
                    <span v-if="serverError" class="hint warn">{{ serverError }}</span>
                  </div>
                  <div class="two">
                    <div class="field">
                      <label>账号</label>
                      <input v-model.trim="username" class="input" autocomplete="username" placeholder="邮箱或用户名" />
                    </div>
                    <div class="field">
                      <label>密码</label>
                      <input
                        v-model="password"
                        class="input"
                        type="password"
                        autocomplete="current-password"
                        @keydown.enter="login()"
                      />
                    </div>
                  </div>
                  <span class="hint faint">密码只用于这次登录，不会保存。启动时会自动下载 authlib-injector。</span>
                  <div v-if="profiles.length" class="field">
                    <label>选择角色</label>
                    <div class="profiles">
                      <button v-for="p in profiles" :key="p.id" class="btn sm" :disabled="busy" @click="login(p.id)">
                        <Avatar :name="p.name" :seed="p.id" :size="20" /> {{ p.name }}
                      </button>
                    </div>
                  </div>
                </div>
              </Transition>
              <div class="actions">
                <button v-if="store.accounts.length" class="btn" @click="adding = false">取消</button>
                <span class="spacer" />
                <button
                  v-if="mode === 'offline'"
                  class="btn primary"
                  :disabled="busy || !offlineName"
                  @click="addOffline"
                >
                  <span v-if="busy" class="spinner" /> 添加
                </button>
                <button
                  v-else
                  class="btn primary"
                  :disabled="busy || !server || !username || !password"
                  @click="login()"
                >
                  <span v-if="busy" class="spinner" /> 登录
                </button>
              </div>
            </div>
          </Transition>
        </div>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped>
.ap-backdrop {
  position: fixed;
  inset: 0;
  z-index: 240;
  background: color-mix(in srgb, var(--scrim) 45%, transparent);
}
.ap-panel {
  --radius: 28px;
  --blur: 34px;
  position: fixed;
  z-index: 250;
  display: grid;
  grid-template-columns: minmax(200px, 42%) 1fr;
  overflow: hidden;
  color: var(--text);
}

/* ───── 左侧皮肤舞台 ───── */
.stage {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  padding: 14px 14px 12px;
  isolation: isolate;
  border-right: 1px solid var(--divider);
}
.stage-glow {
  position: absolute;
  inset: 0;
  z-index: -1;
  pointer-events: none;
  background: radial-gradient(90% 60% at 50% 62%, color-mix(in srgb, var(--accent) 26%, transparent), transparent 70%),
    linear-gradient(180deg, color-mix(in srgb, var(--accent-2) 10%, transparent), transparent 60%);
}
html[data-theme='dark'] .stage-glow {
  background: radial-gradient(90% 60% at 50% 62%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 72%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 60%);
}
/* 脚下的一圈影子 */
.stage-floor {
  position: absolute;
  left: 22%;
  right: 22%;
  bottom: 86px;
  height: 18px;
  z-index: -1;
  border-radius: 50%;
  background: radial-gradient(closest-side, rgba(0, 0, 0, 0.22), transparent);
  pointer-events: none;
}
html[data-theme='dark'] .stage-floor {
  background: radial-gradient(closest-side, rgba(0, 0, 0, 0.45), transparent);
}
.viewer-box {
  position: relative;
  flex: 1;
  min-height: 0;
}
.viewer-ph {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.stage-info {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 0 4px;
}
.who-name {
  font-family: var(--font-display);
  font-size: 19px;
  font-weight: 700;
}
.stage-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 30px;
}
.skin-src {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.spinner.tiny {
  width: 12px;
  height: 12px;
  border-width: 2px;
}

/* ───── 右侧 ───── */
.side {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  padding: 12px 12px 12px 14px;
}
.side-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 2px 8px 4px;
}
.side-head h3 {
  font-size: 16px;
}
.pane {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.acc-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 2px;
  margin: -2px;
}
.acc-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.acc {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px 8px 10px;
  border-radius: 16px;
  cursor: pointer;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--field-border);
  transition: transform 0.45s var(--spring), box-shadow 0.2s ease, background 0.2s ease;
}
.acc:hover {
  transform: translateY(-1px);
  background: var(--glass-bg-hover);
}
.acc:active {
  transform: scale(0.98);
  transition-duration: 0.12s;
}
.acc:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 1.5px var(--accent);
}
.acc.active {
  box-shadow: inset 0 0 0 1.5px var(--accent), 0 6px 18px -12px var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, var(--field-bg));
}
.acc-info {
  flex: 1;
  min-width: 0;
}
.acc-name {
  font-weight: 650;
}
.acc-sub {
  font-size: 11.5px;
  margin-top: 1px;
}
.cur {
  width: 22px;
  height: 22px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 50%;
  color: #fff;
  background: var(--accent);
  box-shadow: 0 3px 8px -3px var(--accent);
  animation: cur-pop 0.5s var(--spring) both;
}
@keyframes cur-pop {
  from {
    transform: scale(0.3) rotate(-30deg);
    opacity: 0;
  }
}
.del {
  flex: none;
  gap: 4px;
  padding: 0 8px;
  opacity: 0;
  transition: opacity 0.15s ease, transform 0.45s var(--spring), background 0.2s ease;
}
.acc:hover .del,
.acc:focus-within .del,
.del.confirm {
  opacity: 1;
}
.del.confirm {
  color: #fff;
  background: var(--danger);
}
.del-text {
  font-size: 12px;
}
.no-acc {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 34px 0;
}
.add-btn {
  flex: none;
  width: 100%;
}
.add-pane {
  overflow: auto;
}
.add-pane > .segmented {
  align-self: flex-start;
}
.form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.hint {
  font-size: 11.5px;
  line-height: 1.5;
  display: flex;
  align-items: center;
  gap: 4px;
}
.hint.warn {
  color: color-mix(in srgb, var(--warning) 80%, var(--text));
}
.hint.ok {
  color: color-mix(in srgb, var(--success) 75%, var(--text));
}
.presets {
  gap: 4px;
  font-size: 12px;
}
.profiles {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.actions {
  margin-top: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 4px;
}
.small {
  font-size: 12px;
}

/* ───── 动画 ───── */
.ap-fade-enter-active,
.ap-fade-leave-active {
  transition: opacity 0.25s ease;
}
.ap-fade-enter-from,
.ap-fade-leave-to {
  opacity: 0;
}
.ap-enter-active {
  transition: transform 0.62s var(--spring), opacity 0.22s ease;
}
.ap-leave-active {
  transition: transform 0.2s var(--ease-out), opacity 0.18s ease;
}
.ap-enter-from {
  opacity: 0;
  transform: scale(0.35);
}
.ap-leave-to {
  opacity: 0;
  transform: scale(0.9);
}
/* 面板展开时左右两栏错开淡入 */
.ap-enter-active .stage,
.ap-enter-active .side {
  animation: rise-in 0.42s var(--ease-out) both;
}
.ap-enter-active .side {
  animation-delay: 0.06s;
}
.pane-enter-active {
  transition: opacity 0.22s ease, transform 0.42s var(--spring);
}
.pane-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.pane-enter-from {
  opacity: 0;
  transform: translateY(10px);
}
.pane-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>
