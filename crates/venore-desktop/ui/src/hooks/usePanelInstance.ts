// =============================================================================
// usePanelInstance - Compositor hook for a single panel instance
// =============================================================================
// Wires panelStore + useResizablePanel + useFloatingPanel into one interface.
// Each panel in the layout calls this once. Handles dock/undock/float/snap.

import { useCallback, useEffect, useRef } from 'react'
import { usePanelStore, usePanelMode, usePanelZ } from '@/stores/panelStore'
import type { PanelMode } from '@/stores/panelStore'
import { useResizablePanel } from './useResizablePanel'
import { useFloatingPanel } from './useFloatingPanel'
import type { PanelDefinition } from '@/components/workspace/panels'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

/** Distance from edge (px) at which a floating panel snaps back to docked */
const SNAP_THRESHOLD = 40

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface UsePanelInstanceOptions {
  def: PanelDefinition
  canvasZoneRef: React.RefObject<HTMLElement | null>
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function usePanelInstance({ def, canvasZoneRef }: UsePanelInstanceOptions) {
  const mode = usePanelMode(def.id)
  const zIndex = usePanelZ(def.id)
  const { setMode, bringToFront } = usePanelStore()

  // -- Rest mode: where the panel goes when "closed" --
  const restMode: PanelMode = def.collapsedContent ? 'collapsed' : 'closed'

  // -- Init effect: promote from 'closed' to defaultMode on first mount --
  const hasInitialized = useRef(false)
  useEffect(() => {
    if (!hasInitialized.current && def.defaultMode && mode === 'closed') {
      setMode(def.id, def.defaultMode)
      hasInitialized.current = true
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // -- Close handler for resize hook --
  const close = useCallback(() => {
    setMode(def.id, restMode)
  }, [def.id, restMode, setMode])

  // -- Docked resize hook --
  const resizable = useResizablePanel({
    initialWidth: def.size.initialWidth,
    minWidth: def.size.minWidth,
    maxWidth: def.size.maxWidth,
    side: def.defaultSide,
    onClose: close,
  })

  // -- Auto-reset width when transitioning to docked (e.g. from toolbar toggle) --
  const prevModeRef = useRef(mode)
  useEffect(() => {
    if (mode === 'docked' && prevModeRef.current !== 'docked') {
      resizable.reset()
    }
    prevModeRef.current = mode
  })

  // -- Dock handler --
  const dock = useCallback(() => {
    setMode(def.id, 'docked')
    resizable.reset()
  }, [def.id, setMode, resizable])

  // -- Floating hook with snap-to-dock --
  const floating = useFloatingPanel({
    initialSize: { width: def.size.initialWidth, height: def.size.floatingHeight },
    boundsRef: canvasZoneRef,
    onDragEnd: (pos, size) => {
      if (def.defaultSide === 'left') {
        if (pos.x < SNAP_THRESHOLD) dock()
      } else {
        if (!canvasZoneRef.current) return
        const canvasWidth = canvasZoneRef.current.getBoundingClientRect().width
        if (pos.x + size.width > canvasWidth - SNAP_THRESHOLD) dock()
      }
    },
  })

  // -- Undock handler --
  const undock = useCallback(
    (mouseX?: number, mouseY?: number) => {
      if (canvasZoneRef.current) {
        const bounds = canvasZoneRef.current.getBoundingClientRect()
        if (mouseX !== undefined && mouseY !== undefined) {
          floating.setPosition({
            x: mouseX - bounds.left - def.size.initialWidth / 2,
            y: mouseY - bounds.top - 20,
          })
        } else {
          floating.setPosition({
            x: Math.max(20, (bounds.width - def.size.initialWidth) / 2),
            y: Math.max(20, (bounds.height - def.size.floatingHeight) / 2),
          })
        }
      }
      setMode(def.id, 'floating')
      bringToFront(def.id)
    },
    [def.id, def.size.initialWidth, def.size.floatingHeight, canvasZoneRef, floating, setMode, bringToFront],
  )

  // -- Toggle (rest ↔ docked) --
  const toggle = useCallback(() => {
    const isActive = mode === 'docked' || mode === 'floating'
    if (isActive) {
      setMode(def.id, restMode)
    } else {
      resizable.reset()
      setMode(def.id, 'docked')
    }
  }, [def.id, mode, restMode, resizable, setMode])

  // -- Focus (bring to front) --
  const focus = useCallback(() => {
    bringToFront(def.id)
  }, [def.id, bringToFront])

  return { mode, zIndex, resizable, floating, close, dock, undock, toggle, focus }
}
