// =============================================================================
// IssuesTab - Issue list with expandable items
// =============================================================================

import { useMemo, useState } from 'react'
import {
  CircleDot, CheckCircle2, Inbox, Loader2, AlertTriangle,
  ExternalLink, ChevronRight, MessageSquare, User, Eye,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { GitHubIssueDto, GitHubLabelDto } from '@/lib/tauri'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { timeAgo, openInBrowser } from './utils'

type StateFilter = 'open' | 'closed' | 'all'

interface IssuesTabProps {
  issues: GitHubIssueDto[]
  loading: boolean
  error: string | null
}

function LabelChip({ label }: { label: GitHubLabelDto }) {
  return (
    <span
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-medium leading-none"
      style={{
        backgroundColor: `#${label.color}18`,
        color: `#${label.color}`,
        border: `1px solid #${label.color}30`,
      }}
    >
      <span
        className="w-1.5 h-1.5 rounded-full"
        style={{ backgroundColor: `#${label.color}` }}
      />
      {label.name}
    </span>
  )
}

function IssueItem({ issue }: { issue: GitHubIssueDto }) {
  const [expanded, setExpanded] = useState(false)
  const openIssue = useCanvasTabStore((s) => s.openIssue)

  return (
    <div className="border-b border-border/50 last:border-b-0">
      {/* Collapsed row */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2.5 px-3 py-2.5 hover:bg-background-tertiary/50 transition-colors text-left group"
      >
        {/* State icon */}
        <div className="shrink-0">
          {issue.state === 'open' ? (
            <CircleDot className="w-4 h-4 text-green-500" />
          ) : (
            <CheckCircle2 className="w-4 h-4 text-purple-500" />
          )}
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          <span className="text-xs font-medium text-foreground truncate block">
            {issue.title}
          </span>
          <div className="flex items-center gap-1.5 mt-1 text-[10px] text-foreground-muted">
            <span className="font-mono">#{issue.number}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{issue.author}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{timeAgo(issue.updated_at)}</span>
          </div>
        </div>

        {/* Expand chevron */}
        <ChevronRight className={cn(
          'w-3.5 h-3.5 text-foreground-muted/30 transition-transform shrink-0',
          expanded && 'rotate-90',
        )} />
      </button>

      {/* Expanded detail */}
      {expanded && (
        <div className="px-3 pb-3 pt-0 ml-[26px] space-y-2.5">
          {/* Body preview */}
          {issue.body && (
            <p className="text-[11px] text-foreground-muted/80 leading-relaxed line-clamp-3">
              {issue.body}
            </p>
          )}

          {/* Stats */}
          <div className="flex items-center gap-3 text-[10px] text-foreground-muted">
            {issue.comments > 0 && (
              <span className="flex items-center gap-1">
                <MessageSquare className="w-3 h-3" />
                {issue.comments}
              </span>
            )}
            {issue.assignees.length > 0 && (
              <span className="flex items-center gap-1">
                <User className="w-3 h-3" />
                {issue.assignees.join(', ')}
              </span>
            )}
          </div>

          {/* Labels */}
          {issue.labels.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {issue.labels.map((l) => (
                <LabelChip key={l.name} label={l} />
              ))}
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={(e) => { e.stopPropagation(); openIssue(issue.number, issue.title) }}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] font-medium
                         bg-brand/15 hover:bg-brand/25 text-brand
                         transition-colors"
            >
              <Eye className="w-3 h-3" />
              Open
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); openInBrowser(issue.html_url) }}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] font-medium
                         bg-background-tertiary hover:bg-background-tertiary/80 text-foreground-muted
                         hover:text-foreground transition-colors"
            >
              <ExternalLink className="w-3 h-3" />
              GitHub
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

export function IssuesTab({ issues, loading, error }: IssuesTabProps) {
  const [filter, setFilter] = useState<StateFilter>('open')

  const filtered = useMemo(() => {
    if (filter === 'all') return issues
    return issues.filter((i) => i.state === filter)
  }, [issues, filter])

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4 h-full">
        <AlertTriangle className="w-8 h-8 text-foreground-muted/30" />
        <span className="text-xs font-medium text-foreground-muted">Failed to load</span>
        <span className="text-[10px] text-foreground-muted/60 text-center">{error}</span>
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Filter bar */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-border shrink-0">
        {(['open', 'closed', 'all'] as StateFilter[]).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={cn(
              'px-2 py-0.5 rounded-md text-[10px] font-medium transition-colors capitalize',
              filter === f
                ? 'bg-brand/15 text-brand'
                : 'text-foreground-muted hover:bg-background-tertiary',
            )}
          >
            {f}
          </button>
        ))}
      </div>

      {/* Issue list */}
      {filtered.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
          <Inbox className="w-8 h-8 text-foreground-muted/30" />
          <span className="text-[11px] text-foreground-muted">No issues</span>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          {filtered.map((issue) => (
            <IssueItem key={issue.number} issue={issue} />
          ))}
        </div>
      )}

      {/* Summary bar */}
      <div className="flex items-center justify-between px-3 h-6 border-t border-border shrink-0">
        <span className="text-[10px] text-foreground-muted/50">
          {filtered.length} of {issues.length}
        </span>
        {filter !== 'all' && (
          <span className="text-[10px] text-brand/50">{filter}</span>
        )}
      </div>
    </div>
  )
}
