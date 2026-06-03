// =============================================================================
// Modal - Generic reusable modal component
// =============================================================================
// Wrapper over shadcn Dialog with standardized header/footer layout

import { ReactNode } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './dialog'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ModalProps {
  /** Is modal open */
  open: boolean
  /** Callback when modal should close */
  onOpenChange: (open: boolean) => void
  /** Icon to display in header (Lucide icon component) */
  icon?: ReactNode
  /** Modal title */
  title: string
  /** Modal description/subtitle */
  description?: string
  /** Modal content */
  children: ReactNode
  /** Footer actions */
  footer?: ReactNode
  /** Max width class (default: max-w-lg) */
  maxWidth?: string
  /** Whether to block closing (hides X button, ignores backdrop clicks) */
  blockClose?: boolean
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function Modal({
  open,
  onOpenChange,
  icon,
  title,
  description,
  children,
  footer,
  maxWidth = 'max-w-lg',
  blockClose = false,
}: ModalProps) {
  return (
    <Dialog open={open} onOpenChange={blockClose ? undefined : onOpenChange}>
      <DialogContent className={`${maxWidth} max-h-[80vh] overflow-hidden flex flex-col`}>
        {/* Header */}
        <DialogHeader>
          <div className="flex items-center gap-3">
            {icon && (
              <div className="p-2 rounded-lg bg-background-tertiary shrink-0">
                {icon}
              </div>
            )}
            <div className="flex-1 min-w-0">
              <DialogTitle>{title}</DialogTitle>
              {description && <DialogDescription>{description}</DialogDescription>}
            </div>
          </div>
        </DialogHeader>

        {/* Content - scrollable. The trailing pb-24 reserves space below
            the last child so popovers/dropdowns triggered on a row near
            the bottom of the scroll area have room to render downward
            without colliding with the footer. */}
        <div className="flex-1 overflow-y-auto py-4 pr-2 pb-24">
          {children}
        </div>

        {/* Footer */}
        {footer && <DialogFooter>{footer}</DialogFooter>}
      </DialogContent>
    </Dialog>
  )
}

// -----------------------------------------------------------------------------
// Export
// -----------------------------------------------------------------------------

export default Modal
