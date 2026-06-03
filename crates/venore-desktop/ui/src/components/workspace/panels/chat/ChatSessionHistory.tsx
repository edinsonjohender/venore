// =============================================================================
// ChatSessionHistory - Full-height session list replacing panel content
// =============================================================================

import { useEffect, useState, useRef } from 'react'
import { Trash2, Pencil, Check, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useChatSessionStore, DRAFT_SESSION_ID } from '@/stores/chatSessionStore'
import { Button } from '@/components/ui/button'
import { formatTimeAgo } from '@/lib/time'

interface ChatSessionHistoryProps {
  projectId?: string
}

export function ChatSessionHistory({ projectId }: ChatSessionHistoryProps) {
  const { t } = useTranslation('chat')
  const sessions = useChatSessionStore((s) => s.sessions)
  const activeSessionId = useChatSessionStore((s) => s.activeSessionId)
  const isLoading = useChatSessionStore((s) => s.isLoading)
  const loadSessions = useChatSessionStore((s) => s.loadSessions)
  const switchSession = useChatSessionStore((s) => s.switchSession)
  const deleteSession = useChatSessionStore((s) => s.deleteSession)
  const renameSession = useChatSessionStore((s) => s.renameSession)
  const openChatTab = useChatSessionStore((s) => s.openChatTab)

  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  // Load sessions on mount
  useEffect(() => {
    loadSessions(projectId)
  }, [loadSessions, projectId])

  // Focus input when editing
  useEffect(() => {
    if (editingId) {
      inputRef.current?.focus()
      inputRef.current?.select()
    }
  }, [editingId])

  const startRename = (id: string, currentName: string) => {
    setEditingId(id)
    setEditName(currentName)
  }

  const confirmRename = () => {
    if (editingId && editName.trim()) {
      renameSession(editingId, editName.trim())
    }
    setEditingId(null)
  }

  const cancelRename = () => {
    setEditingId(null)
  }

  const handleRestoreSession = async (targetSession: typeof sessions[0]) => {
    // Open the restored session as a tab and switch to it. The draft tab (if
    // any) stays put as the "new chat" affordance — it's in-memory only, so
    // there's nothing to clean up (unlike the old empty-session path).
    openChatTab(targetSession.id)
    await switchSession(targetSession.id)
  }

  if (isLoading) {
    return <div className="px-3 py-4 text-center text-xs text-foreground-muted">{t('sessionHistory.loading')}</div>
  }

  // The in-memory draft is never part of history (it has no DB row yet).
  const persistedSessions = sessions.filter((s) => s.id !== DRAFT_SESSION_ID)

  if (persistedSessions.length === 0) {
    return <div className="px-3 py-4 text-center text-xs text-foreground-muted">{t('sessionHistory.noSessions')}</div>
  }

  return (
    <div className="py-1 overflow-y-auto">
      {persistedSessions.map((session) => (
        <div
          key={session.id}
          className={cn(
            'group flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-background-tertiary transition-colors',
            session.id === activeSessionId && 'bg-background-tertiary',
          )}
          onClick={() => editingId !== session.id && handleRestoreSession(session)}
        >
          {editingId === session.id ? (
            <div className="flex items-center gap-1 flex-1 min-w-0">
              <input
                ref={inputRef}
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') confirmRename()
                  if (e.key === 'Escape') cancelRename()
                }}
                className="flex-1 min-w-0 text-xs bg-background border border-border rounded px-1.5 py-0.5 outline-none focus:border-brand"
                onClick={(e) => e.stopPropagation()}
              />
              <Button variant="ghost" size="icon" className="h-5 w-5 shrink-0" onClick={(e) => { e.stopPropagation(); confirmRename() }}>
                <Check className="w-3 h-3" />
              </Button>
              <Button variant="ghost" size="icon" className="h-5 w-5 shrink-0" onClick={(e) => { e.stopPropagation(); cancelRename() }}>
                <X className="w-3 h-3" />
              </Button>
            </div>
          ) : (
            <>
              <div className="flex-1 min-w-0">
                <div className="text-xs text-foreground truncate">{session.name}</div>
                <div className="text-[10px] text-foreground-subtle">{formatTimeAgo(session.updated_at)}</div>
              </div>
              <div className="hidden group-hover:flex items-center gap-0.5 shrink-0">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5"
                  onClick={(e) => { e.stopPropagation(); startRename(session.id, session.name) }}
                  title={t('sessionHistory.renameSession')}
                >
                  <Pencil className="w-3 h-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5 text-red-400 hover:text-red-300"
                  onClick={(e) => { e.stopPropagation(); deleteSession(session.id) }}
                  title={t('sessionHistory.deleteSession')}
                >
                  <Trash2 className="w-3 h-3" />
                </Button>
              </div>
            </>
          )}
        </div>
      ))}
    </div>
  )
}
