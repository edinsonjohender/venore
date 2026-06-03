// =============================================================================
// GitHubPrDetailView - PR detail with split layout and diff viewer
// =============================================================================
// Header (title, state, branches, labels) + PrInnerTabs (Files/Conversation/AI).
// Files tab: split layout with file tree (left) and diff viewer (right).
// Conversation tab: PR body + comments + review comments timeline.

import { useEffect, useState } from 'react'
import {
  GitPullRequest, GitMerge, GitBranch, Loader2, AlertTriangle,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { tauriApi } from '@/lib/tauri'
import type {
  GitHubPrFileDto, GitHubCommentDto, GitHubReviewCommentDto,
  GitHubPullRequestDto, GitHubLabelDto, GitHubPrDetailResponse,
} from '@/lib/tauri'
import { timeAgo } from '../panels/github/utils'
import { PrInnerTabs } from './pr-detail'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface GitHubPrDetailViewProps {
  number: number
  title: string
  projectPath: string
  prData?: GitHubPullRequestDto
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

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function GitHubPrDetailView({ number, title, projectPath, prData }: GitHubPrDetailViewProps) {
  const [files, setFiles] = useState<GitHubPrFileDto[]>([])
  const [comments, setComments] = useState<GitHubCommentDto[]>([])
  const [reviewComments, setReviewComments] = useState<GitHubReviewCommentDto[]>([])
  const [prDetail, setPrDetail] = useState<GitHubPrDetailResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)

    Promise.all([
      tauriApi.githubGetPrFiles({ project_path: projectPath, pr_number: number }),
      tauriApi.githubGetComments({ project_path: projectPath, number, is_pull_request: true }),
      tauriApi.githubGetPrDetail({ project_path: projectPath, pr_number: number }),
    ])
      .then(([filesRes, commentsRes, detailRes]) => {
        if (cancelled) return
        setFiles(filesRes.files)
        setComments(commentsRes.comments)
        setReviewComments(commentsRes.review_comments)
        setPrDetail(detailRes)
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
        <span className="text-xs font-medium text-foreground-muted">Failed to load PR details</span>
        <span className="text-[10px] text-foreground-muted/60 text-center">{error}</span>
      </div>
    )
  }

  return (
    <div className="absolute inset-0 flex flex-col">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border shrink-0">
        <div className="flex items-center gap-2 mb-1.5">
          {prData?.state === 'open' ? (
            <GitPullRequest className="w-4 h-4 text-green-500 shrink-0" />
          ) : (
            <GitMerge className="w-4 h-4 text-purple-500 shrink-0" />
          )}
          <h2 className="text-sm font-semibold text-foreground">{title}</h2>
          <span className="text-xs font-mono text-foreground-muted">#{number}</span>
        </div>
        {prData && (
          <div className="flex flex-wrap items-center gap-2 text-[10px] text-foreground-muted">
            <span>{prData.author}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span className={cn(
              'px-1.5 py-0.5 rounded-full font-medium',
              prData.state === 'open' ? 'bg-green-500/15 text-green-400' : 'bg-purple-500/15 text-purple-400',
            )}>
              {prData.state}
            </span>
            {prData.draft && (
              <span className="px-1.5 py-0.5 rounded-full bg-amber-500/15 text-amber-400 font-medium">Draft</span>
            )}
            <span className="text-foreground-muted/30">&middot;</span>
            <span className="flex items-center gap-1">
              <GitBranch className="w-3 h-3" />
              <span className="font-mono">{prData.head_ref}</span>
              <span className="text-foreground-muted/40">&rarr;</span>
              <span className="font-mono">{prData.base_ref}</span>
            </span>
            {prDetail && (
              <>
                <span className="text-foreground-muted/30">&middot;</span>
                <span className="text-green-400">+{prDetail.additions}</span>
                <span className="text-red-400">-{prDetail.deletions}</span>
                <span className="text-foreground-muted/50">{prDetail.changed_files} files</span>
              </>
            )}
            {prData.labels.length > 0 && (
              <>
                <span className="text-foreground-muted/30">&middot;</span>
                <div className="flex flex-wrap gap-1">
                  {prData.labels.map((l) => <LabelChip key={l.name} label={l} />)}
                </div>
              </>
            )}
          </div>
        )}
      </div>

      {/* Tabs body */}
      <PrInnerTabs
        files={files}
        comments={comments}
        reviewComments={reviewComments}
        projectPath={projectPath}
        prNumber={number}
        prData={prData}
        prDetail={prDetail}
      />
    </div>
  )
}
