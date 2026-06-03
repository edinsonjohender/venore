// =============================================================================
// ReviewCommentGroup — Review comments grouped by file path
// =============================================================================
// Shows the file path header, then each review comment with its diff hunk
// context highlighted via shiki.

import { useState, useEffect, useMemo } from 'react'
import { FileCode } from 'lucide-react'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'
import { highlightCode, getLanguageFromFilename } from '@/lib/highlighter'
import { timeAgo } from '../../panels/github/utils'
import type { GitHubReviewCommentDto } from '@/lib/tauri'

interface ReviewCommentGroupProps {
  path: string
  comments: GitHubReviewCommentDto[]
}

export function ReviewCommentGroup({ path, comments }: ReviewCommentGroupProps) {
  return (
    <div className="relative flex gap-3 pl-4 pb-4">
      {/* Connector line */}
      <div className="absolute left-[23px] top-8 bottom-0 w-px bg-border/40" />

      {/* File icon placeholder for avatar column */}
      <div className="w-7 h-7 rounded-full shrink-0 mt-0.5 z-10 ring-2 ring-background-secondary bg-background-tertiary flex items-center justify-center">
        <FileCode className="w-3.5 h-3.5 text-foreground-muted" />
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="rounded-lg border border-border/40 bg-background-secondary overflow-hidden">
          {/* File path header */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-background-tertiary/30 border-b border-border/30">
            <FileCode className="w-3 h-3 text-foreground-muted shrink-0" />
            <span className="font-mono text-[11px] text-foreground truncate">{path}</span>
          </div>

          {/* Individual comments */}
          {comments.map((comment) => (
            <ReviewCommentEntry key={comment.id} comment={comment} />
          ))}
        </div>
      </div>
    </div>
  )
}

function ReviewCommentEntry({ comment }: { comment: GitHubReviewCommentDto }) {
  return (
    <div className="border-b border-border/20 last:border-b-0">
      {/* Diff hunk context */}
      {comment.diff_hunk && (
        <DiffHunkPreview hunk={comment.diff_hunk} path={comment.path} />
      )}

      {/* Comment header + body */}
      <div className="px-3 py-2">
        <div className="flex items-center gap-1.5 text-[10px] mb-1">
          <img
            src={comment.author_avatar}
            alt={comment.author}
            className="w-4 h-4 rounded-full"
          />
          <span className="font-semibold text-foreground">{comment.author}</span>
          {comment.line && (
            <span className="font-mono text-foreground-muted/40">L{comment.line}</span>
          )}
          <span className="text-foreground-muted/50">{timeAgo(comment.created_at)}</span>
        </div>
        <MarkdownRenderer content={comment.body} className="text-xs" />
      </div>
    </div>
  )
}

function DiffHunkPreview({ hunk, path }: { hunk: string; path: string }) {
  const lang = useMemo(() => getLanguageFromFilename(path), [path])
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    highlightCode(hunk, lang).then((html) => {
      if (!cancelled) setHighlightedHtml(html)
    })
    return () => { cancelled = true }
  }, [hunk, lang])

  // Truncate to last few lines of the hunk for context
  const lines = hunk.split('\n')
  const displayLines = lines.slice(Math.max(0, lines.length - 6))
  const truncated = displayLines.join('\n')

  return (
    <div className="bg-background px-3 py-1.5 border-b border-border/20 overflow-x-auto">
      {highlightedHtml ? (
        <div
          className="text-[10px] font-mono leading-tight [&_pre]:!bg-transparent [&_pre]:!p-0 [&_code]:!text-[10px]"
          dangerouslySetInnerHTML={{ __html: highlightedHtml }}
        />
      ) : (
        <pre className="text-[10px] font-mono leading-tight text-foreground-muted/60 whitespace-pre-wrap">
          {truncated}
        </pre>
      )}
    </div>
  )
}
