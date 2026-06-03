// =============================================================================
// Chat Session Store - Zustand store for chat sessions (conversations)
// =============================================================================

import { create } from 'zustand'
import { listen } from '@tauri-apps/api/event'
import i18n from '@/i18n'
import { tauriApi } from '@/lib/tauri'
import type { ChatSessionDto } from '@/lib/tauri'
import { useChatStore, reconnectToActiveStream } from './chatStore'
import { useTerminalStore } from './terminalStore'
import { useCanvasTabStore } from './canvasTabStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type ChatView = 'chat' | 'history'

/**
 * Sentinel id for the in-memory "draft" session — a New Chat tab that has NOT
 * been persisted yet. Lazy persistence: no `chat_sessions` row is inserted
 * until the first message is sent (see `getOrCreateSendableSession` /
 * `createSession`). This keeps empty chats out of the DB / history entirely.
 * The draft lives in `sessions` + `openChatTabs` like a real session so all
 * the id-keyed UI machinery (tab render, switch, label) works unchanged.
 */
export const DRAFT_SESSION_ID = 'draft'

interface ChatSessionState {
  sessions: ChatSessionDto[]
  activeSessionId: string | null
  activeDevSessionId: string | null
  isLoading: boolean
  chatView: ChatView
  poppedOutSessionIds: Set<string>
  openChatTabs: string[]
  sessionHasMessages: Record<string, boolean>
  /** Per-session in-memory state cache (preserved across tab switches) */
  sessionCache: Record<string, Record<string, unknown>>

  loadSessions: (projectId?: string) => Promise<void>
  createSession: (name?: string, projectId?: string) => Promise<string>
  switchSession: (sessionId: string) => Promise<void>
  deleteSession: (sessionId: string) => Promise<void>
  renameSession: (sessionId: string, name: string) => Promise<void>
  setChatView: (view: ChatView) => void
  openDevSession: (devSessionId: string, sessionName: string, projectId?: string, worktreePath?: string) => Promise<void>
  addPoppedOut: (sessionId: string) => void
  removePoppedOut: (sessionId: string) => void
  openChatTab: (sessionId: string) => void
  closeChatTab: (sessionId: string) => void
  setSessionHasMessages: (sessionId: string, has: boolean) => void
  ensureDraft: () => void
  getOrCreateEmptySession: (projectId?: string) => Promise<string>
  getOrCreateSendableSession: (projectId?: string) => Promise<string>
  autoNameSession: (sessionId: string, firstUserMessage: string) => Promise<void>
  clearSessionCache: (sessionId: string) => void
  updateSessionCache: (sessionId: string, update: Record<string, unknown>) => void
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/** Truncate text at ~50 chars on a word boundary */
function truncateForTabName(text: string, max = 50): string {
  const cleaned = text.replace(/\n/g, ' ').trim()
  if (cleaned.length <= max) return cleaned
  const truncated = cleaned.slice(0, max)
  const lastSpace = truncated.lastIndexOf(' ')
  return (lastSpace > max * 0.4 ? truncated.slice(0, lastSpace) : truncated) + '…'
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

/** Epoch counter — incremented on every session switch to detect stale async loads */
let _switchEpoch = 0

/** Last project loaded — used to detect project switches and reset ephemeral state */
let _lastLoadedProjectId: string | undefined | null = null

export const useChatSessionStore = create<ChatSessionState>()((set, get) => ({
  sessions: [],
  activeSessionId: null,
  activeDevSessionId: null,
  isLoading: false,
  chatView: 'chat',
  poppedOutSessionIds: new Set(),
  openChatTabs: [],
  sessionHasMessages: {},
  sessionCache: {},

  loadSessions: async (projectId?: string) => {
    // Detect project change → full reset of ephemeral state
    if (projectId !== _lastLoadedProjectId) {
      _lastLoadedProjectId = projectId

      // Clear chat messages immediately so old project's messages don't flash
      useChatStore.getState().clearMessages()

      // Reset all ephemeral session state
      set({
        activeSessionId: null,
        activeDevSessionId: null,
        openChatTabs: [],
        sessionHasMessages: {},
        sessionCache: {},
        poppedOutSessionIds: new Set(),
        chatView: 'chat',
      })
    }

    set({ isLoading: true })
    try {
      const sessions = await tauriApi.listChatSessions(projectId)
      // Merge en vez de overwrite: una sesión creada durante este await
      // (p.ej. la sesión vacía inicial) aún no está en la lista de DB. Si la
      // pisáramos, su tab quedaría en `openChatTabs` apuntando a una sesión
      // inexistente → tab invisible. Conservamos las que respaldan un tab abierto.
      set((state) => {
        const dbIds = new Set(sessions.map((s) => s.id))
        const preserved = state.sessions.filter(
          (s) => state.openChatTabs.includes(s.id) && !dbIds.has(s.id),
        )
        return { sessions: [...sessions, ...preserved], isLoading: false }
      })
    } catch (err) {
      console.error('Failed to load sessions:', err)
      set({ isLoading: false })
    }
  },

  // Materialize a real session (DB INSERT). This is the single lazy-persistence
  // point: called from the send path when the active tab is the draft (or none).
  // If a draft tab is open, it's promoted in place — the draft id is swapped for
  // the real id in `openChatTabs`/`sessions`, preserving tab position.
  createSession: async (name?: string, projectId?: string) => {
    try {
      // Snapshot current session's in-memory state before switching (only a real
      // session with content — the draft has nothing worth caching)
      const prevSessionId = get().activeSessionId
      if (prevSessionId && prevSessionId !== DRAFT_SESSION_ID && useChatStore.getState().messages.length > 0) {
        const snapshot = useChatStore.getState().snapshotForPopout()
        set((state) => ({
          sessionCache: { ...state.sessionCache, [prevSessionId]: snapshot },
        }))
      }

      const session = await tauriApi.createChatSession({
        name: name ?? i18n.t('chat:input.newChat'),
        project_id: projectId,
      })

      set((state) => {
        // Swap the draft id for the real id in the tab bar (preserve position).
        // If there's no draft tab, append the new id.
        const hasDraftTab = state.openChatTabs.includes(DRAFT_SESSION_ID)
        const openChatTabs = hasDraftTab
          ? state.openChatTabs.map((id) => (id === DRAFT_SESSION_ID ? session.id : id))
          : state.openChatTabs.includes(session.id)
            ? state.openChatTabs
            : [...state.openChatTabs, session.id]
        // Drop the draft (and any dup) from sessions, prepend the real one.
        const sessions = [
          session,
          ...state.sessions.filter((s) => s.id !== DRAFT_SESSION_ID && s.id !== session.id),
        ]
        const { [DRAFT_SESSION_ID]: _draft, ...sessionHasMessages } = state.sessionHasMessages
        return {
          sessions,
          openChatTabs,
          activeSessionId: session.id,
          sessionHasMessages: { ...sessionHasMessages, [session.id]: false },
        }
      })

      // Promotion happens before the optimistic user message is added, so the
      // draft's (empty) message list carries over cleanly — clear defensively.
      useChatStore.getState().clearMessages()

      return session.id
    } catch (err) {
      console.error('Failed to create session:', err)
      throw err
    }
  },

  switchSession: async (sessionId: string) => {
    // Epoch: if another switch starts while we're awaiting, discard stale results
    const epoch = ++_switchEpoch

    // If leaving a dev session, clear its approvals
    const prevSessionId = get().activeSessionId
    const prevSession = get().sessions.find((s) => s.id === prevSessionId)
    if (prevSession?.dev_session_id && prevSessionId !== sessionId) {
      tauriApi.clearSessionApprovals(prevSession.dev_session_id)
    }

    // Snapshot current session's in-memory state before switching (only a real
    // session with content — never the draft)
    if (prevSessionId && prevSessionId !== DRAFT_SESSION_ID && prevSessionId !== sessionId && useChatStore.getState().messages.length > 0) {
      const snapshot = useChatStore.getState().snapshotForPopout()
      set((state) => ({
        sessionCache: { ...state.sessionCache, [prevSessionId]: snapshot },
      }))
    }

    // Draft target: no DB row exists, so skip the message load entirely —
    // just show a clean empty chat.
    if (sessionId === DRAFT_SESSION_ID) {
      useChatStore.getState().clearMessages()
      set({ activeSessionId: DRAFT_SESSION_ID, activeDevSessionId: null, chatView: 'chat' })
      set((state) => ({
        sessionHasMessages: { ...state.sessionHasMessages, [DRAFT_SESSION_ID]: false },
      }))
      return
    }

    // Clear stale state before loading the target session
    useChatStore.getState().clearMessages()

    // Look up the session to check if it's a dev session
    const session = get().sessions.find((s) => s.id === sessionId)
    const devSessionId = session?.dev_session_id ?? null

    set({ activeSessionId: sessionId, activeDevSessionId: devSessionId, chatView: 'chat' })

    // If it's a dev session, restore the canvas tab and terminal
    if (devSessionId && session) {
      useCanvasTabStore.getState().openSession(devSessionId, session.name)
      useTerminalStore.getState().activateSessionTerminal(devSessionId)
    }

    // Restore from cache if available (preserves streaming state, tool calls, etc.),
    // otherwise load from DB
    const cached = get().sessionCache[sessionId]
    if (cached) {
      useChatStore.setState(cached as Partial<ReturnType<typeof useChatStore.getState>>)
      // Remove consumed cache entry
      set((state) => {
        const { [sessionId]: _, ...rest } = state.sessionCache
        return { sessionCache: rest }
      })
    } else {
      await useChatStore.getState().loadMessages(sessionId)
      // Discard if another switch happened during the async load
      if (epoch !== _switchEpoch) return
    }

    // Track whether this session has content
    const msgCount = useChatStore.getState().messages.length
    set((state) => ({
      sessionHasMessages: { ...state.sessionHasMessages, [sessionId]: msgCount > 0 },
    }))
  },

  deleteSession: async (sessionId: string) => {
    // The draft has no DB row — "deleting" it is just dropping the in-memory tab.
    if (sessionId === DRAFT_SESSION_ID) {
      get().closeChatTab(DRAFT_SESSION_ID)
      return
    }
    try {
      await tauriApi.deleteChatSession(sessionId)

      const { activeSessionId, sessions, openChatTabs } = get()
      const remaining = sessions.filter((s) => s.id !== sessionId)
      const remainingTabs = openChatTabs.filter((id) => id !== sessionId)

      set((state) => {
        const { [sessionId]: _, ...restHasMessages } = state.sessionHasMessages
        const { [sessionId]: _c, ...restCache } = state.sessionCache
        return {
          sessions: remaining,
          openChatTabs: remainingTabs,
          sessionHasMessages: restHasMessages,
          sessionCache: restCache,
        }
      })

      // If the deleted session was active, switch to the first remaining tab,
      // or fall back to a fresh draft (never leave the panel session-less).
      if (activeSessionId === sessionId) {
        const nextTab = remainingTabs[0] ?? remaining[0]?.id
        if (nextTab) {
          await get().switchSession(nextTab)
        } else {
          get().ensureDraft()
        }
      }
    } catch (err) {
      console.error('Failed to delete session:', err)
    }
  },

  renameSession: async (sessionId: string, name: string) => {
    try {
      await tauriApi.renameChatSession(sessionId, name)

      set((state) => ({
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? { ...s, name } : s,
        ),
      }))
    } catch (err) {
      console.error('Failed to rename session:', err)
    }
  },

  setChatView: (view) => set({ chatView: view }),

  openDevSession: async (devSessionId, sessionName, projectId, worktreePath) => {
    try {
      const chatSession = await tauriApi.getOrCreateDevSessionChat({
        dev_session_id: devSessionId,
        session_name: sessionName,
        project_id: projectId,
      })

      // Ensure session is in list
      set((state) => ({
        sessions: state.sessions.some((s) => s.id === chatSession.id)
          ? state.sessions
          : [chatSession, ...state.sessions],
      }))

      // Open as chat tab
      get().openChatTab(chatSession.id)

      // Open canvas tab (idempotent — worktreePath only matters on first open)
      useCanvasTabStore.getState().openSession(devSessionId, sessionName, worktreePath)

      // Delegate to unified switch
      await get().switchSession(chatSession.id)
    } catch (err) {
      console.error('Failed to open dev session:', err)
    }
  },

  addPoppedOut: (sessionId) => set((state) => {
    const next = new Set(state.poppedOutSessionIds)
    next.add(sessionId)
    return { poppedOutSessionIds: next }
  }),

  removePoppedOut: (sessionId) => set((state) => {
    const next = new Set(state.poppedOutSessionIds)
    next.delete(sessionId)
    return { poppedOutSessionIds: next }
  }),

  openChatTab: (sessionId) => set((state) => {
    if (state.openChatTabs.includes(sessionId)) return state
    return { openChatTabs: [...state.openChatTabs, sessionId] }
  }),

  closeChatTab: (sessionId) => {
    const { openChatTabs, activeSessionId, sessions } = get()
    const idx = openChatTabs.indexOf(sessionId)
    if (idx < 0) return

    // If it's a dev session, clean up canvas tab + approvals
    const session = sessions.find((s) => s.id === sessionId)
    if (session?.dev_session_id) {
      useCanvasTabStore.getState().closeTab(`session-${session.dev_session_id}`)
      tauriApi.clearSessionApprovals(session.dev_session_id)
    }

    const remaining = openChatTabs.filter((id) => id !== sessionId)
    // Closing the draft also drops it from `sessions` (it's not a DB row).
    set((state) => ({
      openChatTabs: remaining,
      sessions: sessionId === DRAFT_SESSION_ID
        ? state.sessions.filter((s) => s.id !== DRAFT_SESSION_ID)
        : state.sessions,
    }))

    // If we closed the active tab, switch to an adjacent one
    if (activeSessionId === sessionId) {
      if (remaining.length > 0) {
        const nextIdx = Math.min(idx, remaining.length - 1)
        get().switchSession(remaining[nextIdx])
      } else {
        // No tabs left — fall back to a fresh draft (no DB row).
        get().ensureDraft()
      }
    }
  },

  setSessionHasMessages: (sessionId, has) => set((state) => ({
    sessionHasMessages: { ...state.sessionHasMessages, [sessionId]: has },
  })),

  // Ensure the in-memory draft tab exists and is active. Idempotent — at most
  // one draft. No DB row is created (that's `createSession`, on first send).
  ensureDraft: () => {
    set((state) => {
      const hasDraft = state.sessions.some((s) => s.id === DRAFT_SESSION_ID)
      const draft: ChatSessionDto = {
        id: DRAFT_SESSION_ID,
        name: i18n.t('chat:input.newChat'),
        project_id: null,
        dev_session_id: null,
        created_at: '',
        updated_at: '',
      }
      return {
        sessions: hasDraft ? state.sessions : [draft, ...state.sessions],
        openChatTabs: state.openChatTabs.includes(DRAFT_SESSION_ID)
          ? state.openChatTabs
          : [...state.openChatTabs, DRAFT_SESSION_ID],
        activeSessionId: DRAFT_SESSION_ID,
        activeDevSessionId: null,
        chatView: 'chat',
        sessionHasMessages: { ...state.sessionHasMessages, [DRAFT_SESSION_ID]: false },
      }
    })
    useChatStore.getState().clearMessages()
  },

  // "New Chat" affordance (header button, init). Reuses any empty tab; otherwise
  // drops to a draft. Never inserts a DB row — empty chats stay out of history.
  getOrCreateEmptySession: async (projectId?: string) => {
    void projectId
    const { openChatTabs, sessionHasMessages, activeSessionId } = get()

    // If current tab is already empty, just use it
    if (activeSessionId && !sessionHasMessages[activeSessionId]) {
      return activeSessionId
    }

    // Look for an existing empty tab
    const emptyTab = openChatTabs.find((id) => !sessionHasMessages[id])
    if (emptyTab) {
      await get().switchSession(emptyTab)
      return emptyTab
    }

    // No empty tab — fall back to a draft (materialized later, on first send)
    get().ensureDraft()
    return DRAFT_SESSION_ID
  },

  // Used by the send paths: return a REAL (persisted) session id, materializing
  // the draft if needed. This is where lazy persistence turns into a DB row.
  getOrCreateSendableSession: async (projectId?: string) => {
    const { activeSessionId } = get()
    if (activeSessionId && activeSessionId !== DRAFT_SESSION_ID) {
      return activeSessionId
    }
    return get().createSession(undefined, projectId)
  },

  clearSessionCache: (sessionId) => set((state) => {
    const { [sessionId]: _, ...rest } = state.sessionCache
    return { sessionCache: rest }
  }),

  updateSessionCache: (sessionId, update) => set((state) => {
    const existing = state.sessionCache[sessionId]
    if (!existing) return state // only patch if session was previously cached
    return { sessionCache: { ...state.sessionCache, [sessionId]: { ...existing, ...update } } }
  }),

  autoNameSession: async (sessionId, firstUserMessage) => {
    const session = get().sessions.find((s) => s.id === sessionId)
    if (!session) return
    const defaultName = i18n.t('chat:input.newChat')
    if (session.name !== defaultName) return

    // Show truncated name immediately, then replace with LLM title
    const fallback = truncateForTabName(firstUserMessage)
    if (fallback) {
      get().renameSession(sessionId, fallback)
    }

    // Generate smart title via dedicated backend command (uses user's configured provider/model)
    tauriApi.generateChatTitle(firstUserMessage)
      .then((title) => {
        const trimmed = title?.trim()
        if (trimmed) {
          get().renameSession(sessionId, trimmed)
        }
      })
      .catch((err) => {
        console.warn('[autoNameSession] LLM title generation failed:', err)
      })
  },
}))

// Listen for pop-out window close events (singleton — runs once on module load)
listen<{ sessionId: string }>('chat-popout-closed', async (event) => {
  const sessionId = event.payload.sessionId
  const store = useChatSessionStore.getState()
  store.removePoppedOut(sessionId)

  const key = `chat-popout-${sessionId}`
  const stored = localStorage.getItem(key)

  if (stored) {
    localStorage.removeItem(key)
    try {
      const snapshot = JSON.parse(stored)

      // Cache current session's in-memory state before overwriting
      const currentSessionId = store.activeSessionId
      if (currentSessionId && currentSessionId !== sessionId) {
        const chatState = useChatStore.getState()
        if (chatState.messages.length > 0) {
          useChatSessionStore.setState((state) => ({
            sessionCache: { ...state.sessionCache, [currentSessionId]: chatState.snapshotForPopout() },
          }))
        }
      }

      useChatStore.setState({
        ...snapshot,
        isStreaming: false,
        currentStreamId: null,
        pendingConfirm: null,
        pendingAskUser: null,
        pendingPlan: null,
      })

      useChatSessionStore.setState({
        activeSessionId: sessionId,
        activeDevSessionId: null,
        chatView: 'chat',
        sessionHasMessages: {
          ...store.sessionHasMessages,
          [sessionId]: Array.isArray(snapshot.messages) && snapshot.messages.length > 0,
        },
      })

      await reconnectToActiveStream(sessionId)
      return
    } catch (e) {
      console.error('[chat-popout-closed] Failed to restore from localStorage:', e)
    }
  }

  // Fallback: load from DB + reconnect stream
  await useChatStore.getState().loadMessages(sessionId)
  useChatSessionStore.setState({ activeSessionId: sessionId, chatView: 'chat' })
  await reconnectToActiveStream(sessionId)
})
