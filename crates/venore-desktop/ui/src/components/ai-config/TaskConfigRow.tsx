// =============================================================================
// TaskConfigRow - Task configuration row (provider + model selection)
// =============================================================================

import { useTranslation } from 'react-i18next'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../ui/select'
import type { AIProvider, AITask, TaskSettings } from './AIConfigPanel'
import { PROVIDERS, TASK_LABELS } from './AIConfigPanel'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface TaskConfigRowProps {
  task: AITask
  settings: TaskSettings
  providerStatus: Record<AIProvider, boolean>
  availableModels: Record<AIProvider, string[]>
  onChange: (field: 'provider' | 'model', value: string) => void
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function TaskConfigRow({
  task,
  settings,
  providerStatus,
  availableModels,
  onChange,
}: TaskConfigRowProps) {
  const { t } = useTranslation('workspace')
  const taskInfo = TASK_LABELS[task]
  const configuredProviders = (Object.keys(providerStatus) as AIProvider[]).filter(
    (p) => providerStatus[p]
  )
  const models = availableModels[settings.provider] || []

  return (
    <div className="flex items-center gap-3 p-3 bg-background-secondary rounded-lg">
      {/* Task name */}
      <div className="w-28 shrink-0">
        <span className="text-sm font-medium text-foreground">{taskInfo.name}</span>
        <p className="text-xs text-foreground-muted">{taskInfo.description}</p>
      </div>

      {/* Provider select */}
      <div className="flex-1">
        <Select
          value={settings.provider}
          onValueChange={(value) => onChange('provider', value)}
          disabled={configuredProviders.length === 0}
        >
          <SelectTrigger className="h-9">
            <SelectValue placeholder={t('aiConfig.selectProvider')} />
          </SelectTrigger>
          <SelectContent>
            {configuredProviders.length === 0 ? (
              <SelectItem value="none" disabled>
                {t('aiConfig.noProviders')}
              </SelectItem>
            ) : (
              configuredProviders.map((p) => (
                <SelectItem key={p} value={p}>
                  {PROVIDERS.find((pr) => pr.id === p)?.name || p}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>
      </div>

      {/* Model select */}
      <div className="flex-1">
        <Select
          value={settings.model}
          onValueChange={(value) => onChange('model', value)}
          disabled={models.length === 0}
        >
          <SelectTrigger className="h-9 font-mono text-xs">
            <SelectValue placeholder={t('aiConfig.selectModel')} />
          </SelectTrigger>
          <SelectContent>
            {models.length === 0 ? (
              <SelectItem value="none" disabled>
                {t('aiConfig.noModels')}
              </SelectItem>
            ) : (
              models.map((m) => (
                <SelectItem key={m} value={m} className="font-mono text-xs">
                  {m}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Export
// -----------------------------------------------------------------------------

export default TaskConfigRow
