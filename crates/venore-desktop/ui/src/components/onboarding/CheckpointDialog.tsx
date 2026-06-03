import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/modal'
import { Clock, CheckCircle } from 'lucide-react'
import { formatTimeAgo } from '@/lib/time'
import type { CheckpointInfo } from '@/lib/tauri'

export interface CheckpointDialogProps {
  open: boolean
  checkpoint: CheckpointInfo | null
  lastUpdatedAt?: string // ISO 8601 timestamp
  /** `disk` = legacy batch-generation checkpoint (counts modules); `local` =
   *  new-flow draft from localStorage (counts wizard steps). Changes the
   *  progress label only — buttons and behavior identical. */
  kind?: 'disk' | 'local'
  onContinue: () => void
  onStartNew: () => void
  onCancel: () => void
}

export function CheckpointDialog({
  open,
  checkpoint,
  lastUpdatedAt,
  kind = 'disk',
  onContinue,
  onStartNew,
  onCancel,
}: CheckpointDialogProps) {
  const { t } = useTranslation('wizard')

  if (!checkpoint) return null

  const formattedTime = lastUpdatedAt ? formatTimeAgo(lastUpdatedAt) : formatTimeAgo(Date.now())

  return (
    <Modal
      open={open}
      onOpenChange={(isOpen) => { if (!isOpen) onCancel() }}
      title={t('checkpoint.title')}
      description={t('checkpoint.description')}
      maxWidth="max-w-[548px]"
      footer={
        <div className="flex items-center justify-between w-full">
          <Button variant="ghost" onClick={onCancel}>
            {t('checkpoint.cancel')}
          </Button>
          <div className="flex items-center gap-3">
            <Button onClick={onContinue}>
              {t('checkpoint.continueWhereILeftOff')}
            </Button>
            <Button variant="destructive" onClick={onStartNew}>
              {t('checkpoint.startNew')}
            </Button>
          </div>
        </div>
      }
    >
      {/* Info Card */}
      <div className="space-y-3 p-4 bg-background-secondary rounded-lg border border-border">
        <div className="flex items-center gap-2 text-sm">
          <Clock className="w-4 h-4 text-muted-foreground" />
          <span className="text-foreground">
            {t('checkpoint.lastUpdated', { time: formattedTime })}
          </span>
        </div>

        <div className="flex items-center gap-2 text-sm">
          <CheckCircle className="w-4 h-4 text-brand" />
          <span className="text-foreground">
            {t(
              kind === 'local' ? 'checkpoint.stepsCompleted' : 'checkpoint.modulesGenerated',
              { completed: checkpoint.completed_count, total: checkpoint.total_count }
            )}
          </span>
        </div>

        {/* Progress Bar */}
        <div className="w-full bg-background rounded-full h-2.5 border border-border overflow-hidden">
          <div
            className="bg-brand h-full transition-all duration-300 ease-out"
            style={{ width: `${checkpoint.progress_percent}%` }}
          />
        </div>

        <p className="text-sm text-muted-foreground text-center font-medium">
          {t('checkpoint.percentCompleted', { percent: checkpoint.progress_percent })}
        </p>
      </div>
    </Modal>
  )
}
