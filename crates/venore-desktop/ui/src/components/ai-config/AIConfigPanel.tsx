// =============================================================================
// AIConfigPanel - Reusable AI configuration panel
// =============================================================================
// Reusable component for AI provider and task configuration
// Used in: Settings > AI Providers tab, AIConfigModal (first time setup)
// Data is preloaded at boot into useAIConfigStore — opens instantly.

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ExternalLink, Eye, EyeOff, Settings } from 'lucide-react'
import { ProviderKeyCard } from './ProviderKeyCard'
import { TaskConfigRow } from './TaskConfigRow'
import { Input } from '../ui/input'
import { Button } from '../ui/button'
import { tauriApi } from '../../lib/tauri'
import { useAIConfigStore } from '../../stores/aiConfigStore'

// Provider logos
import logoOpenAI from '../../assets/logo-openai.png'
import logoClaude from '../../assets/logo-claude.png'
import logoGemini from '../../assets/logo-gemini.png'
import logoOllama from '../../assets/logo-ollama.png'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type AIProvider = 'openai' | 'anthropic' | 'gemini' | 'ollama'
export type AITask = 'onboarding' | 'chat' | 'analysis' | 'embeddings'

export interface TaskSettings {
  provider: AIProvider
  model: string
}

export interface ProviderInfo {
  id: AIProvider
  name: string
  description: string
  placeholder: string
  helpUrl: string
}

// -----------------------------------------------------------------------------
// Provider Data
// -----------------------------------------------------------------------------

export const PROVIDERS: ProviderInfo[] = [
  {
    id: 'openai',
    name: 'OpenAI',
    description: 'GPT-4.1, o3, o4-mini',
    placeholder: 'sk-...',
    helpUrl: 'https://platform.openai.com/api-keys',
  },
  {
    id: 'anthropic',
    name: 'Claude',
    description: 'Sonnet 4.5, Opus 4.6, Haiku 4.5',
    placeholder: 'sk-ant-...',
    helpUrl: 'https://console.anthropic.com/settings/keys',
  },
  {
    id: 'gemini',
    name: 'Gemini',
    description: '2.5 Flash, 2.5 Pro',
    placeholder: 'AIza...',
    helpUrl: 'https://aistudio.google.com/app/apikey',
  },
  {
    id: 'ollama',
    name: 'Ollama',
    description: 'Local models',
    placeholder: '',
    helpUrl: 'https://ollama.ai',
  },
]

export const TASK_LABELS: Record<AITask, { name: string; description: string }> = {
  onboarding: { name: 'Onboarding', description: 'Context generation' },
  chat: { name: 'Chat', description: 'Veronica assistant' },
  analysis: { name: 'Analysis', description: 'Code analysis' },
  embeddings: { name: 'Embeddings', description: 'RAG vector search' },
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

interface AIConfigPanelProps {
  /** Callback when configuration changes */
  onConfigChange?: () => void
}

export function AIConfigPanel({ onConfigChange }: AIConfigPanelProps) {
  const { t } = useTranslation('workspace')

  // Read from preloaded store (loaded at boot)
  const providerStatus = useAIConfigStore(s => s.providerStatus)
  const taskSettings = useAIConfigStore(s => s.taskSettings)
  const availableModels = useAIConfigStore(s => s.availableModels)
  const updateTaskSettings = useAIConfigStore(s => s.updateTaskSettings)
  const updateProviderStatus = useAIConfigStore(s => s.updateProviderStatus)
  const reloadFromBackend = useAIConfigStore(s => s.reloadFromBackend)

  // Local UI state
  const [editingProvider, setEditingProvider] = useState<AIProvider | null>(null)
  const [apiKeyInput, setApiKeyInput] = useState('')
  const [showApiKey, setShowApiKey] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [testResult, setTestResult] = useState<{ success: boolean; error?: string } | null>(null)

  // Handle API key save
  const handleSaveApiKey = async () => {
    if (!editingProvider || !apiKeyInput.trim()) return

    setIsSaving(true)
    setTestResult(null)

    try {
      // Store the key
      await tauriApi.setApiKey({
        provider: editingProvider,
        api_key: apiKeyInput.trim(),
      })

      // Test connection
      const result = await tauriApi.testConnection({
        provider: editingProvider,
      })

      if (result.success) {
        setTestResult({ success: true })
        // Reload store from backend
        await reloadFromBackend()
        // Close editor after short delay
        setTimeout(() => {
          setEditingProvider(null)
          setApiKeyInput('')
          setShowApiKey(false)
          setTestResult(null)
          onConfigChange?.()
        }, 1000)
      } else {
        setTestResult({ success: false, error: result.error || t('aiConfig.connectionFailed') })
      }
    } catch (err) {
      setTestResult({ success: false, error: (err as Error).message })
    } finally {
      setIsSaving(false)
    }
  }

  // Handle task settings change
  const handleTaskSettingChange = async (
    task: AITask,
    field: 'provider' | 'model',
    value: string
  ) => {
    if (!taskSettings) return

    const currentSettings = taskSettings[task]
    const newSettings: TaskSettings = {
      ...currentSettings,
      [field]: value,
    }

    // If changing provider, reset model to first available
    if (field === 'provider') {
      const models = availableModels[value as AIProvider]
      newSettings.model = models[0] || ''
    }

    try {
      // Save task settings to backend
      await tauriApi.setTaskSettings({
        task,
        provider: newSettings.provider,
        model: newSettings.model,
      })

      // Update store
      updateTaskSettings(task, 'provider', newSettings.provider)
      updateTaskSettings(task, 'model', newSettings.model)
      onConfigChange?.()
    } catch (err) {
      console.error('Failed to save task settings:', err)
    }
  }

  // Handle Ollama test
  const handleOllamaTest = async () => {
    try {
      const result = await tauriApi.testConnection({ provider: 'ollama' })
      updateProviderStatus('ollama', result.success)
    } catch (err) {
      console.error('Ollama test failed:', err)
      updateProviderStatus('ollama', false)
    }
  }

  const configuredCount = Object.values(providerStatus).filter(Boolean).length

  return (
    <div className="space-y-6">
      {/* API Keys Section */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-medium text-foreground">{t('aiConfig.apiKeys')}</h3>
            <p className="text-xs text-foreground-muted mt-0.5">
              {configuredCount > 0
                ? t('aiConfig.providersConfigured', { count: configuredCount })
                : t('aiConfig.connectOneProvider')}
            </p>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          {PROVIDERS.map((provider) => (
            <ProviderKeyCard
              key={provider.id}
              provider={provider}
              isConfigured={providerStatus[provider.id]}
              isEditing={editingProvider === provider.id}
              onEdit={() => {
                setEditingProvider(provider.id)
                setApiKeyInput('')
                setShowApiKey(false)
                setTestResult(null)
              }}
              onRemove={async () => {
                try {
                  await tauriApi.removeApiKey(provider.id)
                  await reloadFromBackend()
                  onConfigChange?.()
                } catch (err) {
                  console.error('Failed to remove API key:', err)
                }
              }}
              onTest={provider.id === 'ollama' ? handleOllamaTest : undefined}
            />
          ))}
        </div>

        {/* API Key Editor */}
        {editingProvider && (
          <div className="mt-3 p-3 bg-background-secondary border border-border rounded-lg">
            {/* Header */}
            <div className="flex items-center gap-2 mb-3">
              {getProviderIcon(editingProvider)}
              <span className="text-sm font-medium text-foreground">
                {t('aiConfig.apiKeyLabel', { name: PROVIDERS.find((p) => p.id === editingProvider)?.name })}
              </span>
            </div>

            {/* Input */}
            <div className="relative">
              <Input
                type={showApiKey ? 'text' : 'password'}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder={PROVIDERS.find((p) => p.id === editingProvider)?.placeholder}
                className="pr-10 font-mono text-sm"
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowApiKey(!showApiKey)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-foreground-muted hover:text-foreground transition-colors"
              >
                {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between mt-3">
              {/* Help Link */}
              <a
                href={PROVIDERS.find((p) => p.id === editingProvider)?.helpUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-xs text-foreground-muted hover:text-foreground transition-colors"
              >
                <ExternalLink className="w-3 h-3" />
                {t('aiConfig.getApiKey')}
              </a>

              {/* Actions */}
              <div className="flex items-center gap-2">
                {/* Test Result */}
                {testResult && (
                  <span
                    className={`text-xs ${
                      testResult.success ? 'text-semantic-success' : 'text-semantic-error'
                    }`}
                  >
                    {testResult.success ? t('aiConfig.connected') : testResult.error}
                  </span>
                )}

                {/* Cancel */}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setEditingProvider(null)
                    setApiKeyInput('')
                    setShowApiKey(false)
                    setTestResult(null)
                  }}
                >
                  {t('aiConfig.cancel')}
                </Button>

                {/* Save & Test */}
                <Button
                  size="sm"
                  onClick={handleSaveApiKey}
                  disabled={!apiKeyInput.trim() || isSaving}
                >
                  {isSaving ? t('aiConfig.testing') : t('aiConfig.saveAndTest')}
                </Button>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Task Configuration Section */}
      <div>
        <div className="mb-3">
          <h3 className="text-sm font-medium text-foreground">{t('aiConfig.taskConfiguration')}</h3>
          <p className="text-xs text-foreground-muted mt-0.5">
            {t('aiConfig.chooseProviderModel')}
          </p>
        </div>

        <div className="space-y-2">
          {taskSettings &&
            (['onboarding', 'chat', 'analysis', 'embeddings'] as AITask[]).map((task) => (
              <TaskConfigRow
                key={task}
                task={task}
                settings={taskSettings[task]}
                providerStatus={providerStatus}
                availableModels={availableModels}
                onChange={(field, value) => handleTaskSettingChange(task, field, value)}
              />
            ))}
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Utilities
// -----------------------------------------------------------------------------

function getProviderIcon(providerId: string) {
  switch (providerId) {
    case 'openai':
      return <img src={logoOpenAI} alt="OpenAI" className="w-4 h-4 object-contain" />
    case 'anthropic':
      return <img src={logoClaude} alt="Claude" className="w-4 h-4 object-contain" />
    case 'gemini':
      return <img src={logoGemini} alt="Gemini" className="w-4 h-4 object-contain" />
    case 'ollama':
      return <img src={logoOllama} alt="Ollama" className="w-4 h-4 object-contain" />
    default:
      return <Settings className="w-4 h-4 text-foreground-muted" />
  }
}

// -----------------------------------------------------------------------------
// Export
// -----------------------------------------------------------------------------

export default AIConfigPanel
