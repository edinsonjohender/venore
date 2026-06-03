// =============================================================================
// NodeOverview — Module path, file count, and entry point
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Copy, Check } from 'lucide-react'
import type { ModuleDetailsResponse } from '@/lib/tauri'

interface NodeOverviewProps {
  details: ModuleDetailsResponse
}

export function NodeOverview({ details }: NodeOverviewProps) {
  const { t } = useTranslation('project')
  const [copied, setCopied] = useState(false)

  const handleCopy = () => {
    navigator.clipboard.writeText(details.path).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }

  return (
    <section className="px-3 py-2 border-b border-border">
      {/* Path with copy button */}
      <div className="flex items-center gap-1.5 group">
        <span className="text-xs text-foreground-muted font-mono truncate flex-1">
          {details.path}
        </span>
        <button
          onClick={handleCopy}
          className="shrink-0 p-0.5 rounded text-foreground-muted/50 hover:text-foreground-muted transition-colors opacity-0 group-hover:opacity-100"
          title={t('nodeOverview.copyPath')}
        >
          {copied ? (
            <Check className="w-3 h-3 text-accent" />
          ) : (
            <Copy className="w-3 h-3" />
          )}
        </button>
      </div>

      {/* Stats line */}
      <p className="text-[10px] text-foreground-muted mt-1">
        {t('nodeOverview.fileCount', { count: details.file_count })}
        {details.entry_point && (
          <span className="text-foreground-muted/60">
            {' \u00B7 '}{details.entry_point}
          </span>
        )}
      </p>
    </section>
  )
}
