// =============================================================================
// ConversationTab — Full PR conversation timeline
// =============================================================================
// Shows: PR body as first item, then comments + review comments merged
// chronologically. Review comments are grouped by file path.

import { useMemo } from 'react'
import { MessageSquare } from 'lucide-react'
import { TimelineItem } from './TimelineItem'
import { ReviewCommentGroup } from './ReviewCommentGroup'
import type { GitHubCommentDto, GitHubReviewCommentDto, GitHubPullRequestDto, GitHubPrDetailResponse } from '@/lib/tauri'

interface ConversationTabProps {
  prData?: GitHubPullRequestDto
  prDetail?: GitHubPrDetailResponse | null
  comments: GitHubCommentDto[]
  reviewComments: GitHubReviewCommentDto[]
}

type TimelineEntry =
  | { type: 'comment'; data: GitHubCommentDto }
  | { type: 'review-group'; path: string; data: GitHubReviewCommentDto[] }

export function ConversationTab({ prData, prDetail, comments, reviewComments }: ConversationTabProps) {
  // Build timeline: merge comments + grouped review comments, sorted by created_at
  const timeline = useMemo(() => {
    const entries: (TimelineEntry & { sortKey: string })[] = []

    // Add general comments
    for (const c of comments) {
      entries.push({ type: 'comment', data: c, sortKey: c.created_at })
    }

    // Group review comments by path, use earliest timestamp as sort key
    const reviewByPath = new Map<string, GitHubReviewCommentDto[]>()
    for (const rc of reviewComments) {
      const group = reviewByPath.get(rc.path) ?? []
      group.push(rc)
      reviewByPath.set(rc.path, group)
    }
    for (const [path, group] of reviewByPath) {
      const sortKey = group.reduce(
        (min, c) => (c.created_at < min ? c.created_at : min),
        group[0].created_at,
      )
      entries.push({ type: 'review-group', path, data: group, sortKey })
    }

    // Sort chronologically
    entries.sort((a, b) => a.sortKey.localeCompare(b.sortKey))

    return entries
  }, [comments, reviewComments])

  // Get body from prDetail (full) or fall back to prData (may be truncated)
  const prBody = prDetail?.body ?? prData?.body ?? null
  const prAuthor = prDetail?.author ?? prData?.author ?? 'Unknown'
  const prAuthorAvatar = prDetail?.author_avatar ?? prData?.author_avatar ?? ''
  const prCreatedAt = prDetail?.created_at ?? prData?.created_at ?? ''

  const isEmpty = !prBody && timeline.length === 0

  if (isEmpty) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-foreground-muted/50">
        <MessageSquare className="w-8 h-8 mb-2" />
        <span className="text-xs">No conversation yet</span>
      </div>
    )
  }

  return (
    <div className="p-4">
      {/* PR body as first timeline item */}
      {prBody && (
        <TimelineItem
          author={prAuthor}
          authorAvatar={prAuthorAvatar}
          createdAt={prCreatedAt}
          body={prBody}
          action="opened this pull request"
        />
      )}

      {/* Timeline entries */}
      {timeline.map((entry) => {
        if (entry.type === 'comment') {
          return (
            <TimelineItem
              key={`c-${entry.data.id}`}
              author={entry.data.author}
              authorAvatar={entry.data.author_avatar}
              createdAt={entry.data.created_at}
              body={entry.data.body}
            />
          )
        }
        return (
          <ReviewCommentGroup
            key={`rg-${entry.path}`}
            path={entry.path}
            comments={entry.data}
          />
        )
      })}
    </div>
  )
}
