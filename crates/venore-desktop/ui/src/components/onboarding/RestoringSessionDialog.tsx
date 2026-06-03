// =============================================================================
// RestoringSessionDialog - Loading feedback while restoring a checkpoint session
// =============================================================================
// Shown after the user clicks "Continue where I left off" in CheckpointDialog.
// Listens to analysis-progress events emitted by detectProjectModules on backend.

import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { listen } from '@tauri-apps/api/event'
import { Loader2 } from 'lucide-react'
import { Modal } from '@/components/ui/modal'
import { createLogger } from '@/lib/logger'

const log = createLogger('wizard:restore')

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface AnalysisProgress {
  current: number
  total: number
  description: string
}

interface RestoringSessionDialogProps {
  open: boolean
  projectPath: string
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function RestoringSessionDialog({ open, projectPath }: RestoringSessionDialogProps) {
  const { t } = useTranslation('wizard')
  const [progress, setProgress] = useState<AnalysisProgress | null>(null)

  // Listen for real-time analysis progress events (same events as Step3)
  useEffect(() => {
    if (!open || !projectPath) return

    let unlistenProgress: (() => void) | null = null

    const setup = async () => {
      unlistenProgress = await listen<{
        session_id: string
        current_step: number
        total_steps: number
        step_description: string
        current_item?: string
      }>('analysis-progress', (event) => {
        const payload = event.payload
        if (payload.session_id === projectPath) {
          setProgress({
            current: payload.current_step,
            total: payload.total_steps,
            description: payload.step_description + (payload.current_item ? ` (${payload.current_item})` : ''),
          })
        }
      })
    }

    setup().catch((err) => log.error('Failed to setup analysis listeners', err))

    return () => {
      if (unlistenProgress) unlistenProgress()
    }
  }, [open, projectPath])

  // Reset progress when dialog closes
  useEffect(() => {
    if (!open) setProgress(null)
  }, [open])

  const percent = progress ? Math.round((progress.current / progress.total) * 100) : 0

  return (
    <Modal
      open={open}
      onOpenChange={() => {}}
      title={t('restoring.title')}
      description={t('restoring.description')}
      maxWidth="max-w-[548px]"
      blockClose
    >
      <div className="flex flex-col items-center justify-center py-8">
        <Loader2 size={32} className="text-primary animate-spin mb-4" />

        <p className="text-sm mb-3">{t('restoring.analyzingProject')}</p>

        {/* Progress Bar */}
        <div className="w-64 h-1.5 bg-secondary rounded-full overflow-hidden mb-2">
          <div
            className="h-full bg-primary transition-all duration-300"
            style={{ width: `${percent}%` }}
          />
        </div>

        {/* Step Counter */}
        <p className="text-xs text-muted-foreground">
          {progress
            ? t('restoring.stepProgress', { current: progress.current, total: progress.total, description: progress.description })
            : t('restoring.starting')}
        </p>
      </div>
    </Modal>
  )
}
