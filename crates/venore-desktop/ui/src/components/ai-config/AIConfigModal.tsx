// =============================================================================
// AIConfigModal - Modal for AI provider configuration
// =============================================================================
// Modal wrapper for AI configuration, shown on first launch or when
// no AI provider is configured. Uses the shared AIConfigPanel component.

import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Settings } from 'lucide-react'
import { Modal } from '../ui/modal'
import { Button } from '../ui/button'
import { AIConfigPanel } from './AIConfigPanel'
import { tauriApi } from '../../lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface AIConfigModalProps {
  /** Is modal open */
  open: boolean
  /** Callback when modal should close */
  onOpenChange: (open: boolean) => void
  /** Whether configuration is required (blocks closing until configured) */
  isRequired?: boolean
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function AIConfigModal({ open, onOpenChange, isRequired = false }: AIConfigModalProps) {
  const { t } = useTranslation('workspace')
  const [hasConfigured, setHasConfigured] = useState(false)

  // Check if at least one provider is configured
  useEffect(() => {
    if (open) {
      tauriApi.getConfiguredProviders().then((res) => {
        setHasConfigured(res.providers.length > 0)
      }).catch((err) => {
        console.error('Failed to check provider status:', err)
      })
    }
  }, [open])

  const handleConfigChange = async () => {
    try {
      const res = await tauriApi.getConfiguredProviders()
      setHasConfigured(res.providers.length > 0)
    } catch (err) {
      console.error('Failed to check provider status:', err)
    }
  }

  const handleClose = () => {
    if (!isRequired || hasConfigured) {
      onOpenChange(false)
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      icon={<Settings className="w-4 h-4 text-foreground-muted" />}
      title={t('aiConfig.title')}
      description={
        isRequired
          ? t('aiConfig.requiredDescription')
          : t('aiConfig.manageProviders')
      }
      blockClose={isRequired}
      footer={
        <>
          {!isRequired && (
            <Button variant="ghost" onClick={handleClose}>
              {t('aiConfig.cancel')}
            </Button>
          )}
          <Button onClick={handleClose} disabled={isRequired && !hasConfigured}>
            {isRequired ? t('aiConfig.continue') : t('aiConfig.done')}
          </Button>
        </>
      }
    >
      <AIConfigPanel onConfigChange={handleConfigChange} />
    </Modal>
  )
}

// -----------------------------------------------------------------------------
// Export
// -----------------------------------------------------------------------------

export default AIConfigModal
