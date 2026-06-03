// =============================================================================
// SessionActivityTab — Token Usage + Checkpoints + Tool History
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Zap, GitCommit, Check, X, Loader2, ChevronRight, AlertTriangle } from 'lucide-react'
import { formatTimeAgo } from '@/lib/time'
import type { SessionActivityDto, SnapshotDto } from '@/lib/tauri'

interface SessionActivityTabProps {
  activity: SessionActivityDto
  onRevert?: (commitHash: string, toolCallId: string) => void
}

export function SessionActivityTab({ activity, onRevert }: SessionActivityTabProps) {
  return (
    <div className="p-4 flex flex-col gap-4 h-full min-h-0 overflow-hidden">
      {/* Token Usage — full width, compact */}
      <TokenUsageSection activity={activity} />

      {/* Checkpoints + Tool History — 50/50 side by side, fill remaining space */}
      <div className="flex-1 grid grid-cols-2 gap-4 min-h-0 overflow-hidden">
        <CheckpointsSection activity={activity} onRevert={onRevert} />
        <ToolHistorySection activity={activity} />
      </div>
    </div>
  )
}

// ── Token Usage ──────────────────────────────────────────────────────

function TokenUsageSection({ activity }: { activity: SessionActivityDto }) {
  const { t } = useTranslation('sessions')
  const { token_summary } = activity

  return (
    <div className="shrink-0">
      <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2">
        {t('activity.tokenUsage')}
      </h3>
      <div className="grid grid-cols-3 gap-3">
        <div className="bg-background-tertiary rounded px-3 py-2">
          <div className="flex items-center gap-1.5 text-[10px] text-foreground-muted/60 mb-0.5">
            <Zap className="w-3 h-3" />
            {t('activity.promptTokens')}
          </div>
          <div className="text-sm font-medium text-foreground">
            {token_summary.total_prompt_tokens.toLocaleString()}
          </div>
        </div>
        <div className="bg-background-tertiary rounded px-3 py-2">
          <div className="flex items-center gap-1.5 text-[10px] text-foreground-muted/60 mb-0.5">
            <Zap className="w-3 h-3" />
            {t('activity.completionTokens')}
          </div>
          <div className="text-sm font-medium text-foreground">
            {token_summary.total_completion_tokens.toLocaleString()}
          </div>
        </div>
        <div className="bg-background-tertiary rounded px-3 py-2">
          <div className="text-[10px] text-foreground-muted/60 mb-0.5">
            {t('activity.messages')}
          </div>
          <div className="text-sm font-medium text-foreground">
            {token_summary.message_count}
          </div>
        </div>
      </div>
    </div>
  )
}

// ── Checkpoints ──────────────────────────────────────────────────────

/** Extract just the filename from a full path */
function extractFilename(filePath: string): string {
  const parts = filePath.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || filePath
}

function CheckpointsSection({
  activity,
  onRevert,
}: {
  activity: SessionActivityDto
  onRevert?: (commitHash: string, toolCallId: string) => void
}) {
  const { t } = useTranslation('sessions')
  const { snapshots } = activity
  const [confirmingId, setConfirmingId] = useState<string | null>(null)
  const [reverting, setReverting] = useState(false)

  const handleConfirm = async (snap: SnapshotDto) => {
    if (!onRevert) return
    setReverting(true)
    try {
      await onRevert(snap.commit_hash, snap.tool_call_id)
    } finally {
      setReverting(false)
      setConfirmingId(null)
    }
  }

  return (
    <div className="flex flex-col min-h-0 overflow-hidden">
      <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2 shrink-0">
        {t('activity.checkpoints')}
      </h3>
      {snapshots.length === 0 ? (
        <p className="text-[11px] text-foreground-muted/40">{t('activity.noCheckpoints')}</p>
      ) : (
        <div className="space-y-1 overflow-y-auto flex-1 min-h-0">
          {snapshots.map((snap) => (
            <div key={snap.tool_call_id}>
              <div className="flex items-center gap-2 px-2 py-1.5 rounded bg-background-tertiary/50 hover:bg-background-tertiary transition-colors">
                <GitCommit className="w-3.5 h-3.5 text-foreground-muted/50 shrink-0" />
                {snap.tool_name && (
                  <span className="text-[10px] text-foreground-muted/40 shrink-0">
                    {snap.tool_name}
                  </span>
                )}
                {snap.file_path && (
                  <span className="text-[11px] text-foreground truncate min-w-0 font-medium">
                    {extractFilename(snap.file_path)}
                  </span>
                )}
                <code className="text-[10px] font-mono text-foreground-muted/40 shrink-0">
                  {snap.commit_hash.slice(0, 7)}
                </code>
                <span className="text-[10px] text-foreground-muted/40 shrink-0">
                  {formatTimeAgo(snap.created_at)}
                </span>
                <div className="flex-1" />
                {onRevert && (
                  <button
                    onClick={() => setConfirmingId(snap.tool_call_id)}
                    disabled={reverting}
                    className="text-[10px] px-1.5 py-0.5 text-foreground-muted/50 hover:text-foreground-muted hover:bg-background-secondary rounded transition-colors shrink-0 disabled:opacity-40"
                  >
                    {t('activity.revert')}
                  </button>
                )}
              </div>
              {/* Inline confirmation */}
              {confirmingId === snap.tool_call_id && (
                <div className="mx-1 mt-1 mb-1 border border-amber-500/30 bg-amber-500/5 rounded px-3 py-2">
                  <div className="flex items-start gap-2">
                    <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0 mt-0.5" />
                    <div className="flex-1 min-w-0">
                      <p className="text-[11px] text-amber-200/80">
                        {t('activity.revertConfirmMessage')}
                      </p>
                      <div className="flex items-center gap-2 mt-2">
                        <button
                          onClick={() => handleConfirm(snap)}
                          disabled={reverting}
                          className="flex items-center gap-1 text-[10px] px-2 py-0.5 bg-amber-500/20 text-amber-300 hover:bg-amber-500/30 rounded transition-colors disabled:opacity-40"
                        >
                          {reverting && <Loader2 className="w-3 h-3 animate-spin" />}
                          {t('activity.revertConfirm')}
                        </button>
                        <button
                          onClick={() => setConfirmingId(null)}
                          disabled={reverting}
                          className="text-[10px] px-2 py-0.5 text-foreground-muted/50 hover:text-foreground-muted hover:bg-background-tertiary rounded transition-colors disabled:opacity-40"
                        >
                          {t('activity.revertCancel')}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ── Tool History ─────────────────────────────────────────────────────

function ToolHistorySection({ activity }: { activity: SessionActivityDto }) {
  const { t } = useTranslation('sessions')
  const { tool_calls } = activity

  return (
    <div className="flex flex-col min-h-0 overflow-hidden">
      <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2 shrink-0">
        {t('activity.toolHistory')}
      </h3>
      {tool_calls.length === 0 ? (
        <p className="text-[11px] text-foreground-muted/40">{t('activity.noToolCalls')}</p>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto">
          <div className="space-y-0.5">
            {tool_calls.map((tc) => (
              <ToolCallItem key={tc.id} tc={tc} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

function ToolCallItem({ tc }: { tc: SessionActivityDto['tool_calls'][number] }) {
  const { t } = useTranslation('sessions')
  const [expanded, setExpanded] = useState(false)

  const StatusIcon =
    tc.success === true ? Check :
    tc.success === false ? X :
    Loader2

  const statusColor =
    tc.success === true ? 'text-green-400' :
    tc.success === false ? 'text-red-400' :
    'text-blue-400 animate-spin'

  return (
    <div className="min-w-0">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded hover:bg-background-tertiary/50 transition-colors text-left min-w-0"
      >
        <ChevronRight className={`w-3 h-3 text-foreground-muted/40 shrink-0 transition-transform ${expanded ? 'rotate-90' : ''}`} />
        <StatusIcon className={`w-3 h-3 shrink-0 ${statusColor}`} />
        <span className="text-[11px] font-mono text-foreground truncate min-w-0">{tc.tool_name}</span>
        {tc.commit_hash && (
          <span className="text-[9px] px-1 py-0.5 rounded bg-blue-500/15 text-blue-400 shrink-0">
            {t('activity.snapshot')}
          </span>
        )}
        <span className="text-[10px] text-foreground-muted/30 shrink-0 ml-auto">
          {formatTimeAgo(tc.created_at)}
        </span>
      </button>
      {expanded && (
        <div className="ml-6 mr-1 mb-2 space-y-2 min-w-0 overflow-hidden">
          <div className="min-w-0">
            <div className="text-[9px] text-foreground-muted/50 uppercase mb-0.5">{t('activity.arguments')}</div>
            <pre className="text-[10px] font-mono text-foreground-muted bg-background-tertiary rounded p-2 overflow-auto max-h-32 whitespace-pre-wrap break-all">
              {JSON.stringify(tc.arguments, null, 2)}
            </pre>
          </div>
          {tc.output && (
            <div className="min-w-0">
              <div className="text-[9px] text-foreground-muted/50 uppercase mb-0.5">{t('activity.output')}</div>
              <pre className="text-[10px] font-mono text-foreground-muted bg-background-tertiary rounded p-2 overflow-auto max-h-32 whitespace-pre-wrap break-all">
                {tc.output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
