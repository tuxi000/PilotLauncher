<script setup lang="ts" generic="T extends string | number">
// 液态玻璃下拉框：替代原生 <select> / <datalist>
//  - 触发按钮是玻璃风格；弹出层用 Teleport 挂到 body（不会被对话框 / 卡片的 overflow 裁掉）
//  - 玻璃背景（backdrop-filter）、圆角、弹簧动画；悬停与选中高亮；选项可带标签（推荐 / 最新 …）
//  - 长列表：搜索框 + 虚拟滚动（只渲染可见的几十项，Forge 几百个版本也不卡）
//  - 键盘：↑ ↓ Home End PageUp PageDown 回车 Esc；点外部关闭；按可用空间自动向上或向下展开
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import Icon from './Icon.vue'
import { iconUrl } from '../instanceIcons'
import type { SelectOption } from '../selectOption'

// 组件有两个根节点（按钮 + Teleport），class / style 等属性手动绑定到按钮上
defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    modelValue: T | null | undefined
    options: SelectOption<T>[]
    placeholder?: string
    /** 是否显示搜索框；默认 'auto'：选项超过 10 个时显示 */
    searchable?: boolean | 'auto'
    size?: 'sm' | 'md'
    disabled?: boolean
    /** 弹出层最小宽度（像素），默认与触发按钮同宽 */
    minWidth?: number
    emptyText?: string
    title?: string
  }>(),
  {
    placeholder: '请选择',
    searchable: 'auto',
    size: 'md',
    disabled: false,
    minWidth: 0,
    emptyText: '没有匹配的选项',
    title: '',
  },
)
const emit = defineEmits<{
  (e: 'update:modelValue', v: T): void
  (e: 'change', v: T): void
}>()

const root = ref<HTMLElement | null>(null)
const pop = ref<HTMLElement | null>(null)
const listEl = ref<HTMLElement | null>(null)
const searchEl = ref<HTMLInputElement | null>(null)

const open = ref(false)
const query = ref('')
const active = ref(-1)
const scrollTop = ref(0)
const pos = ref({ left: 0, top: 0, bottom: 0, width: 200, maxList: 300, up: false })

const selected = computed(() => props.options.find((o) => o.value === props.modelValue) ?? null)
const showSearch = computed(() => (props.searchable === 'auto' ? props.options.length > 10 : !!props.searchable))
const hasSub = computed(() => props.options.some((o) => !!o.sub))
const ITEM_H = computed(() => (hasSub.value ? 46 : 34))
const SEARCH_H = 44

const filtered = computed(() => {
  const k = query.value.trim().toLowerCase()
  if (!k) return props.options
  return props.options.filter(
    (o) =>
      o.label.toLowerCase().includes(k) ||
      (o.sub ?? '').toLowerCase().includes(k) ||
      String(o.value).toLowerCase().includes(k) ||
      (o.tags ?? []).some((t) => t.text.toLowerCase().includes(k)),
  )
})

const listH = computed(() => Math.max(ITEM_H.value, Math.min(filtered.value.length * ITEM_H.value, pos.value.maxList)))
const range = computed(() => {
  const h = ITEM_H.value
  const start = Math.max(0, Math.floor(scrollTop.value / h) - 4)
  const end = Math.min(filtered.value.length, start + Math.ceil(listH.value / h) + 8)
  return { start, end }
})
const visible = computed(() =>
  filtered.value.slice(range.value.start, range.value.end).map((o, i) => ({ o, i: range.value.start + i })),
)

function place() {
  const el = root.value
  if (!el) return
  const r = el.getBoundingClientRect()
  const vw = window.innerWidth
  const vh = window.innerHeight
  const extra = (showSearch.value ? SEARCH_H : 0) + 14
  const wanted = Math.min(props.options.length * ITEM_H.value, 320) + extra
  const below = vh - r.bottom - 10
  const above = r.top - 10
  const up = below < wanted && above > below
  const space = up ? above : below
  const width = Math.min(Math.max(r.width, props.minWidth), vw - 16)
  const left = Math.min(Math.max(8, r.left), vw - width - 8)
  pos.value = {
    left,
    top: r.bottom + 6,
    bottom: vh - r.top + 6,
    width,
    maxList: Math.max(ITEM_H.value * 2, Math.min(320, space - extra)),
    up,
  }
}

function ensureVisible(i: number) {
  const el = listEl.value
  if (!el || i < 0) return
  const h = ITEM_H.value
  const top = i * h
  if (top < el.scrollTop) el.scrollTop = top
  else if (top + h > el.scrollTop + listH.value) el.scrollTop = top + h - listH.value
  scrollTop.value = el.scrollTop
}

async function show() {
  if (props.disabled || open.value) return
  query.value = ''
  place()
  open.value = true
  active.value = Math.max(0, filtered.value.findIndex((o) => o.value === props.modelValue))
  scrollTop.value = 0
  await nextTick()
  if (listEl.value) {
    // 选中项尽量居中
    const h = ITEM_H.value
    listEl.value.scrollTop = Math.max(0, active.value * h - listH.value / 2 + h / 2)
    scrollTop.value = listEl.value.scrollTop
  }
  if (showSearch.value) searchEl.value?.focus()
  else listEl.value?.focus()
  document.addEventListener('pointerdown', onOutside, true)
  window.addEventListener('resize', place)
  window.addEventListener('scroll', onScrollAnywhere, true)
}

function hide(refocus = false) {
  if (!open.value) return
  open.value = false
  document.removeEventListener('pointerdown', onOutside, true)
  window.removeEventListener('resize', place)
  window.removeEventListener('scroll', onScrollAnywhere, true)
  if (refocus) root.value?.focus()
}

function toggle() {
  if (open.value) hide()
  else show()
}

function choose(o: SelectOption<T> | undefined) {
  if (!o || o.disabled) return
  if (o.value !== props.modelValue) {
    emit('update:modelValue', o.value)
    emit('change', o.value)
  }
  hide(true)
}

function onOutside(e: Event) {
  const t = e.target as Node | null
  if (!t) return
  if (root.value?.contains(t) || pop.value?.contains(t)) return
  hide()
}

function onScrollAnywhere(e: Event) {
  // 列表自身滚动不处理；页面或对话框滚动时跟随触发按钮重新定位
  if (pop.value && e.target instanceof Node && pop.value.contains(e.target)) return
  place()
}

function move(step: number) {
  const list = filtered.value
  if (!list.length) return
  let i = active.value
  for (let n = 0; n < list.length; n++) {
    i = Math.min(list.length - 1, Math.max(0, i + step))
    if (!list[i].disabled) break
    if (i === 0 || i === list.length - 1) break
  }
  active.value = i
  ensureVisible(i)
}

function onKey(e: KeyboardEvent) {
  if (!open.value) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
      e.preventDefault()
      show()
    }
    return
  }
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      move(1)
      break
    case 'ArrowUp':
      e.preventDefault()
      move(-1)
      break
    case 'PageDown':
      e.preventDefault()
      move(8)
      break
    case 'PageUp':
      e.preventDefault()
      move(-8)
      break
    case 'Home':
      if (!showSearch.value) {
        e.preventDefault()
        active.value = 0
        ensureVisible(0)
      }
      break
    case 'End':
      if (!showSearch.value) {
        e.preventDefault()
        active.value = filtered.value.length - 1
        ensureVisible(active.value)
      }
      break
    case 'Enter':
      e.preventDefault()
      choose(filtered.value[active.value])
      break
    case 'Escape':
      // 不让外层对话框收到 Esc（只关闭下拉框）
      e.preventDefault()
      e.stopPropagation()
      hide(true)
      break
    case 'Tab':
      hide()
      break
  }
}

function onListScroll(e: Event) {
  scrollTop.value = (e.target as HTMLElement).scrollTop
}

watch(query, () => {
  active.value = filtered.value.length ? 0 : -1
  scrollTop.value = 0
  if (listEl.value) listEl.value.scrollTop = 0
})
watch(
  () => props.disabled,
  (d) => {
    if (d) hide()
  },
)
onBeforeUnmount(() => hide())

const TONE_DEFAULT: Record<string, string> = { 推荐: 'success', 最新: 'accent', 测试版: 'warning' }
function toneOf(t: { text: string; tone?: string }) {
  return t.tone ?? TONE_DEFAULT[t.text] ?? ''
}
</script>

<template>
  <button
    ref="root"
    v-bind="$attrs"
    type="button"
    class="gs"
    :class="[size, { open, 'is-placeholder': !selected }]"
    :disabled="disabled"
    :title="title || selected?.label || ''"
    aria-haspopup="listbox"
    :aria-expanded="open"
    @click="toggle"
    @keydown="onKey"
  >
    <img v-if="selected?.icon" class="gs-ico" :src="iconUrl(selected.icon)" alt="" />
    <span class="gs-label ellipsis">{{ selected?.label ?? placeholder }}</span>
    <span v-if="selected?.tags?.length && size !== 'sm'" class="gs-tags">
      <span v-for="t in selected.tags.slice(0, 2)" :key="t.text" class="gs-tag" :class="toneOf(t)">{{ t.text }}</span>
    </span>
    <Icon name="chevronDown" :size="size === 'sm' ? 14 : 16" class="gs-chev" />
  </button>

  <Teleport to="body">
    <Transition name="gs-pop">
      <div
        v-if="open"
        ref="pop"
        class="gs-pop glass glass-strong"
        :class="{ up: pos.up }"
        :style="{
          left: `${pos.left}px`,
          width: `${pos.width}px`,
          top: pos.up ? 'auto' : `${pos.top}px`,
          bottom: pos.up ? `${pos.bottom}px` : 'auto',
        }"
        role="listbox"
        v-light
        @keydown="onKey"
      >
        <div v-if="showSearch" class="gs-search">
          <Icon name="search" :size="14" class="faint" />
          <input ref="searchEl" v-model="query" class="gs-input" placeholder="搜索…" spellcheck="false" />
          <span class="faint gs-count">{{ filtered.length }}</span>
        </div>
        <div
          ref="listEl"
          class="gs-list"
          tabindex="-1"
          :style="{ height: `${listH}px` }"
          @scroll="onListScroll"
        >
          <div v-if="!filtered.length" class="gs-empty faint">{{ emptyText }}</div>
          <div v-else class="gs-spacer" :style="{ height: `${filtered.length * ITEM_H}px` }">
            <div
              v-for="{ o, i } in visible"
              :key="String(o.value)"
              class="gs-opt"
              :class="{ active: i === active, selected: o.value === modelValue, disabled: o.disabled }"
              :style="{ top: `${i * ITEM_H}px`, height: `${ITEM_H}px` }"
              role="option"
              :aria-selected="o.value === modelValue"
              @mousemove="active = i"
              @click="choose(o)"
            >
              <img v-if="o.icon" class="gs-ico" :src="iconUrl(o.icon)" alt="" />
              <span class="gs-text">
                <span class="ellipsis">{{ o.label }}</span>
                <span v-if="o.sub" class="gs-sub faint ellipsis">{{ o.sub }}</span>
              </span>
              <span v-if="o.tags?.length" class="gs-tags">
                <span v-for="t in o.tags" :key="t.text" class="gs-tag" :class="toneOf(t)">{{ t.text }}</span>
              </span>
              <span class="gs-check"><Icon v-if="o.value === modelValue" name="check" :size="14" :stroke="2.4" /></span>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.gs {
  /* 注意：类名不能用 empty——glass.css 里的全局 .empty（空状态，纵向排列 + 48px 内边距）会把占位文字和箭头上下堆叠 */
  position: relative;
  display: inline-flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
  height: 36px;
  padding: 0 10px 0 12px;
  border: none;
  border-radius: 12px;
  cursor: pointer;
  text-align: left;
  color: var(--text);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.16), transparent), var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--field-border), inset 0 1px 0 var(--hl-bottom);
  transition: box-shadow 0.2s ease, background 0.2s ease, transform 0.45s var(--spring);
}
.gs:hover:not(:disabled) {
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.2), transparent), var(--glass-bg-hover);
}
.gs:active:not(:disabled) {
  transform: scale(0.98);
  transition-duration: 0.12s;
}
.gs:focus-visible,
.gs.open {
  outline: none;
  box-shadow: inset 0 0 0 1px var(--accent), 0 0 0 4px color-mix(in srgb, var(--accent) 20%, transparent);
}
.gs:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.gs.sm {
  height: 30px;
  padding: 0 8px 0 10px;
  font-size: 12.5px;
  border-radius: 10px;
  gap: 6px;
}
.gs.is-placeholder .gs-label {
  color: var(--text-3);
}
.gs-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1.25;
}
.gs-chev {
  flex: none;
  margin-left: auto;
  color: var(--text-3);
  transition: transform 0.45s var(--spring);
}
.gs.open .gs-chev {
  transform: rotate(180deg);
}
.gs-ico {
  width: 20px;
  height: 20px;
  flex: none;
}
.gs.sm .gs-ico {
  width: 16px;
  height: 16px;
}

/* ───── 弹出层 ───── */
.gs-pop {
  --radius: 16px;
  --blur: 26px;
  position: fixed;
  z-index: 600;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  transform-origin: top center;
  color: var(--text);
  font-size: 13px;
}
.gs-pop.up {
  transform-origin: bottom center;
  flex-direction: column-reverse;
}
.gs-search {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 10px;
  margin: 0 0 2px;
  border-radius: 11px;
  background: var(--field-bg);
  box-shadow: inset 0 0 0 1px var(--field-border);
}
.gs-pop.up .gs-search {
  margin: 2px 0 0;
}
.gs-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 13px;
}
.gs-input::placeholder {
  color: var(--text-3);
}
.gs-count {
  font-size: 11px;
}
.gs-list {
  position: relative;
  overflow-y: auto;
  overflow-x: hidden;
  outline: none;
  overscroll-behavior: contain;
}
.gs-spacer {
  position: relative;
}
.gs-opt {
  position: absolute;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 8px 0 10px;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.12s ease;
}
.gs-opt.active {
  background: var(--glass-bg-hover);
}
html[data-theme='dark'] .gs-opt.active {
  background: rgba(255, 255, 255, 0.09);
}
.gs-opt.selected {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  font-weight: 600;
}
.gs-opt.selected.active {
  background: color-mix(in srgb, var(--accent) 26%, transparent);
}
.gs-opt.disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.gs-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}
.gs-sub {
  font-size: 11px;
  font-weight: 400;
}
.gs-tags {
  display: inline-flex;
  gap: 4px;
  flex: none;
}
.gs-tag {
  height: 18px;
  padding: 0 6px;
  border-radius: 9px;
  display: inline-flex;
  align-items: center;
  font-size: 10.5px;
  font-weight: 600;
  background: color-mix(in srgb, var(--text) 9%, transparent);
  color: var(--text-2);
}
.gs-tag.success {
  background: color-mix(in srgb, var(--success) 20%, transparent);
  color: color-mix(in srgb, var(--success) 70%, var(--text));
}
.gs-tag.accent {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  color: var(--accent);
}
.gs-tag.warning {
  background: color-mix(in srgb, var(--warning) 22%, transparent);
  color: color-mix(in srgb, var(--warning) 75%, var(--text));
}
.gs-tag.danger {
  background: color-mix(in srgb, var(--danger) 18%, transparent);
  color: var(--danger);
}
.gs-check {
  width: 16px;
  flex: none;
  display: grid;
  place-items: center;
  color: var(--accent);
}
.gs-empty {
  padding: 10px;
  text-align: center;
  font-size: 12.5px;
}

.gs-pop-enter-active {
  transition: transform 0.45s var(--spring), opacity 0.18s ease;
}
.gs-pop-leave-active {
  transition: transform 0.14s ease, opacity 0.14s ease;
}
.gs-pop-enter-from,
.gs-pop-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.94);
}
.gs-pop.up.gs-pop-enter-from,
.gs-pop.up.gs-pop-leave-to {
  transform: translateY(6px) scale(0.94);
}
</style>
