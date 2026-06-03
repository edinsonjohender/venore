// =============================================================================
// GitHub panel utilities — shared helpers
// =============================================================================

export function timeAgo(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diffMs = now - then
  const diffMin = Math.floor(diffMs / 60000)
  if (diffMin < 1) return 'just now'
  if (diffMin < 60) return `${diffMin}m`
  const diffH = Math.floor(diffMin / 60)
  if (diffH < 24) return `${diffH}h`
  const diffD = Math.floor(diffH / 24)
  if (diffD < 30) return `${diffD}d`
  const diffMo = Math.floor(diffD / 30)
  return `${diffMo}mo`
}

export function openInBrowser(url: string) {
  import('@tauri-apps/plugin-shell').then(({ open }) => open(url))
}
