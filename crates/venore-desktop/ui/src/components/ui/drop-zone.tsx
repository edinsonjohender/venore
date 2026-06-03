// =============================================================================
// DropZone - Reusable drag-and-drop overlay component
// =============================================================================
// Wraps children with a drop overlay that appears when files are dragged
// over the window. Uses Tauri v2 native drag-drop events via useDropZone.

import type { ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { FolderOpen } from 'lucide-react'
import { useDropZone } from '@/hooks/useDropZone'
import { cn } from '@/lib/utils'

interface DropZoneProps {
  /** Callback when files are dropped */
  onDrop: (paths: string[]) => void
  /** Content to render inside the drop zone */
  children: ReactNode
  /** Disable drag-and-drop */
  disabled?: boolean
  /** Icon shown in the overlay (default: FolderOpen) */
  icon?: LucideIcon
  /** Title text in the overlay */
  title?: string
  /** Subtitle text in the overlay */
  subtitle?: string
  /** Additional class name for the container */
  className?: string
}

export function DropZone({
  onDrop,
  children,
  disabled,
  icon: Icon = FolderOpen,
  title = 'Drop project folder here',
  subtitle = 'or .venore workspace file',
  className,
}: DropZoneProps) {
  const { isDragging } = useDropZone({ onDrop, disabled })

  return (
    <div className={cn('relative', className)}>
      {children}

      {/* Drag Overlay */}
      {isDragging && (
        <div className="absolute inset-0 z-50 bg-background/90 backdrop-blur-sm flex items-center justify-center border-4 border-dashed border-brand rounded-lg m-4">
          <div className="flex flex-col items-center gap-4">
            <div className="w-16 h-16 rounded-full bg-brand/10 flex items-center justify-center">
              <Icon className="w-8 h-8 text-brand" />
            </div>
            <p className="text-lg text-foreground font-medium">{title}</p>
            <p className="text-sm text-foreground-muted">{subtitle}</p>
          </div>
        </div>
      )}
    </div>
  )
}
