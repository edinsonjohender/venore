// =============================================================================
// ModelSelector - Compact dropdown to switch chat model
// =============================================================================

import { ChevronDown } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useAIConfigStore } from '@/stores/aiConfigStore'
import { tauriApi } from '@/lib/tauri'

/** Shorten model IDs for display (e.g. "claude-sonnet-4-5" → "Sonnet 4.5") */
function shortenModelName(model: string): string {
  // Anthropic
  if (model.includes('sonnet')) return model.replace(/^claude-/, '').replace(/-/g, ' ').replace(/(\d) (\d)/g, '$1.$2')
  if (model.includes('haiku')) return model.replace(/^claude-/, '').replace(/-/g, ' ').replace(/(\d) (\d)/g, '$1.$2')
  if (model.includes('opus')) return model.replace(/^claude-/, '').replace(/-/g, ' ').replace(/(\d) (\d)/g, '$1.$2')
  // OpenAI
  if (model.startsWith('gpt-')) return model.replace('gpt-', 'GPT-')
  if (model.startsWith('o')) return model // o1, o3, etc
  // Gemini
  if (model.includes('gemini')) return model.replace('models/', '')
  // Fallback: return as-is, truncated
  return model.length > 20 ? model.slice(0, 18) + '...' : model
}

export function ModelSelector() {
  const { t } = useTranslation('chat')
  const taskSettings = useAIConfigStore((s) => s.taskSettings)
  const availableModels = useAIConfigStore((s) => s.availableModels)
  const updateTaskSettings = useAIConfigStore((s) => s.updateTaskSettings)

  const chatSettings = taskSettings?.chat
  if (!chatSettings) return null

  const currentProvider = chatSettings.provider
  const currentModel = chatSettings.model
  const models = availableModels[currentProvider] ?? []

  const handleSelectModel = async (model: string) => {
    updateTaskSettings('chat', 'model', model)
    try {
      await tauriApi.setTaskSettings({
        task: 'chat',
        provider: currentProvider,
        model,
      })
    } catch (err) {
      console.error('Failed to persist model change:', err)
    }
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 h-7 px-2 rounded-lg text-xs text-foreground-muted hover:text-foreground hover:bg-background-secondary transition-colors"
          title={t('input.model', 'Model')}
        >
          <span className="truncate max-w-[100px]">{shortenModelName(currentModel)}</span>
          <ChevronDown className="w-3 h-3 shrink-0" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-60 overflow-y-auto">
        {models.map((model) => (
          <DropdownMenuItem
            key={model}
            onClick={() => handleSelectModel(model)}
            className={model === currentModel ? 'bg-brand/10 text-brand' : ''}
          >
            {shortenModelName(model)}
          </DropdownMenuItem>
        ))}
        {models.length === 0 && (
          <DropdownMenuItem disabled>No models available</DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
