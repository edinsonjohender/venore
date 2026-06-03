import { useEffect, useRef } from 'react'
import { Window } from '@tauri-apps/api/window'
import { emitTo } from '@tauri-apps/api/event'
import { useChatStore, reconnectToActiveStream } from '@/stores/chatStore'
import { useChatSessionStore } from '@/stores/chatSessionStore'
import { ChatMessages } from '@/components/workspace/panels/chat/ChatMessages'
import { ChatInput } from '@/components/workspace/panels/chat/ChatInput'
import { ChatSessionHistory } from '@/components/workspace/panels/chat/ChatSessionHistory'
import { TitleBar } from '@/components/TitleBar'

interface ChatWindowProps {
  sessionId: string
  sessionName: string
  projectPath: string
  projectId?: string
}

export function ChatWindow({ sessionId, sessionName, projectPath, projectId }: ChatWindowProps) {
  const chatView = useChatSessionStore((s) => s.chatView)
  const hydratedRef = useRef(false)

  // Hydrate from localStorage (primary, fast, complete) or DB (fallback).
  // Guard with ref: React 18 StrictMode double-fires effects.
  useEffect(() => {
    if (hydratedRef.current) return
    hydratedRef.current = true

    const hydrate = async () => {
      const key = `chat-popout-${sessionId}`
      const stored = localStorage.getItem(key)

      if (stored) {
        localStorage.removeItem(key)
        try {
          const snapshot = JSON.parse(stored)
          useChatStore.setState({
            ...snapshot,
            // Clear overlays — oneshot channels don't cross windows
            isStreaming: false,
            currentStreamId: null,
            pendingConfirm: null,
            pendingAskUser: null,
            pendingPlan: null,
          })
          useChatSessionStore.setState({ activeSessionId: sessionId, chatView: 'chat' })
          await reconnectToActiveStream(sessionId)
          return
        } catch (e) {
          console.error('[ChatWindow] Failed to hydrate from localStorage:', e)
        }
      }

      // Fallback: no localStorage (session reopened, completed)
      await useChatStore.getState().loadMessages(sessionId)
      useChatSessionStore.setState({ activeSessionId: sessionId, chatView: 'chat' })
      await reconnectToActiveStream(sessionId)
    }

    hydrate()
  }, [sessionId])

  // On close: snapshot state to localStorage, then notify main window
  useEffect(() => {
    const currentWindow = Window.getCurrent()
    const unlistenPromise = currentWindow.onCloseRequested(async (event) => {
      event.preventDefault()
      // Serialize complete state so main window can restore it
      const snapshot = useChatStore.getState().snapshotForPopout()
      localStorage.setItem(`chat-popout-${sessionId}`, JSON.stringify(snapshot))
      await emitTo('main', 'chat-popout-closed', { sessionId })
      currentWindow.destroy()
    })

    return () => {
      unlistenPromise.then((unlisten) => unlisten())
    }
  }, [sessionId])

  const titleContent = (
    <div className="flex items-center h-full shrink-0 min-w-0">
      <span className="text-xs text-foreground-subtle select-none">-</span>
      <span className="text-xs text-foreground-muted select-none truncate pl-2">
        {sessionName}
      </span>
    </div>
  )

  if (chatView === 'history') {
    return (
      <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
        <TitleBar>{titleContent}</TitleBar>
        <ChatSessionHistory projectId={projectId} />
      </div>
    )
  }

  return (
    <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
      <TitleBar>{titleContent}</TitleBar>
      <ChatMessages />
      <ChatInput projectPath={projectPath} projectId={projectId} />
    </div>
  )
}
