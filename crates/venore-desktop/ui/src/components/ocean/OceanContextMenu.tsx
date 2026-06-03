import { useEffect, useRef, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

import { cn } from '@/lib/utils'

export interface OceanContextMenuItem {
  id: string
  label: string
  icon?: ReactNode
  danger?: boolean
  disabled?: boolean
  onSelect: () => void
}

interface OceanContextMenuProps {
  open: boolean
  /** Screen-space coords of the right-click event (clientX/clientY). */
  position: { x: number; y: number } | null
  items: OceanContextMenuItem[]
  onClose: () => void
}

const MENU_WIDTH = 200
const ITEM_HEIGHT = 32
const VIEWPORT_PADDING = 8

export function OceanContextMenu({ open, position, items, onClose }: OceanContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return

    const handleMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }

    // Defer registration so the same right-click that opened the menu doesn't close it
    const timeout = window.setTimeout(() => {
      window.addEventListener('mousedown', handleMouseDown)
      window.addEventListener('keydown', handleKey)
    }, 0)

    return () => {
      window.clearTimeout(timeout)
      window.removeEventListener('mousedown', handleMouseDown)
      window.removeEventListener('keydown', handleKey)
    }
  }, [open, onClose])

  if (!open || !position) return null

  // Clamp to viewport so the menu never overflows
  const maxLeft = window.innerWidth - MENU_WIDTH - VIEWPORT_PADDING
  const maxTop = window.innerHeight - items.length * ITEM_HEIGHT - VIEWPORT_PADDING
  const left = Math.min(Math.max(position.x, VIEWPORT_PADDING), Math.max(maxLeft, VIEWPORT_PADDING))
  const top = Math.min(Math.max(position.y, VIEWPORT_PADDING), Math.max(maxTop, VIEWPORT_PADDING))

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 rounded-md border border-zinc-700 bg-zinc-900/95 py-1 shadow-xl backdrop-blur-md"
      style={{ left, top, minWidth: MENU_WIDTH }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          disabled={item.disabled}
          onClick={() => {
            if (item.disabled) return
            item.onSelect()
            onClose()
          }}
          className={cn(
            'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-zinc-200',
            'hover:bg-zinc-800',
            item.danger && 'text-red-400 hover:bg-red-900/30',
            item.disabled && 'cursor-not-allowed opacity-50 hover:bg-transparent',
          )}
        >
          {item.icon ? <span className="flex h-4 w-4 items-center justify-center">{item.icon}</span> : null}
          <span>{item.label}</span>
        </button>
      ))}
    </div>,
    document.body,
  )
}
