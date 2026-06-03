// =============================================================================
// SessionDetailView — Container for session detail canvas tab
// =============================================================================
// Loads session data, diff files, and activity in parallel.
// Renders header with stats + SessionInnerTabs.
// Listens for session:file-changed events for real-time updates.

import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, GitBranch, CheckCircle, XCircle, Clock } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import { tauriApi } from '@/lib/tauri'
import type {
  SessionDto, SessionDiffFileDto,
  SessionFileChangedPayload, SessionActivityDto,
  ChatToolCallPayload, ChatToolResultPayload,
} from '@/lib/tauri'
import { SessionInnerTabs } from './session-detail/SessionInnerTabs'

interface SessionDetailViewProps {
  sessionId: string
  projectPath: string
  projectId?: string
}

const STATUS_STYLES: Record<string, { icon: typeof Clock; color: string; labelKey: string }> = {
  active: { icon: Clock, color: 'text-blue-400', labelKey: 'detail.active' },
  completed: { icon: CheckCircle, color: 'text-green-400', labelKey: 'detail.completed' },
  abandoned: { icon: XCircle, color: 'text-foreground-muted', labelKey: 'detail.abandoned' },
}

export function SessionDetailView({ sessionId, projectPath, projectId }: SessionDetailViewProps) {
  const { t } = useTranslation('sessions')
  const [session, setSession] = useState<SessionDto | null>(null)
  const [files, setFiles] = useState<SessionDiffFileDto[]>([])
  const [activity, setActivity] = useState<SessionActivityDto | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actionLoading, setActionLoading] = useState(false)
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const selectedFileRef = useRef<string | null>(null)
  const chatSessionIdRef = useRef<string | null>(null)

  // Keep ref in sync for use inside refreshData without dependency
  selectedFileRef.current = selectedFile

  // Derive stats from files[] for real-time accuracy
  const totalAdditions = useMemo(() => files.reduce((sum, f) => sum + f.additions, 0), [files])
  const totalDeletions = useMemo(() => files.reduce((sum, f) => sum + f.deletions, 0), [files])

  const refreshData = useCallback(async (showLoading = false) => {
    if (showLoading) setLoading(true)
    try {
      const [sessionData, diffFiles] = await Promise.all([
        tauriApi.getSession(sessionId, projectPath),
        tauriApi.sessionDiffFiles({ session_id: sessionId, project_path: projectPath }),
      ])
      setSession(sessionData)
      // Merge: backend is authoritative for files it reports,
      // but preserve event-driven files the backend doesn't see
      setFiles(prev => {
        const merged = new Map<string, SessionDiffFileDto>()
        // Keep event-driven files first
        for (const f of prev) {
          merged.set(f.filename, f)
        }
        // Backend data overwrites (it has latest git diff)
        for (const f of diffFiles) {
          merged.set(f.filename, f)
        }
        return Array.from(merged.values())
      })
      setError(null)

      // Auto-select first file if none selected
      if (!selectedFileRef.current && diffFiles.length > 0) {
        setSelectedFile(diffFiles[0].filename)
      }

      // Load activity if we have a chat session linked to this dev session
      try {
        const chatSession = await tauriApi.getOrCreateDevSessionChat({
          dev_session_id: sessionId,
          session_name: sessionData.name,
          project_id: projectId,
        })
        chatSessionIdRef.current = chatSession.id
        const activityData = await tauriApi.getSessionActivity(chatSession.id)
        setActivity(activityData)
      } catch {
        // Chat session may not exist yet — that's fine
      }
    } catch (err) {
      if (showLoading) setError(err instanceof Error ? err.message : t('detail.failedToLoad'))
    } finally {
      if (showLoading) setLoading(false)
    }
  }, [sessionId, projectPath, projectId, t])

  // Initial load
  useEffect(() => {
    refreshData(true)
  }, [refreshData])

  // Event-driven: listen for session:file-changed events
  useEffect(() => {
    const unlisten = listen<SessionFileChangedPayload>('session:file-changed', (event) => {
      const incoming = event.payload
      if (incoming.dev_session_id !== sessionId) return

      setFiles(prev => {
        const idx = prev.findIndex(f => f.filename === incoming.filename)
        const newFile: SessionDiffFileDto = {
          filename: incoming.filename,
          status: incoming.status,
          additions: incoming.additions,
          deletions: incoming.deletions,
          patch: incoming.patch,
        }
        return idx >= 0
          ? prev.map((f, i) => i === idx ? newFile : f)
          : [...prev, newFile]
      })

      // Auto-navigate to the changed file
      setSelectedFile(incoming.filename)
    })

    return () => { unlisten.then(fn => fn()) }
  }, [sessionId])

  // Event-driven: listen for chat-tool-call events for real-time activity updates
  useEffect(() => {
    const unlistenCall = listen<ChatToolCallPayload>('chat-tool-call', (event) => {
      const payload = event.payload
      setActivity(prev => {
        if (!prev) return prev
        return {
          ...prev,
          tool_calls: [...prev.tool_calls, {
            id: payload.tool_call_id,
            tool_name: payload.tool_name,
            arguments: payload.arguments as Record<string, unknown>,
            success: null,
            output: null,
            commit_hash: null,
            created_at: new Date().toISOString(),
          }],
        }
      })
    })

    const unlistenResult = listen<ChatToolResultPayload>('chat-tool-result', (event) => {
      const payload = event.payload
      setActivity(prev => {
        if (!prev) return prev
        return {
          ...prev,
          tool_calls: prev.tool_calls.map(tc =>
            tc.id === payload.tool_call_id
              ? { ...tc, success: payload.success, output: payload.output.slice(0, 500) }
              : tc
          ),
        }
      })
    })

    return () => {
      unlistenCall.then(fn => fn())
      unlistenResult.then(fn => fn())
    }
  }, [])

  // Listen for full refresh after revert (session:files-refreshed)
  useEffect(() => {
    const unlisten = listen<{ dev_session_id: string }>('session:files-refreshed', (event) => {
      if (event.payload.dev_session_id !== sessionId) return
      refreshData(false)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [sessionId, refreshData])

  // Safety-net polling: refresh every 15s for indirect changes (terminal commands, etc.)
  useEffect(() => {
    const interval = setInterval(() => refreshData(false), 15000)
    return () => clearInterval(interval)
  }, [refreshData])

  const handleComplete = async () => {
    if (!session) return
    setActionLoading(true)
    try {
      const updated = await tauriApi.completeSession(sessionId, projectPath)
      setSession(updated)
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t('detail.failedToComplete'))
    } finally {
      setActionLoading(false)
    }
  }

  const handleAbandon = async () => {
    if (!session) return
    if (!window.confirm(t('detail.confirmAbandon', { name: session.name }))) return
    setActionLoading(true)
    try {
      await tauriApi.abandonSession(sessionId, projectPath)
      setSession({ ...session, status: 'abandoned' })
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t('detail.failedToAbandon'))
    } finally {
      setActionLoading(false)
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  if (error || !session) {
    return (
      <div className="flex-1 flex items-center justify-center text-xs text-red-400">
        {error ?? t('detail.sessionNotFound')}
      </div>
    )
  }

  const status = STATUS_STYLES[session.status] ?? STATUS_STYLES.active
  const StatusIcon = status.icon

  return (
    <div className="flex-1 flex flex-col min-h-0 bg-background-secondary">
      {/* Header */}
      <div className="shrink-0 border-b border-border px-4 py-2 flex items-center gap-3">
        <GitBranch className="w-3.5 h-3.5 text-foreground-muted shrink-0" />
        <div className="flex-1 min-w-0 flex items-center gap-3">
          <h2 className="text-xs font-medium text-foreground truncate">{session.name}</h2>
          <span className="text-[10px] text-foreground-muted/50 font-mono truncate">{session.session_branch} &larr; {session.base_branch}</span>
          <span className="flex items-center gap-2 text-[10px] text-foreground-muted/50">
            <span>{t('detail.files', { count: files.length })}</span>
            {totalAdditions > 0 && <span className="text-green-400/70">+{totalAdditions}</span>}
            {totalDeletions > 0 && <span className="text-red-400/70">-{totalDeletions}</span>}
          </span>
        </div>
        {session.status === 'active' ? (
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              onClick={handleComplete}
              disabled={actionLoading}
              className="flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium bg-green-500/15 text-green-400 hover:bg-green-500/25 rounded transition-colors disabled:opacity-40"
            >
              <CheckCircle className="w-3 h-3" />
              {t('detail.complete')}
            </button>
            <button
              onClick={handleAbandon}
              disabled={actionLoading}
              className="flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium text-foreground-muted/50 hover:text-foreground-muted hover:bg-background-tertiary rounded transition-colors disabled:opacity-40"
            >
              <XCircle className="w-3 h-3" />
              {t('detail.abandon')}
            </button>
          </div>
        ) : (
          <span className={cn('flex items-center gap-1 text-[10px] font-medium shrink-0', status.color)}>
            <StatusIcon className="w-3 h-3" />
            {t(status.labelKey)}
          </span>
        )}
      </div>

      {/* Tabs */}
      <SessionInnerTabs
        files={files}
        session={session}
        activity={activity}
        selectedFile={selectedFile}
        onSelectFile={setSelectedFile}
        onRevert={async (commitHash) => {
          await tauriApi.revertToSnapshot(sessionId, commitHash)
        }}
      />
    </div>
  )
}
