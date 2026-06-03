// =============================================================================
// useUpdateChecker - Polls for context updates at configurable intervals
// =============================================================================

import { useEffect, useRef } from 'react'
import { tauriApi } from '@/lib/tauri'
import { useUpdaterStore } from '@/stores/updaterStore'

export function useUpdateChecker(projectPath: string | undefined) {
  const checkUpdates = useUpdaterStore((s) => s.checkUpdates)
  const initListeners = useUpdaterStore((s) => s.initListeners)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    if (!projectPath) return

    let cleanup: (() => void) | undefined

    // Initialize event listeners
    initListeners().then((unlisten) => {
      cleanup = unlisten
    })

    // Initial check
    checkUpdates(projectPath)

    // Load interval from state and start polling
    tauriApi.getUpdaterState(projectPath).then((state) => {
      if (!state.auto_update_enabled) return
      const ms = state.check_interval_minutes * 60 * 1000
      intervalRef.current = setInterval(() => {
        checkUpdates(projectPath)
      }, ms)
    }).catch(() => {
      // Silently ignore if updater state can't be loaded
    })

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
      cleanup?.()
    }
  }, [projectPath, checkUpdates, initListeners])
}
