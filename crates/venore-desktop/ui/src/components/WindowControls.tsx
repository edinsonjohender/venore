// =============================================================================
// WindowControls - Platform-aware window control buttons
// =============================================================================
// macOS: Traffic light buttons (gray by default, colored on hover)
// Windows/Linux: Standard minimize, maximize, close buttons
//
// Exports `WindowControlButton`: the same flat full-height shell used by the
// OS chrome buttons. Reuse it for any window-scoped action that should
// visually group with min/max/close (e.g. dock-back on a pop-out window).

import { Minus, Square, X } from 'lucide-react'
import { Window } from '@tauri-apps/api/window'
import { useTranslation } from 'react-i18next'
import { isMacOS } from '../lib/platform'
import { cn } from '../lib/utils'
import type { ButtonHTMLAttributes, ReactNode } from 'react'

interface WindowControlButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  icon: ReactNode
  /** Use `error` for destructive controls (close button) — red hover. */
  variant?: 'default' | 'error'
}

export function WindowControlButton({
  icon,
  variant = 'default',
  className,
  ...props
}: WindowControlButtonProps) {
  return (
    <button
      type="button"
      className={cn(
        'h-full px-3 transition-colors flex items-center justify-center',
        variant === 'error'
          ? 'hover:bg-semantic-error group'
          : 'hover:bg-background-tertiary',
        className,
      )}
      {...props}
    >
      <span
        className={cn(
          'inline-flex items-center justify-center text-foreground-muted',
          variant === 'error' && 'group-hover:text-white',
        )}
      >
        {icon}
      </span>
    </button>
  )
}

export function WindowControls() {
  const { t } = useTranslation('workspace')

  const handleMinimize = async () => {
    await Window.getCurrent().minimize()
  }

  const handleMaximize = async () => {
    const w = Window.getCurrent()
    if (await w.isMaximized()) await w.unmaximize()
    else await w.maximize()
  }

  const handleClose = async () => {
    await Window.getCurrent().close()
  }

  if (isMacOS) {
    return (
      <div className="flex items-center gap-2 px-3 h-full group/traffic">
        <button
          onClick={handleClose}
          className="w-3 h-3 rounded-full bg-[#3d3d3d] group-hover/traffic:bg-[#ff5f57] transition-colors"
          aria-label={t('windowControls.close')}
        />
        <button
          onClick={handleMinimize}
          className="w-3 h-3 rounded-full bg-[#3d3d3d] group-hover/traffic:bg-[#febc2e] transition-colors"
          aria-label={t('windowControls.minimize')}
        />
        <button
          onClick={handleMaximize}
          className="w-3 h-3 rounded-full bg-[#3d3d3d] group-hover/traffic:bg-[#28c840] transition-colors"
          aria-label={t('windowControls.maximize')}
        />
      </div>
    )
  }

  return (
    <div className="flex h-full">
      <WindowControlButton
        onClick={handleMinimize}
        aria-label={t('windowControls.minimize')}
        icon={<Minus className="w-3.5 h-3.5" />}
      />
      <WindowControlButton
        onClick={handleMaximize}
        aria-label={t('windowControls.maximize')}
        icon={<Square className="w-3 h-3" />}
      />
      <WindowControlButton
        variant="error"
        onClick={handleClose}
        aria-label={t('windowControls.close')}
        icon={<X className="w-4 h-4" />}
      />
    </div>
  )
}
