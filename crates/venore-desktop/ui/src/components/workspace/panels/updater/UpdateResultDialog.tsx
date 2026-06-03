// =============================================================================
// UpdateResultDialog - Shows regeneration progress and results
// =============================================================================

import { CheckCircle, XCircle, Loader2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/modal'
import { useUpdaterStore } from '@/stores/updaterStore'

interface UpdateResultDialogProps {
  open: boolean
  onClose: () => void
  onMarkSynced: () => void
}

export function UpdateResultDialog({
  open,
  onClose,
  onMarkSynced,
}: UpdateResultDialogProps) {
  const { t } = useTranslation('updater')
  const isRegenerating = useUpdaterStore((s) => s.isRegenerating)
  const progress = useUpdaterStore((s) => s.regenerationProgress)

  return (
    <Modal
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen && !isRegenerating) onClose()
      }}
      title={
        isRegenerating ? t('result.regenerating') : t('result.title')
      }
      description={
        isRegenerating
          ? t('result.regeneratingDescription')
          : t('result.completedDescription')
      }
      maxWidth="max-w-[480px]"
      footer={
        !isRegenerating ? (
          <div className="flex items-center justify-between w-full">
            <Button variant="ghost" onClick={onClose}>
              {t('result.dismiss')}
            </Button>
            <Button onClick={onMarkSynced}>{t('result.markSynced')}</Button>
          </div>
        ) : undefined
      }
    >
      <div className="space-y-3">
        {isRegenerating && progress ? (
          <div className="space-y-3">
            {/* Progress bar */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>
                  {t('result.progress', {
                    current: progress.current,
                    total: progress.total,
                  })}
                </span>
                <span>
                  {Math.round((progress.current / progress.total) * 100)}%
                </span>
              </div>
              <div className="h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-primary transition-all duration-300"
                  style={{
                    width: `${(progress.current / progress.total) * 100}%`,
                  }}
                />
              </div>
            </div>
            {/* Current module */}
            <div className="flex items-center gap-2 text-sm">
              <Loader2 className="h-4 w-4 animate-spin text-primary" />
              <span className="text-muted-foreground">
                {progress.moduleId}
              </span>
              <span className="text-xs text-muted-foreground ml-auto">
                {progress.status}
              </span>
            </div>
          </div>
        ) : !isRegenerating && progress ? (
          <div className="flex items-center gap-3 rounded-lg border border-border bg-background-secondary p-3">
            <CheckCircle className="h-5 w-5 text-green-500" />
            <div className="text-sm">
              <span className="font-medium">{t('result.completed')}</span>
              <span className="text-muted-foreground ml-1">
                {t('result.summary', {
                  current: progress.current,
                  total: progress.total,
                })}
              </span>
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-3 text-sm text-muted-foreground">
            <XCircle className="h-5 w-5" />
            {t('result.noProgress')}
          </div>
        )}
      </div>
    </Modal>
  )
}
