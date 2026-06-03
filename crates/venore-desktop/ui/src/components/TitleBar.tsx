// =============================================================================
// TitleBar - Custom window title bar shell
// =============================================================================
// Renders the shared window chrome: logo, content slot, drag region, and
// window controls. Pass children to customize the content area (menus, title,
// etc.). For the default IDE menu bar, use <TitleBarMenus /> as children.

import venoreIcon from '../assets/venore-icon.svg'
import { isMacOS } from '../lib/platform'
import { WindowControls } from './WindowControls'

interface TitleBarProps {
  children?: React.ReactNode
  /** Buttons rendered just before the OS window controls (min/max/close).
   *  Use this for window-scoped actions (e.g. AI toggle, dock-back) so they
   *  visually group with the chrome instead of floating next to the title. */
  rightActions?: React.ReactNode
}

export function TitleBar({ children, rightActions }: TitleBarProps) {
  return (
    <div className="flex items-center h-8 bg-background border-b border-border shrink-0">
      {/* macOS: traffic lights on the left | Windows: logo & name */}
      {isMacOS ? (
        <WindowControls />
      ) : (
        <div className="flex items-center gap-2 px-3 h-full shrink-0">
          <img
            src={venoreIcon}
            alt="Venore"
            className="h-4 w-auto"
            draggable={false}
          />
          <span className="text-xs font-medium text-foreground-muted select-none">
            Venore
          </span>
        </div>
      )}

      {/* Custom content (menus, pop-out title, etc.) */}
      {children}

      {/* Draggable spacer */}
      <div data-tauri-drag-region className="flex-1 h-full" />

      {/* Window-scoped actions, pushed against the chrome. No padding so any
          chrome-styled buttons (e.g. WindowControlButton) sit flush with the
          min/max/close group. */}
      {rightActions && (
        <div className="flex items-center gap-1 h-full shrink-0">
          {rightActions}
        </div>
      )}

      {/* Window Controls (Windows/Linux only — macOS uses traffic lights on the left) */}
      {!isMacOS && <WindowControls />}
    </div>
  )
}
