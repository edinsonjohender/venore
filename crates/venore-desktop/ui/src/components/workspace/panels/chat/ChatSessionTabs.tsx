// =============================================================================
// ChatSessionTabs - Browser-style session tab bar (matches CanvasHeader design)
// =============================================================================

import { useRef, useState, useCallback, useEffect } from 'react'
import { GitBranch, ChevronLeft, ChevronRight, MoreHorizontal, ExternalLink, Trash2, X as XIcon } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useChatSessionStore, DRAFT_SESSION_ID } from '@/stores/chatSessionStore'
import { useChatStore } from '@/stores/chatStore'
import { tauriApi } from '@/lib/tauri'
import { cn } from '@/lib/utils'

interface ChatSessionTabsProps {
  projectId?: string
  projectPath?: string
}

const SCROLL_STEP = 120

export function ChatSessionTabs({ projectId, projectPath }: ChatSessionTabsProps) {
  const { t } = useTranslation('chat')
  const activeSessionId = useChatSessionStore((s) => s.activeSessionId)
  const switchSession = useChatSessionStore((s) => s.switchSession)
  const loadSessions = useChatSessionStore((s) => s.loadSessions)
  const sessions = useChatSessionStore((s) => s.sessions)
  const poppedOutSessionIds = useChatSessionStore((s) => s.poppedOutSessionIds)
  const openChatTabs = useChatSessionStore((s) => s.openChatTabs)
  const sessionHasMessages = useChatSessionStore((s) => s.sessionHasMessages)
  const closeChatTab = useChatSessionStore((s) => s.closeChatTab)
  const addPoppedOut = useChatSessionStore((s) => s.addPoppedOut)

  const clearMessages = useChatStore((s) => s.clearMessages)

  const getOrCreateEmptySession = useChatSessionStore((s) => s.getOrCreateEmptySession)

  // Load sessions, THEN ensure an active tab — secuencial para que el `set`
  // de `loadSessions` nunca pise la sesión vacía recién creada (la carrera que
  // dejaba el tab bar vacío). El guard de ref evita doble creación en StrictMode.
  const initRef = useRef(false)
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      // list only — no auto-restore of old conversations
      await loadSessions(projectId)
      if (cancelled) return
      // Releer estado fresco tras el await (no las props/closure del montaje)
      const { activeSessionId: active, openChatTabs: tabs } = useChatSessionStore.getState()
      if (active || tabs.length > 0) return
      if (initRef.current) return
      initRef.current = true
      await getOrCreateEmptySession(projectId)
    })()
    return () => { cancelled = true }
  }, [loadSessions, getOrCreateEmptySession, projectId])

  // Build tab items from openChatTabs (exclude popped-out sessions)
  const chatTabItems = openChatTabs
    .map((id) => sessions.find((s) => s.id === id))
    .filter((s): s is NonNullable<typeof s> => !!s)
    .filter((s) => !poppedOutSessionIds.has(s.id))

  const scrollRef = useRef<HTMLDivElement>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)

  const checkOverflow = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    setCanScrollLeft(el.scrollLeft > 0)
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1)
  }, [])

  useEffect(() => {
    checkOverflow()
    const el = scrollRef.current
    if (!el) return
    const ro = new ResizeObserver(checkOverflow)
    ro.observe(el)
    return () => ro.disconnect()
  }, [checkOverflow, chatTabItems.length])

  const scroll = useCallback((dir: -1 | 1) => {
    scrollRef.current?.scrollBy({ left: dir * SCROLL_STEP, behavior: 'smooth' })
  }, [])

  const handleTabClick = (sessionId: string) => {
    // Skip if already the active session (avoids clearing in-memory streaming state)
    if (activeSessionId === sessionId) return
    switchSession(sessionId)
  }

  const handleTabPopOut = async (sessionId: string) => {
    if (!projectPath) return
    const session = sessions.find((s) => s.id === sessionId)
    const name = session?.name ?? 'Chat'

    // Snapshot fully to localStorage BEFORE opening the window (synchronous)
    const snapshot = useChatStore.getState().snapshotForPopout()
    localStorage.setItem(`chat-popout-${sessionId}`, JSON.stringify(snapshot))

    await tauriApi.openChatWindow(sessionId, projectPath, name, projectId)
    addPoppedOut(sessionId)

    clearMessages()
    useChatStore.setState({ isStreaming: false, currentStreamId: null })

    const { openChatTabs: tabs, poppedOutSessionIds: popped } = useChatSessionStore.getState()
    const visible = tabs.filter(
      (id) => id !== sessionId && !popped.has(id)
    )
    if (visible.length > 0) {
      useChatSessionStore.getState().switchSession(visible[0])
    } else {
      useChatSessionStore.setState({ activeSessionId: null })
    }
  }

  const handleTabClear = (sessionId: string) => {
    if (activeSessionId === sessionId) {
      clearMessages()
    }
  }

  return (
    <div className="flex items-center h-9 bg-[hsl(var(--canvas-tab-bar))] shrink-0 relative">
      {/* Scroll left */}
      {canScrollLeft && (
        <button
          onClick={() => scroll(-1)}
          className="shrink-0 w-6 h-full flex items-center justify-center text-foreground-subtle hover:text-foreground transition-colors"
        >
          <ChevronLeft className="w-3 h-3" />
        </button>
      )}

      {/* Tabs */}
      <div
        ref={scrollRef}
        onScroll={checkOverflow}
        className="flex items-center h-full overflow-x-hidden min-w-0"
      >
        {chatTabItems.map((session, i) => {
          const isDev = !!session.dev_session_id
          const isActive = activeSessionId === session.id
          const label = sessionHasMessages[session.id] ? session.name : t('headerActions.newChat')
          // Show divider before this tab if neither this nor the previous tab is active
          const prevIsActive = i > 0 && activeSessionId === chatTabItems[i - 1].id
          const showDivider = i > 0 && !isActive && !prevIsActive

          return (
            <div key={session.id} className="flex items-center h-full">
              {showDivider && (
                <div className="w-px h-4 bg-border shrink-0" />
              )}
              <div
                role="button"
                tabIndex={0}
                onClick={() => handleTabClick(session.id)}
                onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleTabClick(session.id) }}
                className={cn(
                  'group h-full px-3.5 text-xs flex items-center gap-2 shrink-0 max-w-[220px] transition-colors relative cursor-pointer',
                  isActive
                    ? 'bg-background-tertiary text-foreground z-[1]'
                    : 'text-foreground-subtle hover:text-foreground-muted hover:bg-white/[0.04]',
                )}
                title={label}
              >
                <span className="truncate">{label}</span>
                {isDev && <GitBranch className="w-3 h-3 shrink-0 opacity-50" />}
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
                      type="button"
                      onClick={(e) => e.stopPropagation()}
                      className={cn(
                        'shrink-0 w-4 h-4 flex items-center justify-center rounded-sm transition-opacity',
                        'hover:bg-foreground/10',
                        isActive ? 'opacity-40 hover:opacity-80' : 'opacity-0 group-hover:opacity-40 hover:!opacity-80',
                      )}
                    >
                      <MoreHorizontal className="w-3 h-3" />
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" sideOffset={4}>
                    <DropdownMenuItem
                      className="text-xs"
                      onSelect={() => handleTabPopOut(session.id)}
                      disabled={!isActive || !projectPath || session.id === DRAFT_SESSION_ID}
                    >
                      <ExternalLink className="w-3.5 h-3.5 mr-2" />
                      {t('headerActions.popOut')}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      className="text-xs"
                      onSelect={() => handleTabClear(session.id)}
                      disabled={!isActive}
                    >
                      <Trash2 className="w-3.5 h-3.5 mr-2" />
                      {t('headerActions.deleteChat')}
                    </DropdownMenuItem>
                    <DropdownMenuSeparator className="bg-border" />
                    <DropdownMenuItem className="text-xs" onSelect={() => closeChatTab(session.id)}>
                      <XIcon className="w-3.5 h-3.5 mr-2" />
                      {t('headerActions.closeChat')}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </div>
          )
        })}
      </div>

      {/* Scroll right */}
      {canScrollRight && (
        <button
          onClick={() => scroll(1)}
          className="shrink-0 w-6 h-full flex items-center justify-center text-foreground-subtle hover:text-foreground transition-colors"
        >
          <ChevronRight className="w-3 h-3" />
        </button>
      )}

      <div className="flex-1" />

      {/* Bottom line — active tab covers it */}
      <div className="absolute bottom-0 left-0 right-0 h-px bg-border" />
    </div>
  )
}
