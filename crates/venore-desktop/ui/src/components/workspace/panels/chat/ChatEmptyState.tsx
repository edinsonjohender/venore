// =============================================================================
// ChatEmptyState - Centered placeholder when no messages exist
// =============================================================================

import { MessageSquare } from 'lucide-react'
import { useTranslation } from 'react-i18next'

export function ChatEmptyState() {
  const { t } = useTranslation('chat')

  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
      <MessageSquare className="w-10 h-10 text-foreground-muted/20" />
      <span className="text-sm font-medium text-foreground-muted">{t('emptyState.title')}</span>
      <span className="text-xs text-foreground-subtle text-center">
        {t('emptyState.description')}
      </span>
    </div>
  )
}
