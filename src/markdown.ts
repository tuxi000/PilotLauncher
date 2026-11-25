// 社区资源简介的渲染：Modrinth 的 markdown 用 marked 转 HTML，CurseForge 本身是 HTML；
// 两者都经过 DOMPurify 净化（去掉脚本、iframe、表单、内联事件与样式），链接统一在外部浏览器打开。
import { marked } from 'marked'
import DOMPurify from 'dompurify'

let hooked = false

function ensureHooks() {
  if (hooked) return
  hooked = true
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    const el = node as Element
    if (el.tagName === 'IMG') {
      el.setAttribute('loading', 'lazy')
      el.setAttribute('referrerpolicy', 'no-referrer')
    }
    if (el.tagName === 'A') {
      el.setAttribute('rel', 'noopener noreferrer')
    }
  })
}

export function renderBody(body: string, format: 'markdown' | 'html'): string {
  ensureHooks()
  const html = format === 'markdown' ? (marked.parse(body ?? '', { async: false, gfm: true }) as string) : body ?? ''
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'iframe', 'form', 'input', 'button', 'textarea', 'select', 'object', 'embed', 'script'],
    FORBID_ATTR: ['style', 'class', 'id'],
  })
}

/** 把简介里的链接转为可在外部打开的 http(s) 地址（CurseForge 的 /linkout 跳转会被还原） */
export function resolveLink(href: string | null, platform: 'modrinth' | 'curseforge'): string | null {
  if (!href) return null
  let url = href.trim()
  const linkout = url.match(/linkout\?remoteUrl=([^&]+)/)
  if (linkout) {
    try {
      url = decodeURIComponent(decodeURIComponent(linkout[1]))
    } catch {
      /* 保持原样 */
    }
  }
  if (url.startsWith('//')) url = `https:${url}`
  else if (url.startsWith('/')) url = (platform === 'curseforge' ? 'https://www.curseforge.com' : 'https://modrinth.com') + url
  return /^https?:\/\//i.test(url) ? url : null
}
