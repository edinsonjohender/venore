// =============================================================================
// WizardFooter - Footer with navigation buttons
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useWizardNavigation } from '@/hooks/useWizardNavigation'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface WizardFooterProps {
  onCancel?: () => void
  customNextButton?: React.ReactNode
  hideNavigation?: boolean
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function WizardFooter({
  onCancel,
  customNextButton,
  hideNavigation = false,
}: WizardFooterProps) {
  const { t } = useTranslation('wizard')
  const { canGoNext, canGoBack, goNext, goBack, currentStep } =
    useWizardNavigation()

  if (hideNavigation) {
    return null
  }

  return (
    <div className="flex items-center justify-between border-t border-border bg-background-secondary px-6 py-4">
      {/* Left Side - Cancel/Back */}
      <div>
        {currentStep === 0 ? (
          <Button variant="ghost" onClick={onCancel}>
            {t('footer.cancel')}
          </Button>
        ) : (
          <Button
            variant="ghost"
            onClick={goBack}
            disabled={!canGoBack}
          >
            <ChevronLeft className="mr-2 h-4 w-4" />
            {t('footer.back')}
          </Button>
        )}
      </div>

      {/* Right Side - Next/Custom */}
      <div>
        {customNextButton || (
          <Button onClick={goNext} disabled={!canGoNext}>
            {t('footer.next')}
            <ChevronRight className="ml-2 h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  )
}
