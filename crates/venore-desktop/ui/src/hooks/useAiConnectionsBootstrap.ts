// =============================================================================
// useAiConnectionsBootstrap - Mirror backend AI connection state into the store
// =============================================================================
// Subscribes once to the "ai-connection:update" Tauri event and seeds the
// store with the current snapshot. Mounts in App (and later in NodeWindow)
// so every Tauri webview reflects the same registry.

import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { tauriApi, type AiConnectionDto } from '@/lib/tauri'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'

const UPDATE_EVENT = 'ai-connection:update'

export function useAiConnectionsBootstrap() {
  const applySnapshot = useAIConnectionStore((s) => s.applySnapshot)

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false

    // Subscribe first so we don't miss updates that fire during the
    // initial fetch.
    listen<AiConnectionDto[]>(UPDATE_EVENT, (event) => {
      applySnapshot(event.payload)
    })
      .then((stop) => {
        if (cancelled) {
          stop()
        } else {
          unlisten = stop
        }
      })
      .catch((err) => {
        console.error('Failed to subscribe to ai-connection:update', err)
      })

    tauriApi
      .listAiConnections()
      .then((snapshot) => {
        if (!cancelled) applySnapshot(snapshot)
      })
      .catch((err) => {
        console.error('Failed to fetch initial ai connections', err)
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [applySnapshot])
}
