// =============================================================================
// useMeshInit — Single call to backend + event listeners
// =============================================================================

import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Window } from '@tauri-apps/api/window'
import { tauriApi, type MeshPeerInfo, type MeshTransportStatus } from '@/lib/tauri'
import { useMeshStore } from '@/stores/meshStore'

/// React 18 StrictMode in dev mounts → cleans up → mounts each effect to
/// verify it's idempotent. Without absorption, our cleanup would
/// unregister the project for ~30ms between the two mounts, briefly
/// removing this window from every other peer's discovery. The debounce
/// holds the unregister long enough that the re-mount can cancel it.
const STRICTMODE_DEBOUNCE_MS = 200

export function useMeshInit(projectPath: string, projectId?: string) {
  // Persists across re-mounts in the same window so StrictMode's
  // cleanup → mount cycle can cancel a pending unregister.
  const pendingUnregisterRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (!projectId) return

    // If a previous cleanup scheduled an unregister for this same
    // window, the re-mount cancels it (StrictMode case).
    if (pendingUnregisterRef.current !== null) {
      clearTimeout(pendingUnregisterRef.current)
      pendingUnregisterRef.current = null
    }

    const projectName = projectPath.split(/[\\/]/).pop() || 'unknown'

    // Mark this window's project so the store can filter "self" out of
    // the peer list — backend broadcasts include all local peers (needed
    // for single-process multi-window).
    useMeshStore.getState().setMyProjectId(projectId)

    // One call — backend handles register + transport + handler + auto-connect + loop
    tauriApi.meshInit(projectId, projectName, projectPath).catch((e) => {
      console.warn('[Mesh] Init failed:', e)
      useMeshStore.getState().setError(String(e))
    })

    // Listen for events from the backend (replaces polling).
    // setPeers in the store filters our own project_id out of the
    // broadcast so we never see "self" as a peer.
    const unsubs = [
      listen<MeshPeerInfo[]>('mesh:peers-updated', (e) => {
        useMeshStore.getState().setPeers(e.payload)
      }),
      listen<MeshTransportStatus>('mesh:status-updated', (e) => {
        useMeshStore.getState().setStatus(e.payload)
      }),
      listen<{ message: string }>('mesh:error', (e) => {
        useMeshStore.getState().setError(e.payload.message)
      }),
    ]

    // Window close (Alt+F4, click X): unregister immediately (no
    // debounce — there will be no re-mount to absorb). Tauri 2 awaits
    // the callback before actually closing.
    const closeUnlistenPromise = Window.getCurrent().onCloseRequested(async () => {
      if (pendingUnregisterRef.current !== null) {
        clearTimeout(pendingUnregisterRef.current)
        pendingUnregisterRef.current = null
      }
      try {
        await tauriApi.meshUnregisterProject(projectId)
      } catch (e) {
        console.warn('[Mesh] Unregister on close failed:', e)
      }
    })

    // Cleanup: unsubscribe from events and SCHEDULE (not execute) the
    // unregister. If the effect re-mounts within the debounce window
    // (StrictMode), the next mount cancels the scheduled call and the
    // peer never disappears from anyone's view. Real unmounts (project
    // change, navigate away) wait the full debounce, which is invisible
    // to the user.
    return () => {
      unsubs.forEach((p) => p.then((fn) => fn()))
      closeUnlistenPromise.then((fn) => fn())
      useMeshStore.getState().setMyProjectId(null)

      pendingUnregisterRef.current = setTimeout(() => {
        pendingUnregisterRef.current = null
        tauriApi.meshUnregisterProject(projectId).catch((e) => {
          console.warn('[Mesh] Unregister on cleanup failed:', e)
        })
      }, STRICTMODE_DEBOUNCE_MS)
    }
  }, [projectPath, projectId])
}
