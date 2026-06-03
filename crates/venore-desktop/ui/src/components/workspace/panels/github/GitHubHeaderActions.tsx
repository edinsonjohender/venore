// =============================================================================
// GitHubHeaderActions - Refresh button for panel header
// =============================================================================

import { RefreshCw } from 'lucide-react'
import type { PanelContentProps } from '../registry'

// The refresh callback is set by the panel and read here via a simple ref.
// This avoids prop drilling through the registry system.
let _onRefresh: (() => void) | null = null
let _refreshing = false

export function setGitHubRefreshHandler(fn: (() => void) | null, loading: boolean) {
  _onRefresh = fn
  _refreshing = loading
}

export function GitHubHeaderActions(_props: PanelContentProps) {
  return (
    <button
      onClick={() => _onRefresh?.()}
      disabled={_refreshing}
      className="p-0.5 rounded hover:bg-background-tertiary text-foreground-muted transition-colors"
      title="Refresh"
    >
      <RefreshCw className={`w-3 h-3 ${_refreshing ? 'animate-spin' : ''}`} />
    </button>
  )
}
