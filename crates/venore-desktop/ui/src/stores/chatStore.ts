// =============================================================================
// Chat Store - Zustand store for chat messages and streaming state
// =============================================================================

import { create } from 'zustand'
import { listen } from '@tauri-apps/api/event'
import i18n from '@/i18n'
import { tauriApi, VenoreError } from '@/lib/tauri'
import { useChatSessionStore } from './chatSessionStore'
import type {
  ChatMessageInput,
  ChatMessageDto,
  ChatStreamDeltaPayload,
  ChatStreamDonePayload,
  ChatStreamErrorPayload,
  ChatToolCallPayload,
  ChatToolResultPayload,
  ChatToolConfirmPayload,
  ChatSnapshotPayload,
  ChatCompactedPayload,
  ChatAskUserEventPayload,
  ChatTaskUpdateEventPayload,
  ChatPlanReadyEventPayload,
  ChatSubAgentEventPayload,
  SnapshotDto,
} from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type MessageRole = 'user' | 'assistant' | 'system'

export type ToolCallStatus = 'pending' | 'running' | 'completed' | 'denied' | 'error'

export interface ToolCallInfo {
  id: string
  name: string
  arguments: Record<string, unknown>
  status: ToolCallStatus
  result?: string
  commitHash?: string
}

export interface AttachmentDisplay {
  name: string
  mimeType: string
  thumbnailUrl: string | null // data: URL for images
}

export interface ChatMessage {
  id: string
  role: MessageRole
  content: string
  timestamp: number
  isStreaming?: boolean
  toolCalls?: ToolCallInfo[]
  attachments?: AttachmentDisplay[]
}

// Agent interaction types (ask_user, tasks, plan, sub-agents)

export interface AskUserOption {
  label: string
  description: string | null
}

export interface AskUserPayload {
  tool_call_id: string
  question: string
  options: AskUserOption[]
}

export interface TaskItemPayload {
  id: string
  subject: string
  status: string
  description: string
}

export interface PlanReadyPayload {
  tool_call_id: string
  summary: string
  steps: string[]
}

export interface SubAgentPayload {
  agent_id: string
  agent_type: string
  task: string
  status: string // "started" | "completed" | "failed"
  result: string | null
}

export interface ProviderInfo {
  provider: string
  model: string
}

export interface TokenUsageInfo {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

export interface ChatError {
  message: string
  code: string
}

interface ChatStoreState {
  messages: ChatMessage[]
  isStreaming: boolean
  currentStreamId: string | null
  error: ChatError | null
  providerInfo: ProviderInfo | null
  tokenUsage: TokenUsageInfo | null
  pendingConfirm: ChatToolConfirmPayload | null

  // Persisted snapshots (tool_call_id → commit_hash) loaded from DB
  snapshots: SnapshotDto[]

  // Compaction state
  lastCompaction: { action: string; tokens_saved: number } | null

  // Agent interaction state
  pendingAskUser: AskUserPayload | null
  tasks: TaskItemPayload[]
  pendingPlan: PlanReadyPayload | null
  subAgents: SubAgentPayload[]

  // Message actions
  addMessage: (role: MessageRole, content: string, attachments?: AttachmentDisplay[]) => string
  appendToLastMessage: (chunk: string) => void
  setStreaming: (streaming: boolean) => void
  clearMessages: () => void

  // Streaming actions
  sendMessage: (content: string, sessionId?: string | null, projectPath?: string | null, contextModules?: Array<{ name: string; path: string }>, devSessionId?: string | null, attachments?: Array<{ name: string; mime_type: string; data_base64: string }>, knowledgeFeatureId?: string | null) => Promise<void>
  stopStreaming: () => Promise<void>

  // Tool call actions
  approveToolCall: (toolCallId: string, approved: boolean, allowSession?: boolean) => Promise<void>

  // Agent interaction actions
  respondToAskUser: (toolCallId: string, response: string) => Promise<void>
  approvePlan: (toolCallId: string, approved: boolean) => Promise<void>

  // State actions
  setError: (message: string, code?: string) => void
  setProviderInfo: (info: ProviderInfo) => void
  setTokenUsage: (usage: TokenUsageInfo) => void

  // Session actions
  loadMessages: (sessionId: string) => Promise<void>

  // Revert actions
  revertToSnapshot: (devSessionId: string, commitHash: string, messageId?: string) => Promise<void>

  // Pop-out state transfer
  snapshotForPopout: () => Record<string, unknown>
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

let _counter = 0

function nextId(): string {
  return `msg-${Date.now()}-${++_counter}`
}

// Inactivity timeout: auto-recover if no stream activity for 3 minutes
let _lastStreamActivity = 0
let _inactivityIntervalId: ReturnType<typeof setInterval> | null = null
const INACTIVITY_TIMEOUT_MS = 3 * 60 * 1000

/** Reverse-scan messages, find a tool call by ID, merge patch. Returns original ref if no match. */
function updateToolCallById(
  messages: ChatMessage[],
  toolCallId: string,
  patch: Partial<ToolCallInfo>,
): ChatMessage[] {
  const msgs = [...messages]
  for (let i = msgs.length - 1; i >= 0; i--) {
    const msg = msgs[i]
    if (!msg.toolCalls) continue
    const idx = msg.toolCalls.findIndex((tc) => tc.id === toolCallId)
    if (idx >= 0) {
      const updated = [...msg.toolCalls]
      updated[idx] = { ...updated[idx], ...patch }
      msgs[i] = { ...msg, toolCalls: updated }
      return msgs
    }
  }
  return messages
}

// -----------------------------------------------------------------------------
// Stream Listeners (registered once, outside React lifecycle)
// -----------------------------------------------------------------------------

let _listenersReady: Promise<void> | null = null

function setupStreamListeners(): Promise<void> {
  if (_listenersReady) return _listenersReady

  _listenersReady = Promise.all([
    listen<ChatStreamDeltaPayload>('chat-stream-delta', (event) => {
      const { session_id, content } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      _lastStreamActivity = Date.now()
      useChatStore.getState().appendToLastMessage(content)
    }),
    listen<ChatStreamDonePayload>('chat-stream-done', (event) => {
      _lastStreamActivity = 0
      const { session_id, provider, model, prompt_tokens, completion_tokens, total_tokens } =
        event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) {
        // Stream finished for a non-active session — clear its cached state so
        // switching back will load the (now-complete) conversation from DB
        if (session_id) {
          useChatSessionStore.getState().clearSessionCache(session_id)
          useChatSessionStore.getState().setSessionHasMessages(session_id, true)
        }
        return
      }
      // Force any remaining "running" tool calls and sub-agents to "completed"
      useChatStore.setState((state) => {
        const msgs = [...state.messages]
        for (let i = msgs.length - 1; i >= 0; i--) {
          const msg = msgs[i]
          if (!msg.toolCalls) continue
          const hasRunning = msg.toolCalls.some((tc) => tc.status === 'running')
          if (hasRunning) {
            msgs[i] = {
              ...msg,
              toolCalls: msg.toolCalls.map((tc) =>
                tc.status === 'running' ? { ...tc, status: 'completed' as ToolCallStatus } : tc,
              ),
            }
          }
        }
        // Mark any "started" sub-agents as "completed" — stream is done
        const subAgents = state.subAgents.some((a) => a.status === 'started')
          ? state.subAgents.map((a) => a.status === 'started' ? { ...a, status: 'completed' } : a)
          : state.subAgents
        return { messages: msgs, subAgents }
      })
      // Clear any pending overlays — stream is done, backend no longer waiting
      useChatStore.setState({ pendingConfirm: null, pendingAskUser: null, pendingPlan: null })
      useChatStore.getState().setStreaming(false)
      useChatStore.getState().setProviderInfo({ provider, model })
      useChatStore.getState().setTokenUsage({ prompt_tokens, completion_tokens, total_tokens })

      // Mark session as having messages + trigger auto-naming
      const sessionStore = useChatSessionStore.getState()
      const sessionId = sessionStore.activeSessionId
      if (sessionId) {
        sessionStore.setSessionHasMessages(sessionId, true)
        const firstUser = useChatStore.getState().messages.find((m) => m.role === 'user')
        if (firstUser) {
          sessionStore.autoNameSession(sessionId, firstUser.content)
        }
      }
    }),
    listen<ChatStreamErrorPayload>('chat-stream-error', (event) => {
      _lastStreamActivity = 0
      const { session_id, message, code } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) {
        if (session_id) useChatSessionStore.getState().clearSessionCache(session_id)
        return
      }
      useChatStore.getState().setStreaming(false)
      useChatStore.getState().setError(message, code)
      useChatStore.setState((state) => ({
        pendingConfirm: null, pendingAskUser: null, pendingPlan: null,
        subAgents: state.subAgents.some((a) => a.status === 'started')
          ? state.subAgents.map((a) => a.status === 'started' ? { ...a, status: 'failed' } : a)
          : state.subAgents,
      }))
    }),
    listen<ChatToolCallPayload>('chat-tool-call', (event) => {
      const { session_id, tool_call_id, tool_name, arguments: args } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      // Add tool call to the last assistant message
      useChatStore.setState((state) => {
        const msgs = [...state.messages]
        const last = msgs[msgs.length - 1]
        if (!last || last.role !== 'assistant') return state
        const tc: ToolCallInfo = { id: tool_call_id, name: tool_name, arguments: args, status: 'running' }
        msgs[msgs.length - 1] = { ...last, toolCalls: [...(last.toolCalls ?? []), tc] }
        return { messages: msgs }
      })
    }),
    listen<ChatToolResultPayload>('chat-tool-result', (event) => {
      const { session_id, tool_call_id, success, output } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      useChatStore.setState((state) => ({
        messages: updateToolCallById(state.messages, tool_call_id, {
          status: success ? 'completed' : 'error',
          result: output,
        }),
      }))
    }),
    listen<ChatToolConfirmPayload>('chat-tool-confirm', (event) => {
      const { session_id } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id) return
      if (session_id !== activeSessionId) {
        useChatSessionStore.getState().updateSessionCache(session_id, { pendingConfirm: event.payload })
        return
      }
      useChatStore.setState({ pendingConfirm: event.payload })
    }),
    listen<ChatAskUserEventPayload>('chat-ask-user', (event) => {
      const { session_id, tool_call_id, question, options } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id) return
      const payload = { tool_call_id, question, options }
      if (session_id !== activeSessionId) {
        useChatSessionStore.getState().updateSessionCache(session_id, { pendingAskUser: payload })
        return
      }
      useChatStore.setState({ pendingAskUser: payload })
    }),
    listen<ChatTaskUpdateEventPayload>('chat-task-update', (event) => {
      const { session_id, tasks } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      useChatStore.setState({ tasks })
    }),
    listen<ChatPlanReadyEventPayload>('chat-plan-ready', (event) => {
      const { session_id, tool_call_id, summary, steps } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id) return
      const payload = { tool_call_id, summary, steps }
      if (session_id !== activeSessionId) {
        useChatSessionStore.getState().updateSessionCache(session_id, { pendingPlan: payload })
        return
      }
      useChatStore.setState({ pendingPlan: payload })
    }),
    listen<ChatSubAgentEventPayload>('chat-sub-agent', (event) => {
      const { session_id, agent_id, agent_type, task, status, result } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      useChatStore.setState((state) => {
        const existing = state.subAgents.findIndex((a) => a.agent_id === agent_id)
        const payload: SubAgentPayload = { agent_id, agent_type, task, status, result }
        if (existing >= 0) {
          const updated = [...state.subAgents]
          updated[existing] = payload
          return { subAgents: updated }
        }
        return { subAgents: [...state.subAgents, payload] }
      })
    }),
    listen<ChatSnapshotPayload>('chat-snapshot', (event) => {
      const { session_id, tool_call_id, commit_hash } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      useChatStore.setState((state) => ({
        messages: updateToolCallById(state.messages, tool_call_id, { commitHash: commit_hash }),
      }))
    }),
    listen<ChatCompactedPayload>('chat-compacted', (event) => {
      const { session_id, action, tokens_saved } = event.payload
      const activeSessionId = useChatSessionStore.getState().activeSessionId
      if (!session_id || session_id !== activeSessionId) return
      useChatStore.setState({ lastCompaction: { action, tokens_saved } })
    }),
  ]).then(() => void 0)

  return _listenersReady
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useChatStore = create<ChatStoreState>()((set, get) => ({
  messages: [],
  isStreaming: false,
  currentStreamId: null,
  error: null,
  providerInfo: null,
  tokenUsage: null,
  pendingConfirm: null,
  snapshots: [],
  lastCompaction: null,
  pendingAskUser: null,
  tasks: [],
  pendingPlan: null,
  subAgents: [],

  addMessage: (role, content, attachments?) => {
    const id = nextId()
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id,
          role,
          content,
          timestamp: Date.now(),
          isStreaming: role === 'assistant' && state.isStreaming,
          ...(attachments && attachments.length > 0 ? { attachments } : {}),
        },
      ],
    }))
    return id
  },

  appendToLastMessage: (chunk) =>
    set((state) => {
      const msgs = [...state.messages]
      const last = msgs[msgs.length - 1]
      if (!last) return state
      msgs[msgs.length - 1] = { ...last, content: last.content + chunk }
      return { messages: msgs }
    }),

  setStreaming: (streaming) =>
    set((state) => {
      if (!streaming && state.messages.length > 0) {
        const msgs = [...state.messages]
        const last = msgs[msgs.length - 1]
        msgs[msgs.length - 1] = { ...last, isStreaming: false }
        return { isStreaming: false, currentStreamId: null, messages: msgs }
      }
      return { isStreaming: streaming }
    }),

  clearMessages: () => set({ messages: [], snapshots: [], lastCompaction: null, error: null, tokenUsage: null, isStreaming: false, currentStreamId: null, pendingConfirm: null, pendingAskUser: null, tasks: [], pendingPlan: null, subAgents: [] }),

  sendMessage: async (content, sessionId, projectPath, contextModules, devSessionId, attachments, knowledgeFeatureId) => {
    const state = get()
    if (state.isStreaming) return

    // Ensure stream listeners are registered before sending
    await setupStreamListeners()

    // Clear previous error
    set({ error: null })

    // Build display attachments for the user message bubble
    const displayAttachments: AttachmentDisplay[] | undefined =
      attachments && attachments.length > 0
        ? attachments.map((a) => ({
            name: a.name,
            mimeType: a.mime_type,
            thumbnailUrl: a.mime_type.startsWith('image/')
              ? `data:${a.mime_type};base64,${a.data_base64}`
              : null,
          }))
        : undefined

    // Add user message (optimistic)
    get().addMessage('user', content, displayAttachments)

    // Mark session as having messages immediately (so "New Chat" button works during streaming)
    if (sessionId) {
      useChatSessionStore.getState().setSessionHasMessages(sessionId, true)
    }

    // Generate stream ID
    const streamId = crypto.randomUUID()

    // Set streaming state and add empty assistant message
    set({ isStreaming: true, currentStreamId: streamId })
    get().addMessage('assistant', '')

    // Start inactivity monitor
    _lastStreamActivity = Date.now()
    if (_inactivityIntervalId) clearInterval(_inactivityIntervalId)
    _inactivityIntervalId = setInterval(() => {
      const s = useChatStore.getState()
      if (!s.isStreaming) { clearInterval(_inactivityIntervalId!); _inactivityIntervalId = null; return }
      if (_lastStreamActivity > 0 && Date.now() - _lastStreamActivity > INACTIVITY_TIMEOUT_MS) {
        clearInterval(_inactivityIntervalId!); _inactivityIntervalId = null
        s.stopStreaming()
        useChatStore.setState({
          error: { message: 'No response from AI for 3 minutes. Please try again.', code: 'INACTIVITY_TIMEOUT' },
        })
      }
    }, 30_000)

    // Build messages array for backend:
    // - Exclude the empty assistant message we just added (last message)
    // - Exclude system messages (system prompt is built by the backend)
    // - Exclude empty-content messages (from previously failed/lost streams)
    const chatMessages: ChatMessageInput[] = get()
      .messages.filter((m) => m.id !== get().messages[get().messages.length - 1]?.id)
      .filter((m) => m.role !== 'system')
      .filter((m) => m.content.trim() !== '')
      .map((m) => ({ role: m.role, content: m.content }))

    try {
      await tauriApi.sendChatMessage({
        messages: chatMessages,
        stream_id: streamId,
        session_id: sessionId ?? undefined,
        project_path: projectPath ?? undefined,
        context_modules: contextModules,
        dev_session_id: devSessionId ?? undefined,
        knowledge_feature_id: knowledgeFeatureId ?? undefined,
        attachments: attachments && attachments.length > 0 ? attachments : undefined,
      })
    } catch (err) {
      const message = err instanceof Error ? err.message : i18n.t('chat:input.sendFailed')
      const code = err instanceof VenoreError ? err.code : 'UNKNOWN_ERROR'
      set({ isStreaming: false, currentStreamId: null, error: { message, code } })
      // Remove the empty assistant message
      set((state) => ({
        messages: state.messages.filter((m) => m.id !== state.messages[state.messages.length - 1]?.id || m.content !== ''),
      }))
    }
  },

  stopStreaming: async () => {
    const { currentStreamId } = get()
    if (!currentStreamId) return

    try {
      await tauriApi.stopChatStream(currentStreamId)
    } catch (err) {
      console.error('Failed to stop stream:', err)
    }

    set((state) => {
      const msgs = [...state.messages]
      const last = msgs[msgs.length - 1]
      if (last) {
        msgs[msgs.length - 1] = { ...last, isStreaming: false }
      }
      return { isStreaming: false, currentStreamId: null, messages: msgs,
               pendingConfirm: null, pendingAskUser: null, pendingPlan: null }
    })
  },

  approveToolCall: async (toolCallId, approved, allowSession) => {
    const { pendingConfirm } = get()
    // Use the dev session id when present, otherwise fall back to the chat
    // session id. Always-allow needs a stable approval key — without a dev
    // session (e.g. Knowledge mode), passing null meant the backend stored
    // nothing, so subsequent calls kept asking. The backend looks up under
    // the same priority chain (dev_session_id → chat_session_id → stream_id).
    const sessionStore = useChatSessionStore.getState()
    const approvalKey = sessionStore.activeDevSessionId ?? sessionStore.activeSessionId
    set({ pendingConfirm: null })
    if (!approved) {
      set((state) => ({
        messages: updateToolCallById(state.messages, toolCallId, { status: 'denied' }),
      }))
    }
    try {
      await tauriApi.approveToolCall(
        toolCallId,
        approved,
        allowSession ?? false,
        approvalKey ?? undefined,
        pendingConfirm?.tool_name,
      )
    } catch (err) {
      console.error('Failed to approve tool call:', err)
    }
  },

  respondToAskUser: async (toolCallId, response) => {
    set({ pendingAskUser: null })
    try {
      await tauriApi.respondToAgent(toolCallId, response)
    } catch (err) {
      console.error('Failed to respond to agent:', err)
    }
  },

  approvePlan: async (toolCallId, approved) => {
    set({ pendingPlan: null })
    try {
      await tauriApi.approvePlan(toolCallId, approved)
    } catch (err) {
      console.error('Failed to approve plan:', err)
    }
  },

  setError: (message, code = 'UNKNOWN_ERROR') => set({ error: { message, code } }),
  setProviderInfo: (info) => set({ providerInfo: info }),
  setTokenUsage: (usage) => set({ tokenUsage: usage }),

  loadMessages: async (sessionId: string) => {
    try {
      const [records, snapshots] = await Promise.all([
        tauriApi.getChatMessages(sessionId),
        tauriApi.getChatSnapshots(sessionId).catch(() => [] as SnapshotDto[]),
      ])
      // Discard if the user switched to a different session during the async load
      if (useChatSessionStore.getState().activeSessionId !== sessionId) return
      const messages: ChatMessage[] = records.map((r: ChatMessageDto) => {
        // Restore attachment metadata from DB (thumbnailUrl is null — images show as file chips)
        let attachments: AttachmentDisplay[] | undefined
        if (r.attachments_json) {
          try {
            const parsed = JSON.parse(r.attachments_json) as Array<{ name: string; mimeType: string }>
            attachments = parsed.map((a) => ({ name: a.name, mimeType: a.mimeType, thumbnailUrl: null }))
          } catch { /* ignore malformed JSON */ }
        }
        return {
          id: r.id,
          role: r.role as MessageRole,
          content: r.content,
          timestamp: new Date(r.created_at).getTime(),
          isStreaming: false,
          ...(attachments && attachments.length > 0 ? { attachments } : {}),
        }
      })
      set({ messages, snapshots, error: null })
    } catch (err) {
      console.error('Failed to load messages:', err)
      // Clear stale messages on error instead of leaving old session's messages
      set({ messages: [], snapshots: [] })
    }
  },

  revertToSnapshot: async (devSessionId, commitHash, messageId?) => {
    try {
      await tauriApi.revertToSnapshot(devSessionId, commitHash, messageId)
      // Remove messages after the revert point from local state (only when messageId is known)
      if (messageId) {
        set((state) => {
          const idx = state.messages.findIndex((m) => m.id === messageId)
          if (idx >= 0) {
            return { messages: state.messages.slice(0, idx + 1) }
          }
          return state
        })
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to revert'
      set({ error: { message, code: 'REVERT_FAILED' } })
    }
  },

  snapshotForPopout: () => {
    const { messages, isStreaming, currentStreamId, error, providerInfo, tokenUsage,
      pendingConfirm, pendingAskUser, tasks, pendingPlan, subAgents, snapshots, lastCompaction } = get()
    return { messages, isStreaming, currentStreamId, error, providerInfo, tokenUsage,
      pendingConfirm, pendingAskUser, tasks, pendingPlan, subAgents, snapshots, lastCompaction }
  },

}))

/**
 * Check if a session has an active backend stream and reconnect the UI.
 * Called after pop-out transfers to restore streaming state that
 * localStorage snapshots intentionally clear (oneshot channels can't transfer).
 */
export async function reconnectToActiveStream(sessionId: string): Promise<void> {
  try {
    const streamId = await tauriApi.getSessionStreamStatus(sessionId)
    if (streamId) {
      useChatStore.setState({ isStreaming: true, currentStreamId: streamId })
      const msgs = useChatStore.getState().messages
      const last = msgs[msgs.length - 1]
      if (!last || last.role !== 'assistant') {
        useChatStore.getState().addMessage('assistant', '')
      }
    }
  } catch (e) {
    console.error('[reconnectToActiveStream] Failed:', e)
  }
}

// Register listeners on module load so ALL windows (main + pop-out) receive events
setupStreamListeners()
