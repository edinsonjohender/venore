// =============================================================================
// useAutoDisconnectAiOnContextChange — clear AI attachments on context switch
// =============================================================================
// AI connections are conversation-scoped: a node attached while talking about
// project A on session α makes no sense once you switch to project B or open
// session β. This hook watches both signals and disconnects everything when
// either changes (skipping the very first render so opening the workspace
// doesn't wipe a fresh setup).

import { useEffect, useRef } from 'react'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { useChatSessionStore } from '@/stores/chatSessionStore'

export function useAutoDisconnectAiOnContextChange(projectPath: string | undefined) {
  const activeSessionId = useChatSessionStore((s) => s.activeSessionId)
  const initialised = useRef(false)
  const lastProjectPath = useRef<string | undefined>(projectPath)
  const lastSessionId = useRef<string | null>(activeSessionId)

  useEffect(() => {
    if (!initialised.current) {
      initialised.current = true
      lastProjectPath.current = projectPath
      lastSessionId.current = activeSessionId
      return
    }
    const projectChanged = lastProjectPath.current !== projectPath
    const sessionChanged = lastSessionId.current !== activeSessionId
    if (projectChanged || sessionChanged) {
      useAIConnectionStore.getState().disconnectAll()
    }
    lastProjectPath.current = projectPath
    lastSessionId.current = activeSessionId
  }, [projectPath, activeSessionId])
}
