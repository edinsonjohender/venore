// =============================================================================
// SessionSummaryTab — Session objective + stats + placeholder summary
// =============================================================================

import { useTranslation } from 'react-i18next'
import { GitBranch, FileCode, Sparkles } from 'lucide-react'
import type { SessionDto } from '@/lib/tauri'

interface SessionSummaryTabProps {
  session: SessionDto
}

export function SessionSummaryTab({ session }: SessionSummaryTabProps) {
  const { t } = useTranslation('sessions')

  return (
    <div className="p-4 space-y-4 max-w-lg">
      {/* Objective */}
      {session.objective && (
        <div>
          <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-1">
            {t('summary.objective')}
          </h3>
          <p className="text-[12px] text-foreground leading-relaxed">
            {session.objective}
          </p>
        </div>
      )}

      {/* Stats */}
      <div>
        <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2">
          {t('summary.stats')}
        </h3>
        <div className="grid grid-cols-3 gap-3">
          <div className="bg-background-tertiary rounded px-3 py-2">
            <div className="flex items-center gap-1.5 text-[10px] text-foreground-muted/60 mb-0.5">
              <FileCode className="w-3 h-3" />
              {t('summary.files')}
            </div>
            <div className="text-sm font-medium text-foreground">{session.files_changed}</div>
          </div>
          <div className="bg-background-tertiary rounded px-3 py-2">
            <div className="text-[10px] text-green-400/60 mb-0.5">{t('summary.additions')}</div>
            <div className="text-sm font-medium text-green-400">+{session.additions}</div>
          </div>
          <div className="bg-background-tertiary rounded px-3 py-2">
            <div className="text-[10px] text-red-400/60 mb-0.5">{t('summary.deletions')}</div>
            <div className="text-sm font-medium text-red-400">-{session.deletions}</div>
          </div>
        </div>
      </div>

      {/* Branch info */}
      <div>
        <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2">
          {t('summary.branch')}
        </h3>
        <div className="flex items-center gap-2 text-[11px]">
          <GitBranch className="w-3.5 h-3.5 text-foreground-muted/40" />
          <span className="font-mono text-foreground">{session.session_branch}</span>
          <span className="text-foreground-muted/40">&larr;</span>
          <span className="font-mono text-foreground-muted">{session.base_branch}</span>
        </div>
      </div>

      {/* AI Summary placeholder */}
      <div>
        <h3 className="text-[10px] font-medium text-foreground-muted/60 uppercase tracking-wider mb-2">
          {t('summary.aiSummary')}
        </h3>
        <button
          disabled
          className="flex items-center gap-2 px-3 py-2 text-[11px] bg-background-tertiary rounded text-foreground-muted/50 cursor-not-allowed"
        >
          <Sparkles className="w-3.5 h-3.5" />
          {t('summary.generateComingSoon')}
        </button>
      </div>
    </div>
  )
}
