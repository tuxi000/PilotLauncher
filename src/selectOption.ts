// GlassSelect 的选项类型
export type SelectTone = 'accent' | 'success' | 'warning' | 'danger'

export interface SelectTag {
  text: string
  tone?: SelectTone
}

export interface SelectOption<V extends string | number = string> {
  value: V
  label: string
  /** 第二行的小字说明 */
  sub?: string
  /** 右侧的标签（推荐 / 最新 / 测试版 …） */
  tags?: SelectTag[]
  disabled?: boolean
  /** 左侧图标：instanceIcons 中的图标名 */
  icon?: string
}
