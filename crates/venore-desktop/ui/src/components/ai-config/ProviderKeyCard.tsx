// =============================================================================
// ProviderKeyCard - Individual provider configuration card
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Settings, Check, RefreshCw } from 'lucide-react'
import { Card } from '../ui/card'
import { Button } from '../ui/button'
import { tauriApi } from '../../lib/tauri'
import type { ProviderInfo } from './AIConfigPanel'

// Provider logos
import logoOpenAI from '../../assets/logo-openai.png'
import logoClaude from '../../assets/logo-claude.png'
import logoGemini from '../../assets/logo-gemini.png'
import logoOllama from '../../assets/logo-ollama.png'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ProviderKeyCardProps {
  provider: ProviderInfo
  isConfigured: boolean
  isEditing: boolean
  onEdit: () => void
  onRemove: () => void
  onTest?: () => Promise<void>
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function ProviderKeyCard({
  provider,
  isConfigured,
  isEditing,
  onEdit,
  onRemove,
  onTest,
}: ProviderKeyCardProps) {
  const { t } = useTranslation('workspace')
  const isOllama = provider.id === 'ollama'
  const [isTesting, setIsTesting] = useState(false)

  const handleTest = async () => {
    if (!onTest) return
    setIsTesting(true)
    try {
      await onTest()
    } finally {
      setIsTesting(false)
    }
  }

  return (
    <Card
      className={`
        relative p-3 transition-colors cursor-pointer
        ${isEditing ? 'border-brand bg-brand/5' : ''}
        ${isConfigured && !isEditing ? 'border-semantic-success/30 bg-semantic-success/5' : ''}
        ${!isConfigured && !isEditing ? 'hover:border-border-hover' : ''}
      `}
    >
      <div className="flex items-center gap-2">
        {/* Provider Icon */}
        <div className="w-8 h-8 rounded-md bg-background-tertiary flex items-center justify-center shrink-0">
          {getProviderIcon(provider.id)}
        </div>

        {/* Provider Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium text-foreground">{provider.name}</span>
            {isConfigured && <Check className="w-3.5 h-3.5 text-semantic-success" />}
          </div>
          <p className="text-xs text-foreground-muted truncate">{provider.description}</p>
        </div>

        {/* Action Button */}
        {isOllama ? (
          // Test button for Ollama
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={handleTest}
            disabled={isTesting}
            title={t('aiConfig.testOllamaConnection')}
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin' : ''}`} />
          </Button>
        ) : (
          // Settings button for cloud providers
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={isConfigured ? onRemove : onEdit}
            title={isConfigured ? t('aiConfig.remove') : t('aiConfig.configure')}
          >
            <Settings className="w-3.5 h-3.5" />
          </Button>
        )}
      </div>
    </Card>
  )
}

// -----------------------------------------------------------------------------
// Utilities
// -----------------------------------------------------------------------------

function getProviderIcon(providerId: string) {
  switch (providerId) {
    case 'openai':
      return <img src={logoOpenAI} alt="OpenAI" className="w-5 h-5 object-contain" />
    case 'anthropic':
      return <img src={logoClaude} alt="Claude" className="w-5 h-5 object-contain" />
    case 'gemini':
      return <img src={logoGemini} alt="Gemini" className="w-5 h-5 object-contain" />
    case 'ollama':
      return <img src={logoOllama} alt="Ollama" className="w-5 h-5 object-contain" />
    default:
      return <Settings className="w-5 h-5 text-foreground-muted" />
  }
}

// -----------------------------------------------------------------------------
// Export
// -----------------------------------------------------------------------------

export default ProviderKeyCard
