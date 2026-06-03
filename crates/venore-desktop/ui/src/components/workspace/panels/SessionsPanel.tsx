// =============================================================================
// SessionsPanel — List + create sessions (branch-per-session workflow)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import {
  GitBranch, Plus, Loader2, CheckCircle, XCircle, Clock,
  ChevronDown, ChevronRight, AlertTriangle,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PanelContentProps } from './registry'
import { tauriApi } from '@/lib/tauri'
import type { SessionDto } from '@/lib/tauri'
import { useChatSessionStore } from '@/stores/chatSessionStore'

// Slugify name into a branch name
function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
}

const STATUS_ICONS: Record<string, typeof CheckCircle> = {
  active: Clock,
  completed: CheckCircle,
  abandoned: XCircle,
}

const STATUS_COLORS: Record<string, string> = {
  active: 'bg-blue-500/15 text-blue-400',
  completed: 'bg-green-500/15 text-green-400',
  abandoned: 'bg-foreground-muted/10 text-foreground-muted',
}

export function SessionsPanel({ projectPath, projectId }: PanelContentProps) {
  const { t } = useTranslation('sessions')
  const [sessions, setSessions] = useState<SessionDto[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const openDevSession = useChatSessionStore((s) => s.openDevSession)

  const handleOpenSession = (id: string, name: string, worktreePath?: string) => {
    openDevSession(id, name, projectId, worktreePath)
  }

  const fetchSessions = useCallback(async () => {
    if (!projectPath || !projectId) return
    setLoading(true)
    setError(null)
    try {
      const data = await tauriApi.listSessions(projectId, projectPath)
      setSessions(data)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : t('panel.failedToLoad')
      setError(msg)
    } finally {
      setLoading(false)
    }
  }, [projectPath, projectId, t])

  useEffect(() => {
    fetchSessions()
  }, [fetchSessions])

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 h-full">
        <AlertTriangle className="w-10 h-10 text-foreground-muted/20" />
        <span className="text-xs font-medium text-foreground-muted">
          {error.includes('Not a git repository') ? t('panel.gitRequired') : t('panel.error')}
        </span>
        <span className="text-[10px] text-foreground-muted/60 text-center leading-relaxed">
          {error.includes('Not a git repository')
            ? t('panel.gitRequiredDescription')
            : error}
        </span>
        <button
          onClick={fetchSessions}
          className="text-[10px] text-brand hover:text-brand/80 transition-colors"
        >
          {t('panel.retry')}
        </button>
      </div>
    )
  }

  const active = sessions.filter((s) => s.status === 'active')
  const completed = sessions.filter((s) => s.status === 'completed')
  const abandoned = sessions.filter((s) => s.status === 'abandoned')

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 h-8 border-b border-border shrink-0">
        <span className="text-[10px] text-foreground-muted">
          {t('panel.sessionCount', { count: sessions.length })}
        </span>
        <button
          onClick={() => setShowForm(!showForm)}
          className="flex items-center gap-1 text-[10px] text-foreground-muted hover:text-foreground transition-colors"
        >
          <Plus className="w-3 h-3" />
          {t('panel.new')}
        </button>
      </div>

      {/* Create form */}
      {showForm && (
        <CreateSessionForm
          projectPath={projectPath}
          projectId={projectId ?? ''}
          onCreated={(session) => {
            setShowForm(false)
            fetchSessions()
            handleOpenSession(session.id, session.name, session.worktree_path || undefined)
          }}
          onCancel={() => setShowForm(false)}
        />
      )}

      {/* Session list */}
      <div className="flex-1 overflow-y-auto py-1">
        {sessions.length === 0 && !showForm && (
          <div className="flex flex-col items-center justify-center gap-2 py-8 px-4">
            <GitBranch className="w-8 h-8 text-foreground-muted/20" />
            <span className="text-[11px] text-foreground-muted/60 text-center">
              {t('panel.noSessionsDescription')}
            </span>
          </div>
        )}

        <SessionGroup label={t('panel.active')} sessions={active} defaultOpen onSelect={(s) => handleOpenSession(s.id, s.name, s.worktree_path || undefined)} />
        <SessionGroup label={t('panel.completed')} sessions={completed} defaultOpen={false} onSelect={(s) => handleOpenSession(s.id, s.name, s.worktree_path || undefined)} />
        <SessionGroup label={t('panel.abandoned')} sessions={abandoned} defaultOpen={false} onSelect={(s) => handleOpenSession(s.id, s.name, s.worktree_path || undefined)} />
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Session Group
// -----------------------------------------------------------------------------

function SessionGroup({
  label,
  sessions,
  defaultOpen,
  onSelect,
}: {
  label: string
  sessions: SessionDto[]
  defaultOpen: boolean
  onSelect: (session: SessionDto) => void
}) {
  const [open, setOpen] = useState(defaultOpen)

  if (sessions.length === 0) return null

  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-1 px-3 py-1.5 text-[10px] font-medium text-foreground-muted hover:bg-background-tertiary/50 transition-colors"
      >
        {open ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
        {label}
        <span className="text-foreground-muted/50">({sessions.length})</span>
      </button>
      {open && sessions.map((session) => (
        <SessionItem key={session.id} session={session} onSelect={onSelect} />
      ))}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Session Item
// -----------------------------------------------------------------------------

function SessionItem({
  session,
  onSelect,
}: {
  session: SessionDto
  onSelect: (session: SessionDto) => void
}) {
  const { t } = useTranslation('sessions')
  const StatusIcon = STATUS_ICONS[session.status] ?? Clock
  const colorClass = STATUS_COLORS[session.status] ?? ''

  return (
    <button
      onClick={() => onSelect(session)}
      className="w-full flex items-center gap-2 px-3 py-1.5 text-[11px] text-foreground-muted hover:bg-background-tertiary/50 transition-colors"
    >
      <GitBranch className="w-3.5 h-3.5 shrink-0 opacity-50" />
      <div className="flex-1 min-w-0 text-left">
        <div className="truncate font-medium text-foreground">{session.name}</div>
        <div className="truncate text-[9px] text-foreground-muted/60">{session.session_branch}</div>
      </div>
      <span className={cn('shrink-0 text-[9px] px-1.5 py-0.5 rounded font-medium leading-none flex items-center gap-1', colorClass)}>
        <StatusIcon className="w-2.5 h-2.5" />
        {t(`panel.${session.status}`)}
      </span>
      {(session.additions > 0 || session.deletions > 0) && (
        <span className="shrink-0 flex items-center gap-1 text-[9px]">
          {session.additions > 0 && <span className="text-green-400">+{session.additions}</span>}
          {session.deletions > 0 && <span className="text-red-400">-{session.deletions}</span>}
        </span>
      )}
    </button>
  )
}

// -----------------------------------------------------------------------------
// Create Session Form
// -----------------------------------------------------------------------------

function CreateSessionForm({
  projectPath,
  projectId,
  onCreated,
  onCancel,
}: {
  projectPath: string
  projectId: string
  onCreated: (session: SessionDto) => void
  onCancel: () => void
}) {
  const { t } = useTranslation('sessions')
  const [name, setName] = useState('')
  const [objective, setObjective] = useState('')
  const [baseBranch, setBaseBranch] = useState('')
  const [branchName, setBranchName] = useState('')
  const [branches, setBranches] = useState<string[]>([])
  const [isLocalGit, setIsLocalGit] = useState(true)
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [branchManuallyEdited, setBranchManuallyEdited] = useState(false)

  // Load branches on mount
  useEffect(() => {
    tauriApi.listGitBranches({ project_path: projectPath }).then((res) => {
      setBranches(res.branches)
      setIsLocalGit(res.is_local_git)
      const defaultBranch = res.branches.find((b) => b === 'main') ?? res.branches.find((b) => b === 'master') ?? res.branches[0] ?? ''
      setBaseBranch(defaultBranch)
    }).catch((err) => {
      console.error('[Sessions] listGitBranches failed:', err)
      setError(err instanceof Error ? err.message : t('form.failedToLoadBranches'))
    })
  }, [projectPath, t])

  // Auto-suggest branch name from session name
  useEffect(() => {
    if (!branchManuallyEdited && name) {
      setBranchName(`session/${slugify(name)}`)
    }
  }, [name, branchManuallyEdited])

  const handleCreate = async () => {
    if (!name.trim() || !baseBranch || !branchName.trim()) return
    setCreating(true)
    setError(null)
    try {
      const session = await tauriApi.createSession({
        name: name.trim(),
        objective: objective.trim(),
        project_path: projectPath,
        project_id: projectId,
        base_branch: baseBranch,
        branch_name: branchName.trim(),
      })
      onCreated(session)
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t('form.failedToCreate'))
    } finally {
      setCreating(false)
    }
  }

  const canCreate = isLocalGit && !!name.trim() && !!branchName.trim()

  return (
    <div className="border-b border-border px-3 py-2 space-y-2">
      {/* Not a local git repo warning */}
      {!isLocalGit && (
        <div className="flex items-start gap-2 text-[10px] text-amber-400 bg-amber-500/10 px-2 py-1.5 rounded">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>
            {t('form.notLocalGit')}{' '}
            {t('form.cloneToEnable')}
          </span>
        </div>
      )}

      {/* Name */}
      <input
        type="text"
        placeholder={t('form.sessionName')}
        value={name}
        onChange={(e) => setName(e.target.value)}
        className="w-full px-2 py-1 text-[11px] bg-background-secondary border border-border rounded text-foreground placeholder:text-foreground-muted/40 outline-none focus:border-brand/50"
        autoFocus
      />

      {/* Objective */}
      <textarea
        placeholder={t('form.objective')}
        value={objective}
        onChange={(e) => setObjective(e.target.value)}
        rows={2}
        className="w-full px-2 py-1 text-[11px] bg-background-secondary border border-border rounded text-foreground placeholder:text-foreground-muted/40 outline-none focus:border-brand/50 resize-none"
      />

      {/* Base branch */}
      <div>
        <label className="text-[9px] text-foreground-muted/60 uppercase tracking-wider">{t('form.baseBranch')}</label>
        <select
          value={baseBranch}
          onChange={(e) => setBaseBranch(e.target.value)}
          className="w-full px-2 py-1 text-[11px] bg-background-secondary border border-border rounded text-foreground outline-none focus:border-brand/50"
        >
          {branches.map((b) => (
            <option key={b} value={b}>{b}</option>
          ))}
        </select>
      </div>

      {/* Branch name */}
      <div>
        <label className="text-[9px] text-foreground-muted/60 uppercase tracking-wider">{t('form.branchName')}</label>
        <input
          type="text"
          value={branchName}
          onChange={(e) => {
            setBranchName(e.target.value)
            setBranchManuallyEdited(true)
          }}
          className="w-full px-2 py-1 text-[11px] bg-background-secondary border border-border rounded text-foreground font-mono outline-none focus:border-brand/50"
        />
      </div>

      {/* Error */}
      {error && (
        <div className="text-[10px] text-red-400 bg-red-500/10 px-2 py-1 rounded">
          {error}
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2 pt-1">
        <button
          onClick={handleCreate}
          disabled={creating || !canCreate}
          className="flex items-center gap-1 px-3 py-1 text-[10px] font-medium bg-brand/20 text-brand hover:bg-brand/30 rounded transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {creating ? <Loader2 className="w-3 h-3 animate-spin" /> : <GitBranch className="w-3 h-3" />}
          {t('form.create')}
        </button>
        <button
          onClick={onCancel}
          className="px-3 py-1 text-[10px] text-foreground-muted hover:text-foreground transition-colors"
        >
          {t('form.cancel')}
        </button>
      </div>
    </div>
  )
}
