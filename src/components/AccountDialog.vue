<script setup lang="ts">
// 账户管理：离线登录 / 外置登录（authlib-injector）
import { ref, watch } from 'vue'
import GlassDialog from './GlassDialog.vue'
import Segmented from './Segmented.vue'
import Avatar from './Avatar.vue'
import Icon from './Icon.vue'
import { api, errMsg, type GameProfile, type ServerInfo } from '../api'
import { store, refreshAccounts, toast, toastError } from '../store'

const mode = ref<'list' | 'offline' | 'yggdrasil'>('list')
const busy = ref(false)

// 离线
const offlineName = ref('')
const nameWarning = ref('')
watch(offlineName, (v) => {
  nameWarning.value =
    v && !/^[A-Za-z0-9_]{3,16}$/.test(v) ? '建议使用 3~16 位英文、数字或下划线，否则部分服务器可能无法进入' : ''
})

async function addOffline() {
  busy.value = true
  try {
    const acc = await api.addOfflineAccount(offlineName.value)
    await refreshAccounts()
    toast(`已添加离线账户 ${acc.name}`, 'success')
    offlineName.value = ''
    mode.value = 'list'
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

async function login(profileId?: string) {
  busy.value = true
  try {
    const r = await api.addYggdrasilAccount(server.value, username.value, password.value, profileId)
    if (r.status === 'choose_profile') {
      profiles.value = r.profiles
      toast('该账户有多个角色，请选择一个', 'info')
      return
    }
    await refreshAccounts()
    toast(`登录成功：${r.account?.name}`, 'success')
    password.value = ''
    profiles.value = []
    mode.value = 'list'
  } catch (e) {
    toastError(e)
  } finally {
    busy.value = false
  }
}

async function select(id: string) {
  try {
    store.settings = await api.selectAccount(id)
  } catch (e) {
    toastError(e)
  }
}

async function remove(id: string) {
  try {
    const list = await api.removeAccount(id)
    store.accounts = list.accounts
    if (store.settings) store.settings.selectedAccount = list.selected
  } catch (e) {
    toastError(e)
  }
}

function close() {
  store.accountDialog = false
  mode.value = 'list'
}
</script>

<template>
  <GlassDialog :open="store.accountDialog" title="账户管理" :width="520" @close="close">
    <Segmented
      v-model="mode"
      :options="[
        { value: 'list', label: '我的账户' },
        { value: 'offline', label: '离线登录' },
        { value: 'yggdrasil', label: '外置登录' },
      ]"
    />

    <div v-if="mode === 'list'" class="pane">
      <div v-if="store.accounts.length === 0" class="empty">
        <Icon name="user" :size="34" />
        <div>还没有账户，先添加一个离线账户或外置账户吧</div>
      </div>
      <TransitionGroup name="list" tag="div" class="accounts">
        <div
          v-for="a in store.accounts"
          :key="a.id"
          class="acc"
          :class="{ active: store.settings?.selectedAccount === a.id }"
          @click="select(a.id)"
        >
          <Avatar :name="a.name" :seed="a.uuid" :size="40" />
          <div class="info">
            <div class="name">{{ a.name }}</div>
            <div class="sub muted ellipsis">
              {{ a.kind === 'offline' ? '离线账户' : `外置登录 · ${a.serverName ?? a.serverUrl}` }}
            </div>
          </div>
          <span v-if="store.settings?.selectedAccount === a.id" class="badge accent">当前</span>
          <button class="btn ghost sm icon" title="删除账户" @click.stop="remove(a.id)">
            <Icon name="trash" :size="16" />
          </button>
        </div>
      </TransitionGroup>
    </div>

    <div v-else-if="mode === 'offline'" class="pane">
      <div class="field">
        <label>玩家名</label>
        <input v-model.trim="offlineName" class="input" maxlength="16" placeholder="例如 Steve" @keydown.enter="addOffline" />
        <span v-if="nameWarning" class="hint warn">{{ nameWarning }}</span>
        <span class="hint muted">离线账户的 UUID 按 “OfflinePlayer:玩家名” 计算，与服务器离线模式一致。</span>
      </div>
      <div class="actions">
        <button class="btn primary" :disabled="busy || !offlineName" @click="addOffline">添加离线账户</button>
      </div>
    </div>

    <div v-else class="pane">
      <div class="field">
        <label>认证服务器地址</label>
        <div class="row">
          <input v-model.trim="server" class="input" placeholder="https://example.com/api/yggdrasil" @blur="checkServer" />
          <button class="btn sm" :disabled="busy || !server" @click="checkServer">检测</button>
        </div>
        <div class="row presets">
          <span class="faint">常用：</span>
          <button v-for="p in presets" :key="p.url" class="btn ghost sm" @click="server = p.url; checkServer()">
            {{ p.label }}
          </button>
        </div>
        <span v-if="serverInfo" class="hint ok">
          <Icon name="check" :size="14" /> {{ serverInfo.serverName ?? '认证服务器' }}（{{ serverInfo.apiRoot }}）
        </span>
        <span v-if="serverError" class="hint warn">{{ serverError }}</span>
      </div>
      <div class="field">
        <label>账号（邮箱或用户名）</label>
        <input v-model.trim="username" class="input" autocomplete="username" />
      </div>
      <div class="field">
        <label>密码</label>
        <input v-model="password" class="input" type="password" autocomplete="current-password" @keydown.enter="login()" />
        <span class="hint muted">密码只用于本次登录，不会保存；启动时自动下载 authlib-injector。</span>
      </div>
      <div v-if="profiles.length" class="field">
        <label>选择角色</label>
        <div class="profiles">
          <button v-for="p in profiles" :key="p.id" class="btn sm" :disabled="busy" @click="login(p.id)">
            <Avatar :name="p.name" :seed="p.id" :size="20" /> {{ p.name }}
          </button>
        </div>
      </div>
      <div class="actions">
        <button class="btn primary" :disabled="busy || !server || !username || !password" @click="login()">
          <span v-if="busy" class="spinner" />
          登录
        </button>
      </div>
    </div>
  </GlassDialog>
</template>

<style scoped>
.pane {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 16px;
  min-height: 180px;
}
.accounts {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.acc {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 16px;
  cursor: pointer;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--field-border);
  transition: transform 0.45s var(--spring), box-shadow 0.2s ease;
}
.acc:hover {
  transform: scale(1.015);
}
.acc.active {
  box-shadow: inset 0 0 0 1.5px var(--accent), 0 6px 18px -10px var(--accent);
}
.info {
  flex: 1;
  min-width: 0;
}
.name {
  font-weight: 600;
}
.sub {
  font-size: 12px;
  margin-top: 2px;
}
.hint {
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 4px;
}
.hint.warn {
  color: var(--warning);
}
.hint.ok {
  color: var(--success);
}
.presets {
  gap: 4px;
  font-size: 12px;
}
.profiles {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
