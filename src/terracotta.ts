// 联机（Terracotta | 陶瓦联机）相关的文案与工具函数
import type { RoomPhase, TerracottaRoom } from './api'

/** 状态的中文名称 */
export const ROOM_STATE_LABEL: Record<string, string> = {
  waiting: '空闲',
  'host-scanning': '正在等待局域网世界',
  'host-starting': '正在创建房间',
  'host-ok': '房间已开启',
  'guest-connecting': '正在连接房间',
  'guest-starting': '正在建立连接',
  'guest-ok': '已加入房间',
  exception: '出现异常',
}

/** exception 状态的 type（见 Terracotta 文档 http-api.md） */
export const EXCEPTION_TEXT: Record<number, string> = {
  0: '加入房间失败：房间已关闭，或网络不稳定',
  1: '与房主的连接断开了：房间已关闭，或网络不稳定',
  2: '加入房间失败：EasyTier 意外退出了',
  3: '创建房间失败：EasyTier 意外退出了',
  4: '房间已自动关闭：房主退出了游戏世界（局域网世界不再可用）',
  5: '协议错误：房主返回了无效的数据',
}

/** guest-starting 的 NAT 连接难度（由双方 NAT 类型推算，仅供参考） */
export const DIFFICULTY_TEXT: Record<string, { text: string; tone: 'success' | 'accent' | 'warning' | 'danger' | '' }> = {
  UNKNOWN: { text: '正在检测网络类型…', tone: '' },
  EASIEST: { text: '网络状况极好，马上就好', tone: 'success' },
  SIMPLE: { text: '网络状况较好，建立连接需要一点时间', tone: 'accent' },
  MEDIUM: { text: '网络状况一般，已启用备用线路，可能连接失败', tone: 'warning' },
  TOUGH: { text: '网络状况较差，已启用备用线路，可能连接失败', tone: 'danger' },
}

export const PROFILE_KIND_LABEL: Record<string, string> = {
  HOST: '房主',
  LOCAL: '你',
  GUEST: '房客',
}

export function isHostPhase(s: string | null | undefined): boolean {
  return !!s && s.startsWith('host-')
}

export function isGuestPhase(s: string | null | undefined): boolean {
  return !!s && s.startsWith('guest-')
}

/** 房间正在进行（用于首页 / 导航栏的状态提示） */
export function roomSummary(room: TerracottaRoom | null | undefined): { text: string; live: boolean } | null {
  const s = (room?.state ?? 'waiting') as RoomPhase
  switch (s) {
    case 'host-ok':
      return { text: `房主 · ${room?.room ?? ''} · ${room?.profiles.length ?? 1} 人`, live: true }
    case 'guest-ok':
      return { text: `已加入房间 · ${room?.profiles.length ?? 1} 人`, live: true }
    case 'host-scanning':
    case 'host-starting':
      return { text: '正在创建联机房间', live: false }
    case 'guest-connecting':
    case 'guest-starting':
      return { text: '正在加入联机房间', live: false }
    default:
      return null
  }
}

const CHARS = '0123456789ABCDEFGHJKLMNPQRSTUVWXYZ'
const CODE_LEN = 21 // 'U/XXXX-XXXX-XXXX-XXXX'.length

/**
 * 邀请码规范化与校验（与 Terracotta 源码 controller/rooms/scaffolding/room.rs 的规则一致）：
 * `U/XXXX-XXXX-XXXX-XXXX`，不区分大小写，I / O 视为 1 / 0，16 位 34 进制数必须是 7 的倍数。
 * 允许夹在一段文字中间（例如直接粘贴整条聊天消息）。无效时返回 null。
 */
export function normalizeRoomCode(input: string): string | null {
  const s = input.trim().toUpperCase()
  for (let start = 0; start + CODE_LEN <= s.length; start++) {
    if (s[start] !== 'U' || s[start + 1] !== '/') continue
    const body = s.slice(start + 2, start + CODE_LEN)
    let value = BigInt(0)
    let ok = true
    for (let i = 18; i >= 0; i--) {
      const c = body[i]
      if (i === 4 || i === 9 || i === 14) {
        if (c !== '-') {
          ok = false
          break
        }
        continue
      }
      const mapped = c === 'I' ? '1' : c === 'O' ? '0' : c
      const v = CHARS.indexOf(mapped)
      if (v < 0) {
        ok = false
        break
      }
      value = value * BigInt(34) + BigInt(v)
    }
    if (ok && value % BigInt(7) === BigInt(0)) return 'U/' + body.replace(/I/g, '1').replace(/O/g, '0')
  }
  return null
}
