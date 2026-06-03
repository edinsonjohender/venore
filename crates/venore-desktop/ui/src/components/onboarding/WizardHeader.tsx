// =============================================================================
// WizardHeader - Header for wizard modal (v1 structure)
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Sparkles, X } from 'lucide-react'
import { StepIndicator, type Step } from './StepIndicator'
import { useWizardDataStore } from '@/stores/wizardDataStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface WizardHeaderProps {
  currentStep: number
  onClose?: () => void
}

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

// Step labels are resolved at render time via useTranslation
const WIZARD_STEP_KEYS = [
  'header.steps.context',
  'header.steps.rules',
  'header.steps.analysisIndex',
  'header.steps.indexResults',
  'header.steps.complete',
]

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function WizardHeader({ currentStep, onClose }: WizardHeaderProps) {
  const { t } = useTranslation('wizard')
  const projectName = useWizardDataStore((state) => state.step2.projectName)

  const wizardSteps: Step[] = WIZARD_STEP_KEYS.map((key, i) => ({
    num: i + 1,
    label: t(key),
  }))

  const stepCountText = t('header.step', { current: currentStep + 1, total: wizardSteps.length })

  return (
    <>
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-border">
        {/* Left: Icon + Title */}
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-9 h-9 rounded-lg flex items-center justify-center shrink-0 bg-brand/20">
            <Sparkles size={18} className="text-brand" />
          </div>
          <div className="min-w-0">
            <h2 className="text-base font-semibold text-foreground">
              {t('header.contextAgent')}
            </h2>
            <p className="text-xs text-foreground-muted truncate">
              {projectName || t('header.newCodebase')}
            </p>
          </div>
        </div>

        {/* Right: Step count + Close */}
        <div className="flex items-center gap-4 shrink-0">
          <span className="text-xs text-foreground-subtle">
            {stepCountText}
          </span>
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 rounded-md hover:bg-background-tertiary transition-colors"
            >
              <X size={16} className="text-foreground-muted" />
            </button>
          )}
        </div>
      </div>

      {/* Step Indicator */}
      <StepIndicator
        steps={wizardSteps}
        currentStep={currentStep + 1}
        variant="brand"
      />
    </>
  )
}
