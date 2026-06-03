// =============================================================================
// Step0PathSelector - Project folder selection
// =============================================================================
//
// ⚠️ DEPRECATED — NO LONGER USED IN THE WIZARD FLOW.
//
// In the 9-step wizard this was a Step that asked the user to pick the
// project folder. In the new 5-step flow, the folder picker fires
// automatically when the wizard opens (see
// `OnboardingWizardModal::selectProjectPath`), so this Step component is
// never rendered. Kept around as a reference / fallback in case we need a
// path-selection UI elsewhere (drag-drop empty state, retry on invalid
// path, etc). If you're certain it's not needed, it's safe to delete.

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FolderOpen, CheckCircle, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { open } from '@tauri-apps/plugin-dialog'
import { basename } from '@tauri-apps/api/path'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function Step0PathSelector() {
  const { t } = useTranslation('wizard')
  const [isSelecting, setIsSelecting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const projectPath = useWizardDataStore((s) => s.step2.projectPath)
  const projectName = useWizardDataStore((s) => s.step2.projectName)
  const setProjectPath = useWizardDataStore((s) => s.setProjectPath)
  const setProjectContextName = useWizardDataStore((s) => s.setProjectName)

  const handleSelectFolder = async () => {
    setIsSelecting(true)
    setError(null)

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('step0.title'),
      })

      if (selected && typeof selected === 'string') {
        // Extract project name from path
        const name = await basename(selected) || t('modal.unnamedProject')

        setProjectPath(selected, name)
        setProjectContextName(name)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to select folder')
    } finally {
      setIsSelecting(false)
    }
  }

  return (
    <div className="mx-auto max-w-2xl space-y-6 p-6">
      {/* Header */}
      <div className="text-center">
        <h2 className="mb-2 text-2xl font-semibold text-foreground">
          {t('step0.title')}
        </h2>
        <p className="text-foreground-muted">
          {t('step0.description')}
        </p>
      </div>

      {/* Folder Selection Card */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('step0.projectLocation')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Select Button */}
          <Button
            onClick={handleSelectFolder}
            disabled={isSelecting}
            className="w-full justify-start gap-3"
            variant="outline"
            size="lg"
          >
            <FolderOpen className="h-5 w-5" />
            {isSelecting
              ? t('step0.openingFolderPicker')
              : projectPath
                ? t('step0.changeFolder')
                : t('step0.selectFolder')}
          </Button>

          {/* Selected Path Display */}
          {projectPath && (
            <div
              className={cn(
                'rounded-lg border p-4',
                error
                  ? 'border-semantic-error bg-semantic-error/5'
                  : 'border-brand bg-brand/5'
              )}
            >
              <div className="flex items-start gap-3">
                {error ? (
                  <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-semantic-error" />
                ) : (
                  <CheckCircle className="mt-0.5 h-5 w-5 shrink-0 text-brand" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="mb-1 flex items-center gap-2">
                    <span className="font-medium text-foreground">
                      {projectName}
                    </span>
                  </div>
                  <p className="break-all text-sm text-foreground-muted">
                    {projectPath}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Error Display */}
          {error && (
            <div className="rounded-lg border border-semantic-error bg-semantic-error/10 p-3">
              <p className="text-sm text-semantic-error">{error}</p>
            </div>
          )}

          {/* Help Text */}
          {!projectPath && (
            <p className="text-sm text-foreground-muted">
              {t('step0.helpText')}
            </p>
          )}
        </CardContent>
      </Card>

      {/* Checkpoint Dialog is handled by OnboardingWizardModal.tsx */}
    </div>
  )
}
