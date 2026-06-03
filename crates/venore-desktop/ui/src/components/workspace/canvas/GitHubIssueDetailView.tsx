// =============================================================================
// GitHubIssueDetailView - Issue detail tab content
// =============================================================================
// Shows issue header, body with markdown, and comments with markdown.
// Fetches comment data on mount via tauri commands.

import { useEffect, useState } from 'react'
import {
  CircleDot, CheckCircle2, Loader2, AlertTriangle,
  MessageSquare, User,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { tauriApi } from '@/lib/tauri'
import type { GitHubCommentDto, GitHubIssueDto, GitHubLabelDto } from '@/lib/tauri'
import { timeAgo } from '../panels/github/utils'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface GitHubIssueDetailViewProps {
  number: number
  title: string
  projectPath: string
  /** Full issue data from the sidebar listing */
  issueData?: GitHubIssueDto
}

// -----------------------------------------------------------------------------
// Sub-components
// -----------------------------------------------------------------------------

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
      <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: `#${label.color}` }} />
      {label.name}
    </span>
  )
}

function CommentItem({ comment }: { comment: GitHubCommentDto }) {
  return (
    <div className="flex gap-2.5 px-3 py-2.5 border-b border-border/30 last:border-b-0">
      <img
        src={comment.author_avatar}
        alt={comment.author}
        className="w-6 h-6 rounded-full shrink-0 mt-0.5"
      />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5 text-[10px]">
          <span className="font-medium text-foreground">{comment.author}</span>
          <span className="text-foreground-muted/50">{timeAgo(comment.created_at)}</span>
        </div>
        <div className="mt-1">
          <MarkdownRenderer content={comment.body} className="text-xs" />
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function GitHubIssueDetailView({ number, title, projectPath, issueData }: GitHubIssueDetailViewProps) {
  const [comments, setComments] = useState<GitHubCommentDto[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)

    tauriApi.githubGetComments({ project_path: projectPath, number, is_pull_request: false })
      .then((res) => {
        if (cancelled) return
        setComments(res.comments)
      })
      .catch((err) => {
        if (cancelled) return
        setError(err instanceof Error ? err.message : String(err))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })

    return () => { cancelled = true }
  }, [number, projectPath])

  if (loading) {
    return (
      <div className="absolute inset-0 flex items-center justify-center">
        <Loader2 className="w-6 h-6 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-6">
        <AlertTriangle className="w-8 h-8 text-foreground-muted/30" />
        <span className="text-xs font-medium text-foreground-muted">Failed to load issue details</span>
        <span className="text-[10px] text-foreground-muted/60 text-center">{error}</span>
      </div>
    )
  }

  return (
    <div className="absolute inset-0 overflow-y-auto">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border">
        <div className="flex items-center gap-2 mb-1.5">
          {issueData?.state === 'open' ? (
            <CircleDot className="w-4 h-4 text-green-500 shrink-0" />
          ) : (
            <CheckCircle2 className="w-4 h-4 text-purple-500 shrink-0" />
          )}
          <h2 className="text-sm font-semibold text-foreground">{title}</h2>
          <span className="text-xs font-mono text-foreground-muted">#{number}</span>
        </div>
        {issueData && (
          <div className="flex flex-wrap items-center gap-2 text-[10px] text-foreground-muted">
            <span>{issueData.author}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span className={cn(
              'px-1.5 py-0.5 rounded-full font-medium',
              issueData.state === 'open' ? 'bg-green-500/15 text-green-400' : 'bg-purple-500/15 text-purple-400',
            )}>
              {issueData.state}
            </span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{timeAgo(issueData.created_at)}</span>
            {issueData.assignees.length > 0 && (
              <>
                <span className="text-foreground-muted/30">&middot;</span>
                <span className="flex items-center gap-1">
                  <User className="w-3 h-3" />
                  {issueData.assignees.join(', ')}
                </span>
              </>
            )}
            {issueData.labels.length > 0 && (
              <>
                <span className="text-foreground-muted/30">&middot;</span>
                <div className="flex flex-wrap gap-1">
                  {issueData.labels.map((l) => <LabelChip key={l.name} label={l} />)}
                </div>
              </>
            )}
          </div>
        )}
      </div>

      {/* Body */}
      {issueData?.body && (
        <div className="px-4 py-3 border-b border-border">
          <MarkdownRenderer content={issueData.body} />
        </div>
      )}

      {/* Comments */}
      {comments.length > 0 ? (
        <div>
          <div className="flex items-center gap-2 px-4 py-2 text-xs font-medium text-foreground-muted">
            <MessageSquare className="w-3.5 h-3.5" />
            Comments ({comments.length})
          </div>
          {comments.map((c) => <CommentItem key={c.id} comment={c} />)}
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center py-8 text-foreground-muted/50">
          <MessageSquare className="w-6 h-6 mb-2" />
          <span className="text-xs">No comments</span>
        </div>
      )}
    </div>
  )
}
