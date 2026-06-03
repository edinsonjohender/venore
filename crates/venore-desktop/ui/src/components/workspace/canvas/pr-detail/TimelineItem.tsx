// =============================================================================
// TimelineItem — A single entry in the PR conversation timeline
// =============================================================================
// Avatar on the left with vertical connector line, content card on the right.

import { MarkdownRenderer } from '@/components/ui/markdown-renderer'
import { timeAgo } from '../../panels/github/utils'

interface TimelineItemProps {
  author: string
  authorAvatar: string
  createdAt: string
  body: string
  /** Optional action label like "opened this pull request" */
  action?: string
}

export function TimelineItem({ author, authorAvatar, createdAt, body, action }: TimelineItemProps) {
  return (
    <div className="relative flex gap-3 pl-4">
      {/* Vertical connector line */}
      <div className="absolute left-[23px] top-8 bottom-0 w-px bg-border/40" />

      {/* Avatar */}
      <img
        src={authorAvatar}
        alt={author}
        className="w-7 h-7 rounded-full shrink-0 mt-0.5 z-10 ring-2 ring-background-secondary"
      />

      {/* Content card */}
      <div className="flex-1 min-w-0 pb-4">
        <div className="rounded-lg border border-border/40 bg-background-secondary overflow-hidden">
          {/* Header */}
          <div className="flex items-center gap-1.5 px-3 py-1.5 bg-background-tertiary/30 border-b border-border/30 text-[10px]">
            <span className="font-semibold text-foreground">{author}</span>
            {action && <span className="text-foreground-muted">{action}</span>}
            <span className="text-foreground-muted/50">{timeAgo(createdAt)}</span>
          </div>
          {/* Body */}
          {body && (
            <div className="px-3 py-2">
              <MarkdownRenderer content={body} className="text-xs" />
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
