// =============================================================================
// AI Config Store - Preloaded AI configuration (providers, models, tasks)
// =============================================================================
// Loaded once at boot via getAIBootData(). AIConfigPanel reads from this store
// instead of making 6+ sequential API calls on every mount.

import { create } from 'zustand'
import { tauriApi } from '@/lib/tauri'
import type { AIProvider, AITask, TaskSettings } from '@/components/ai-config/AIConfigPanel'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface AIConfigState {
  // Data
  providerStatus: Record<AIProvider, boolean>
  taskSettings: Record<AITask, TaskSettings> | null
  availableModels: Record<AIProvider, string[]>
  loaded: boolean

  // Actions
  load: () => Promise<void>
  updateTaskSettings: (task: AITask, field: 'provider' | 'model', value: string) => void
  updateProviderStatus: (provider: AIProvider, status: boolean) => void
  setAvailableModels: (provider: AIProvider, models: string[]) => void
  reloadFromBackend: () => Promise<void>
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useAIConfigStore = create<AIConfigState>((set, get) => ({
  providerStatus: { openai: false, anthropic: false, gemini: false, ollama: false },
  taskSettings: null,
  availableModels: { openai: [], anthropic: [], gemini: [], ollama: [] },
  loaded: false,

  load: async () => {
    if (get().loaded) return
    try {
      const data = await tauriApi.getAIBootData()

      const configured = new Set(data.configured_providers)
      const providerStatus: Record<AIProvider, boolean> = {
        openai: configured.has('openai'),
        anthropic: configured.has('anthropic'),
        gemini: configured.has('gemini'),
        ollama: data.ollama_available,
      }

      const { onboarding, chat, analysis, embeddings } = data.task_settings
      const taskSettings: Record<AITask, TaskSettings> = {
        onboarding: { provider: onboarding.provider as AIProvider, model: onboarding.model },
        chat: { provider: chat.provider as AIProvider, model: chat.model },
        analysis: { provider: analysis.provider as AIProvider, model: analysis.model },
        embeddings: { provider: embeddings.provider as AIProvider, model: embeddings.model },
      }

      const availableModels: Record<AIProvider, string[]> = {
        openai: data.available_models['openai'] ?? [],
        anthropic: data.available_models['anthropic'] ?? [],
        gemini: data.available_models['gemini'] ?? [],
        ollama: data.available_models['ollama'] ?? [],
      }

      set({ providerStatus, taskSettings, availableModels, loaded: true })
    } catch (err) {
      console.error('Failed to load AI boot data:', err)
      // Mark as loaded anyway so the boot screen can proceed
      set({ loaded: true })
    }
  },

  updateTaskSettings: (task, field, value) => {
    const { taskSettings } = get()
    if (!taskSettings) return
    set({
      taskSettings: {
        ...taskSettings,
        [task]: { ...taskSettings[task], [field]: value },
      },
    })
  },

  updateProviderStatus: (provider, status) => {
    set({ providerStatus: { ...get().providerStatus, [provider]: status } })
  },

  setAvailableModels: (provider, models) => {
    set({ availableModels: { ...get().availableModels, [provider]: models } })
  },

  reloadFromBackend: async () => {
    // Force a fresh reload (used after saving API keys)
    set({ loaded: false })
    await get().load()
  },
}))
