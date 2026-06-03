// =============================================================================
// SidebarModal — types
// =============================================================================
// Generic types for the configurable sidebar-modal component used by Settings
// and any future "left-nav inside a modal" surface (theme picker, integrations
// hub, etc.). Sections are passed declaratively so the modal stays dumb and
// the caller controls what each section renders.

import type { ComponentType, ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'

export interface SidebarSection<T extends string = string> {
  id: T
  label: string
  icon: LucideIcon
  /** Component rendered in the right pane when this section is active. */
  component: ComponentType
  /** Optional small chip rendered next to the label (e.g. "Soon", "Beta"). */
  badge?: string
  /** Color hint for the badge — defaults to brand-color. */
  badgeVariant?: 'default' | 'success' | 'warning' | 'error'
  /** When true the row is greyed out and clicks are ignored. */
  disabled?: boolean
  /** Shown as the row's title attribute + on the content-pane subheader. */
  description?: string
}

export interface SidebarModalConfig<T extends string = string> {
  title: string
  subtitle?: string
  /** Defaults to a settings gear if not provided. */
  headerIcon?: ReactNode
  sections: SidebarSection<T>[]
  /** Section id to land on when the modal opens (defaults to first). */
  defaultSection?: T
  sidebarWidth?: number
  maxWidth?: number
  maxHeight?: string
}

export interface SidebarModalProps<T extends string = string> {
  isOpen: boolean
  onClose: () => void
  config: SidebarModalConfig<T>
  /** Controlled mode — supply both to drive section from outside. */
  activeSection?: T
  onSectionChange?: (section: T) => void
}
