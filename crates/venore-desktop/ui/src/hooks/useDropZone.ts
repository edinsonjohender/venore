// =============================================================================
// useDropZone - Hook for Tauri v2 native drag-and-drop events
// =============================================================================
// Uses getCurrentWebview().onDragDropEvent() to get real file paths
// (browser drag events do NOT expose paths when Tauri intercepts them)

import { useEffect, useRef, useState } from 'react'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { createLogger } from '@/lib/logger'

const log = createLogger('drop-zone')

interface UseDropZoneOptions {
  /** Callback when files are dropped */
  onDrop: (paths: string[]) => void
  /** Disable drag-and-drop events */
  disabled?: boolean
}

interface UseDropZoneResult {
  /** Whether a drag operation is currently over the window */
  isDragging: boolean
}

/**
 * Hook that subscribes to Tauri v2 native drag-drop events.
 *
 * Uses `getCurrentWebview().onDragDropEvent()` which provides real filesystem
 * paths (unlike browser DragEvent which has no path access in Tauri).
 *
 * @example
 * const { isDragging } = useDropZone({
 *   onDrop: (paths) => console.log('Dropped:', paths),
 *   disabled: isModalOpen,
 * })
 */
export function useDropZone({ onDrop, disabled }: UseDropZoneOptions): UseDropZoneResult {
  const [isDragging, setIsDragging] = useState(false)

  // Use ref to avoid re-subscribing when onDrop reference changes
  const onDropRef = useRef(onDrop)
  onDropRef.current = onDrop

  useEffect(() => {
    if (disabled) {
      setIsDragging(false)
      return
    }

    let unlisten: (() => void) | undefined

    const setup = async () => {
      try {
        const webview = getCurrentWebview()
        unlisten = await webview.onDragDropEvent((event) => {
          switch (event.payload.type) {
            case 'enter':
              log.debug('Drag enter', event.payload.paths)
              setIsDragging(true)
              break

            case 'over':
              // No-op, just keeps the drag active
              break

            case 'drop':
              log.info('Drop received', event.payload.paths)
              setIsDragging(false)
              if (event.payload.paths.length > 0) {
                onDropRef.current(event.payload.paths)
              }
              break

            case 'leave':
              log.debug('Drag leave')
              setIsDragging(false)
              break
          }
        })

        log.debug('Subscribed to drag-drop events')
      } catch (err) {
        log.error('Failed to subscribe to drag-drop events', err)
      }
    }

    setup()

    return () => {
      unlisten?.()
      log.debug('Unsubscribed from drag-drop events')
    }
  }, [disabled])

  return { isDragging }
}
