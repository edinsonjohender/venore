// =============================================================================
// PullsTab - Pull request list with expandable items
// =============================================================================

import { useEffect, useMemo, useState } from 'react'
import {
  GitPullRequest, GitMerge, Inbox, Loader2, AlertTriangle,
  ExternalLink, ChevronRight, MessageSquare, GitBranch, Eye, Bot, X,
  Search, FileText, Layers, Zap,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { tauriApi } from '@/lib/tauri'
import type { GitHubPullRequestDto, GitHubLabelDto, AnalysisDepthLevel } from '@/lib/tauri'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { timeAgo, openInBrowser } from './utils'

type StateFilter = 'open' | 'closed' | 'all'

const DEPTH_OPTIONS: { level: AnalysisDepthLevel; label: string; description: string; icon: typeof Search }[] = [
  { level: 'minimal',  label: 'Minimal',  description: 'Only PR patches — fastest, lowest cost',              icon: Zap      },
  { level: 'normal',   label: 'Normal',   description: 'Patches + project context files (.context.md)',       icon: FileText },
  { level: 'detailed', label: 'Detailed', description: 'Adds RAG symbol search + module health analysis',     icon: Search   },
  { level: 'expert',   label: 'Expert',   description: 'Full changed files + all enrichments — most thorough', icon: Layers   },
]

interface AnalyzeModalProps {
  pr: GitHubPullRequestDto
  projectPath: string
  onClose: () => void
  onStarted: () => void
}

function AnalyzeModal({ pr, projectPath, onClose, onStarted }: AnalyzeModalProps) {
  const [depth, setDepth] = useState<AnalysisDepthLevel>('normal')
  const [starting, setStarting] = useState(false)
  const openAI = useCanvasTabStore((s) => s.openAI)

  // Load persisted depth setting
  useEffect(() => {
    tauriApi.getAnalysisDepth()
      .then((d) => setDepth(d as AnalysisDepthLevel))
      .catch(() => { /* keep default */ })
  }, [])

  const handleStart = () => {
    setStarting(true)
    // Persist the selected depth, then start pipeline
    tauriApi.setAnalysisDepth(depth)
      .catch(() => { /* ignore */ })
    tauriApi.startPipeline({ projectPath, prNumber: pr.number, prTitle: pr.title })
      .then(() => {
        onStarted()
        openAI()
        onClose()
      })
      .catch((err) => {
        console.error('Failed to start pipeline:', err)
        setStarting(false)
      })
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" onClick={onClose}>
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />

      {/* Modal */}
      <div
        className="relative w-[340px] bg-background-secondary border border-border rounded-lg shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border/50">
          <div className="flex items-center gap-2 min-w-0">
            <Bot className="w-4 h-4 text-amber-400 shrink-0" />
            <span className="text-xs font-medium text-foreground truncate">Analyze PR</span>
          </div>
          <button
            onClick={onClose}
            className="p-0.5 rounded hover:bg-background-tertiary text-foreground-muted/50 hover:text-foreground-muted transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* PR info */}
        <div className="px-4 py-2.5 border-b border-border/30 bg-background-tertiary/30">
          <div className="flex items-center gap-2">
            <GitPullRequest className="w-3.5 h-3.5 text-green-500 shrink-0" />
            <span className="text-[11px] font-medium text-foreground truncate">{pr.title}</span>
          </div>
          <div className="flex items-center gap-1.5 mt-1 ml-[22px] text-[10px] text-foreground-muted">
            <span className="font-mono">#{pr.number}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{pr.author}</span>
          </div>
        </div>

        {/* Depth selector */}
        <div className="px-4 py-3 space-y-1.5">
          <span className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
            Analysis Depth
          </span>
          <div className="space-y-1">
            {DEPTH_OPTIONS.map(({ level, label, description, icon: Icon }) => (
              <button
                key={level}
                onClick={() => setDepth(level)}
                className={cn(
                  'w-full flex items-start gap-2.5 px-2.5 py-2 rounded-md text-left transition-colors',
                  depth === level
                    ? 'bg-brand/15 border border-brand/30'
                    : 'border border-transparent hover:bg-background-tertiary/60',
                )}
              >
                <Icon className={cn(
                  'w-3.5 h-3.5 mt-0.5 shrink-0',
                  depth === level ? 'text-brand' : 'text-foreground-muted/40',
                )} />
                <div className="min-w-0">
                  <span className={cn(
                    'text-[11px] font-medium',
                    depth === level ? 'text-brand' : 'text-foreground',
                  )}>
                    {label}
                  </span>
                  <p className="text-[10px] text-foreground-muted/60 leading-tight mt-0.5">
                    {description}
                  </p>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Actions */}
        <div className="px-4 py-3 border-t border-border/30 flex items-center justify-end gap-2">
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded-md text-[11px] font-medium text-foreground-muted hover:bg-background-tertiary transition-colors"
          >
            Cancel
          </button>
          <button
            disabled={starting}
            onClick={handleStart}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[11px] font-medium transition-colors',
              starting
                ? 'bg-amber-500/10 text-amber-400/60 cursor-not-allowed'
                : 'bg-amber-500/20 hover:bg-amber-500/30 text-amber-400',
            )}
          >
            {starting ? <Loader2 className="w-3 h-3 animate-spin" /> : <Bot className="w-3 h-3" />}
            Start Analysis
          </button>
        </div>
      </div>
    </div>
  )
}

interface PullsTabProps {
  pulls: GitHubPullRequestDto[]
  loading: boolean
  error: string | null
  projectPath: string
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

function PullItem({ pr, projectPath }: { pr: GitHubPullRequestDto; projectPath: string }) {
  const [expanded, setExpanded] = useState(false)
  const [showAnalyzeModal, setShowAnalyzeModal] = useState(false)
  const openPr = useCanvasTabStore((s) => s.openPr)

  return (
    <div className="border-b border-border/50 last:border-b-0">
      {/* Collapsed row */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2.5 px-3 py-2.5 hover:bg-background-tertiary/50 transition-colors text-left group"
      >
        {/* State icon */}
        <div className="shrink-0">
          {pr.state === 'open' ? (
            <GitPullRequest className="w-4 h-4 text-green-500" />
          ) : (
            <GitMerge className="w-4 h-4 text-purple-500" />
          )}
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium text-foreground truncate">
              {pr.title}
            </span>
            {pr.draft && (
              <span className="shrink-0 text-[9px] px-1.5 py-0.5 rounded-full bg-amber-500/15 text-amber-400 font-medium">
                Draft
              </span>
            )}
          </div>
          <div className="flex items-center gap-1.5 mt-1 text-[10px] text-foreground-muted">
            <span className="font-mono">#{pr.number}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{pr.author}</span>
            <span className="text-foreground-muted/30">&middot;</span>
            <span>{timeAgo(pr.updated_at)}</span>
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
          {/* Branch info */}
          <div className="flex items-center gap-1.5 text-[10px] text-foreground-muted">
            <GitBranch className="w-3 h-3 shrink-0" />
            <span className="font-mono truncate">{pr.head_ref}</span>
            <span className="text-foreground-muted/40">&rarr;</span>
            <span className="font-mono truncate">{pr.base_ref}</span>
          </div>

          {/* Stats */}
          <div className="flex items-center gap-3 text-[10px] text-foreground-muted">
            {pr.comments > 0 && (
              <span className="flex items-center gap-1">
                <MessageSquare className="w-3 h-3" />
                {pr.comments}
              </span>
            )}
            {pr.review_comments > 0 && (
              <span className="flex items-center gap-1">
                <GitPullRequest className="w-3 h-3" />
                {pr.review_comments} reviews
              </span>
            )}
          </div>

          {/* Labels */}
          {pr.labels.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {pr.labels.map((l) => (
                <LabelChip key={l.name} label={l} />
              ))}
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={(e) => { e.stopPropagation(); openPr(pr.number, pr.title) }}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] font-medium
                         bg-brand/15 hover:bg-brand/25 text-brand
                         transition-colors"
            >
              <Eye className="w-3 h-3" />
              Open
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation()
                setShowAnalyzeModal(true)
              }}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] font-medium transition-colors
                         bg-amber-500/15 hover:bg-amber-500/25 text-amber-400"
            >
              <Bot className="w-3 h-3" />
              Analyze
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); openInBrowser(pr.html_url) }}
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

      {/* Analyze configuration modal */}
      {showAnalyzeModal && (
        <AnalyzeModal
          pr={pr}
          projectPath={projectPath}
          onClose={() => setShowAnalyzeModal(false)}
          onStarted={() => setShowAnalyzeModal(false)}
        />
      )}
    </div>
  )
}

export function PullsTab({ pulls, loading, error, projectPath }: PullsTabProps) {
  const [filter, setFilter] = useState<StateFilter>('open')

  const filtered = useMemo(() => {
    if (filter === 'all') return pulls
    return pulls.filter((pr) => pr.state === filter)
  }, [pulls, filter])

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

      {/* PR list */}
      {filtered.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
          <Inbox className="w-8 h-8 text-foreground-muted/30" />
          <span className="text-[11px] text-foreground-muted">No pull requests</span>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          {filtered.map((pr) => (
            <PullItem key={pr.number} pr={pr} projectPath={projectPath} />
          ))}
        </div>
      )}

      {/* Summary bar */}
      <div className="flex items-center justify-between px-3 h-6 border-t border-border shrink-0">
        <span className="text-[10px] text-foreground-muted/50">
          {filtered.length} of {pulls.length}
        </span>
        {filter !== 'all' && (
          <span className="text-[10px] text-brand/50">{filter}</span>
        )}
      </div>
    </div>
  )
}
