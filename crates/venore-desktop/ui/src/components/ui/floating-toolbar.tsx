// =============================================================================
// FloatingToolbar - Reusable floating toolbar container
// =============================================================================
// Absolute-positioned bottom-center bar with glassmorphism.
// Each screen provides its own buttons as children.

import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface FloatingToolbarProps {
  children: ReactNode
  className?: string
}

export function FloatingToolbar({ children, className }: FloatingToolbarProps) {
  return (
    <div className={cn('absolute bottom-4 left-1/2 -translate-x-1/2 z-20', className)}>
      <div className="flex items-center gap-1 px-2 py-1.5 rounded-xl border border-border bg-background/80 backdrop-blur-sm shadow-lg">
        {children}
      </div>
    </div>
  )
}

/** Visual separator between toolbar button groups */
FloatingToolbar.Separator = function Separator() {
  return <div className="w-px h-5 bg-border mx-1" />
}
