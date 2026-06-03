// =============================================================================
// FloatingOverlay - Base container for floating overlays above chat input
// =============================================================================
// Positions content absolute above the input area. Supports accent border
// colors, ESC-to-dismiss, and scroll for long content.

import { useEffect, useCallback, type ReactNode } from 'react'
import { cn } from '@/lib/utils'

export type AccentColor = 'amber' | 'brand' | 'blue' | 'emerald'

// Subtle left-edge accent — only hint of color on the overlay
const accentBarMap: Record<AccentColor, string> = {
  amber: 'border-l-amber-500/40',
  brand: 'border-l-brand/40',
  blue: 'border-l-blue-500/40',
  emerald: 'border-l-emerald-500/40',
}

interface FloatingOverlayProps {
  children: ReactNode
  accentColor?: AccentColor
  onDismiss?: () => void
}

export function FloatingOverlay({ children, accentColor = 'brand', onDismiss }: FloatingOverlayProps) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape' && onDismiss) {
        e.preventDefault()
        onDismiss()
      }
    },
    [onDismiss],
  )

  useEffect(() => {
    if (!onDismiss) return
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown, onDismiss])

  return (
    <div
      className={cn(
        'absolute bottom-full left-0 right-0 mb-2 mx-3 z-50',
        'bg-background-secondary border border-border border-l-2 rounded-lg shadow-xl',
        'max-h-[60vh] overflow-y-auto',
        'animate-in fade-in slide-in-from-bottom-2 duration-150',
        accentBarMap[accentColor],
      )}
    >
      {children}
    </div>
  )
}
