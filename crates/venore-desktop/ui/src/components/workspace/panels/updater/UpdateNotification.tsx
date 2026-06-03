// =============================================================================
// UpdateNotification - Badge shown when modules are outdated
// =============================================================================

import { RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useUpdaterStore } from '@/stores/updaterStore'

interface UpdateNotificationProps {
  onClick: () => void
}

export function UpdateNotification({ onClick }: UpdateNotificationProps) {
  const { t } = useTranslation('updater')
  const report = useUpdaterStore((s) => s.updateReport)
  const isChecking = useUpdaterStore((s) => s.isChecking)

  if (isChecking) {
    return (
      <button
        className="flex items-center gap-1.5 rounded-md bg-muted/50 px-2.5 py-1 text-xs text-muted-foreground animate-pulse"
        disabled
      >
        <RefreshCw className="h-3 w-3 animate-spin" />
        {t('notification.checking')}
      </button>
    )
  }

  if (!report || report.affected_modules.length === 0) return null

  const count = report.affected_modules.length

  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1.5 rounded-md bg-amber-500/10 px-2.5 py-1 text-xs font-medium text-amber-500 hover:bg-amber-500/20 transition-colors"
    >
      <RefreshCw className="h-3 w-3" />
      {t('notification.count', { count })}
    </button>
  )
}
