// 加载器版本列表 / 前置模组版本列表的前端缓存与请求合并：
//  - 同一 (加载器, 游戏版本) 同时只发一个请求，后来的调用复用同一个 Promise
//  - 结果在本次运行中缓存 10 分钟（后端还有 1 小时的内存 + 磁盘缓存）
//  - 失败的请求不缓存，下次重新请求
import { api, LOADER_ADDONS, type LoaderKind, type LoaderVersion, type ResourceVersion } from './api'

const TTL = 10 * 60_000

interface Entry<T> {
  at: number
  p: Promise<T>
}

function memo<T>(map: Map<string, Entry<T>>, key: string, force: boolean, run: () => Promise<T>): Promise<T> {
  const hit = map.get(key)
  if (!force && hit && Date.now() - hit.at < TTL) return hit.p
  const p = run()
  map.set(key, { at: Date.now(), p })
  p.catch(() => {
    if (map.get(key)?.p === p) map.delete(key)
  })
  return p
}

const loaderCache = new Map<string, Entry<LoaderVersion[]>>()
const addonCache = new Map<string, Entry<ResourceVersion[]>>()

export const ALL_LOADERS: LoaderKind[] = ['forge', 'neoforge', 'fabric', 'quilt', 'optifine']

export function loaderVersions(kind: LoaderKind, mc: string, force = false): Promise<LoaderVersion[]> {
  return memo(loaderCache, `${kind}@${mc}`, force, () => api.listLoaderVersions(kind, mc, force))
}

/** Fabric API / QFAPI 适配 mc 的版本（Modrinth，按 game_versions 与 loaders 过滤，新到旧） */
export function addonVersions(kind: LoaderKind, mc: string, force = false): Promise<ResourceVersion[]> {
  const a = LOADER_ADDONS[kind]
  if (!a) return Promise.resolve([])
  return memo(addonCache, `${a.projectId}@${mc}`, force, () =>
    api.communityVersions('modrinth', a.projectId, mc, [a.loader]).then((list) =>
      list.filter((v) => v.gameVersions.includes(mc) && v.loaders.includes(a.loader)),
    ),
  )
}

/** 选好游戏版本后并行预取所有加载器（以及 Fabric API / QFAPI）的版本列表 */
export function prefetchLoaders(mc: string) {
  if (!mc) return
  for (const k of ALL_LOADERS) loaderVersions(k, mc).catch(() => {})
  for (const k of Object.keys(LOADER_ADDONS) as LoaderKind[]) addonVersions(k, mc).catch(() => {})
}
