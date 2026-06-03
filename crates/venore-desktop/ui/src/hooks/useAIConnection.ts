// =============================================================================
// useAIConnection - Register an AI connection on mount; do NOT auto-unregister
// =============================================================================
// Why no auto-unregister:
//   The connection state (rainbow-border + animated line) is driven by the
//   user's explicit toggle, not by component lifetime. When a node pops out
//   to its own OS window, the in-app FloatingNodePanel unmounts but the
//   connection must survive into the new window. Tying unregister to the
//   React unmount would kill the connection mid-pop-out.
//
//   Explicit close paths (X button on a panel, X-close on a NodeWindow)
//   call `unregisterConnection` on the corresponding store/listener to
//   keep the "close panel = drop connection" UX intact.

import { useEffect } from 'react'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import type { AiConnectionTarget } from '@/lib/tauri'

export function useAIConnection(connectionId: string, target: AiConnectionTarget) {
  const registerConnection = useAIConnectionStore((s) => s.registerConnection)
  const isActive = useAIConnectionStore(
    (s) => s.connections[connectionId]?.active ?? false,
  )
  const toggleConnection = useAIConnectionStore((s) => s.toggleConnection)

  useEffect(() => {
    registerConnection(connectionId, target)
    // We deliberately want to re-register if the *kind* of target changes
    // (e.g. a panel switches between knowledge_node and code_module
    // semantics), but not on every render because of object identity. The
    // serialized form is a stable surrogate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionId, registerConnection, JSON.stringify(target)])

  return { isActive, toggle: () => toggleConnection(connectionId) }
}
