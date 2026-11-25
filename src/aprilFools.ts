// 愚人节版本识别（参考 PCL：内置名单优先，规则只用来兜底未来的新版本）。
// 与后端 src-tauri/src/minecraft/april_fools.rs 的名单与规则保持一致：
//  - 后端给清单里的每个版本算好 aprilFools 字段，给实例算好 versionType = 'april_fools'
//  - 前端只在字段缺失（例如旧缓存）或只有版本号（LoaderPicker）时用这里的函数判断

/** 官方清单（version_manifest_v2.json）中实际存在的愚人节版本 ID（2026-11 核实） */
export const APRIL_FOOLS_IDS: string[] = [
  '15w14a',
  '1.RV-Pre1',
  '3D Shareware v1.34',
  '20w14infinite',
  '22w13oneblockatatime',
  '23w13a_or_b',
  '24w14potato',
  '25w14craftmine',
  '26w14a',
]

/** 其他启动器可能使用的别名 */
const ALIASES = ['20w14∞', '20w14infinity', '22w13oneBlockAtATime']

/** 版本列表里显示的名称（官方英文名） */
export const APRIL_FOOLS_NAMES: Record<string, string> = {
  '15w14a': 'The Love and Hugs Update',
  '1.RV-Pre1': 'Trendy Update',
  '3D Shareware v1.34': '3D Shareware',
  '20w14infinite': '20w14∞',
  '22w13oneblockatatime': 'One Block at a Time',
  '23w13a_or_b': 'The Vote Update',
  '24w14potato': 'Poisonous Potato Update',
  '25w14craftmine': 'Craftmine Update',
  '26w14a': 'Herdcraft Update',
}

/** 官方清单里没有、无法安装的早期愚人节版本：显示为灰色条目，点击打开 Minecraft Wiki */
export interface OfflineAprilFools {
  id: string
  name: string
  releaseTime: string
  wiki: string
}

export const APRIL_FOOLS_OFFLINE: OfflineAprilFools[] = [
  {
    id: '2.0',
    name: 'Minecraft 2.0（基于 1.5.1，蓝 / 红 / 紫三个版本）',
    releaseTime: '2013-04-01T00:00:00+00:00',
    wiki: 'https://zh.minecraft.wiki/w/Java%E7%89%882.0',
  },
]

const LOWER = new Set([...APRIL_FOOLS_IDS, ...ALIASES].map((s) => s.toLowerCase()))

export function isAprilFoolsId(id: string | null | undefined): boolean {
  return !!id && LOWER.has(id.trim().toLowerCase())
}

/** 常规快照格式：24w14a；1.20.5-pre1 / 1.21-rc1 / 26.1-snapshot-3 / 26.4-pre-1 / 1.14 Pre-Release 2 / 1.7 */
const WEEKLY = /^\d\dw\d\d[a-z]$/
const NUMBERED = /^\d+(\.\d+)+(-pre\d+|-rc\d+|-pre-\d+|-rc-\d+|-snapshot-\d+| Pre-Release \d+)?$/

export function isRegularSnapshotId(id: string): boolean {
  return WEEKLY.test(id) || NUMBERED.test(id)
}

/** releaseTime 是否在 3 月 31 日 ~ 4 月 2 日（UTC，直接取 ISO 字符串的月-日，与后端一致） */
function nearAprilFirst(releaseTime: string | null | undefined): boolean {
  const md = (releaseTime ?? '').slice(5, 10)
  return md === '03-31' || md === '04-01' || md === '04-02'
}

/** 名单优先；规则兜底：snapshot + 4 月 1 日前后 + ID 不符合任何常规快照格式 */
export function isAprilFools(v: {
  id: string
  type?: string | null
  releaseTime?: string | null
  aprilFools?: boolean | null
}): boolean {
  if (v.aprilFools === true) return true
  if (isAprilFoolsId(v.id)) return true
  return v.type === 'snapshot' && nearAprilFirst(v.releaseTime) && !isRegularSnapshotId(v.id.trim())
}
