// =============================================================================
// GitHubTabs - PRs / Issues tab switcher
// =============================================================================

import { GitPullRequest, CircleDot } from 'lucide-react'
import { cn } from '@/lib/utils'

export type GitHubTabId = 'pulls' | 'issues'

interface GitHubTabsProps {
  active: GitHubTabId
  onChange: (tab: GitHubTabId) => void
  pullCount: number | null
  issueCount: number | null
}

const TABS: { id: GitHubTabId; label: string; icon: typeof GitPullRequest }[] = [
  { id: 'pulls', label: 'PRs', icon: GitPullRequest },
  { id: 'issues', label: 'Issues', icon: CircleDot },
]

export function GitHubTabs({ active, onChange, pullCount, issueCount }: GitHubTabsProps) {
  const counts: Record<GitHubTabId, number | null> = { pulls: pullCount, issues: issueCount }

  return (
    <div className="flex shrink-0 border-b border-border">
      {TABS.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          onClick={() => onChange(id)}
          className={cn(
            'flex-1 flex items-center justify-center gap-1.5 h-8 text-xs font-medium',
            'border-b-2 transition-colors',
            active === id
              ? 'text-brand border-brand'
              : 'text-foreground-muted border-transparent hover:text-foreground',
          )}
        >
          <Icon className="w-3.5 h-3.5" />
          {label}
          {counts[id] != null && (
            <span className={cn(
              'text-[10px] tabular-nums',
              active === id ? 'text-brand/70' : 'text-foreground-muted/50',
            )}>
              {counts[id]}
            </span>
          )}
        </button>
      ))}
    </div>
  )
}
