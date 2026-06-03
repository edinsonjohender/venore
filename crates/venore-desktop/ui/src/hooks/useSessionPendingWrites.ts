// =============================================================================
// useSessionPendingWrites — list of AI write proposals scoped to a chat session
// =============================================================================
// Pending writes are session-scoped on the backend (each pending entry
// carries a session_id captured at proposal time). This hook keeps a
// reactive list synced with the registry so chat-side UI (bulk
// accept/discard bar, count chip) doesn't need to chase events itself.
//
// Refresh triggers:
//   - sessionId changes (new chat opened) — initial load.
//   - `ai-write-proposed` event fires for any node — covers create,
//     edit, regenerate (which re-emits) and discard (also re-emits).
//   - `ocean-knowledge-changed` — covers accept (which emits this).

import { useEffect, useState, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { tauriApi, type PendingWriteDto } from '@/lib/tauri'

export function useSessionPendingWrites(sessionId: string | null) {
  const [writes, setWrites] = useState<PendingWriteDto[]>([])

  const refresh = useCallback(async () => {
    if (!sessionId) {
      setWrites([])
      return
    }
    try {
      const res = await tauriApi.listSessionPendingWrites({ session_id: sessionId })
      setWrites(res.writes)
    } catch (err) {
      console.error('listSessionPendingWrites failed', err)
    }
  }, [sessionId])

  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    if (!sessionId) return
    let cancelled = false
    const u1 = listen('ai-write-proposed', () => {
      if (!cancelled) refresh()
    })
    const u2 = listen('ocean-knowledge-changed', () => {
      if (!cancelled) refresh()
    })
    return () => {
      cancelled = true
      u1.then((fn) => fn())
      u2.then((fn) => fn())
    }
  }, [sessionId, refresh])

  return { writes, refresh }
}
