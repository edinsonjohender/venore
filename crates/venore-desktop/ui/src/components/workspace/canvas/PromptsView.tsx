// =============================================================================
// PromptsView — Task-based navigation + provider inner tabs
// =============================================================================
// Left sidebar lists tasks (Chat, Context, GitHub). Selecting a task shows
// provider tabs (Base | Claude | OpenAI | Gemini | Ollama) with a Monaco editor.

import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import Editor, { type OnMount } from '@monaco-editor/react'
import { Loader2, RotateCcw, Save, History, Plus } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  tauriApi,
  type PromptDto,
  type PromptVersionDto,
} from '@/lib/tauri'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'

import logoOpenAI from '@/assets/logo-openai.png'
import logoClaude from '@/assets/logo-claude.png'
import logoGemini from '@/assets/logo-gemini.png'
import logoOllama from '@/assets/logo-ollama.png'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const TASK_LABEL_KEYS: Record<string, string> = {
  chat: 'prompts.tasks.chat',
  context: 'prompts.tasks.context',
  github: 'prompts.tasks.github',
}

const PROVIDER_TABS = [
  { id: 'base', labelKey: 'base', logo: null },
  { id: 'anthropic', labelKey: 'Claude', logo: logoClaude },
  { id: 'openai', labelKey: 'OpenAI', logo: logoOpenAI },
  { id: 'gemini', labelKey: 'Gemini', logo: logoGemini },
  { id: 'ollama', labelKey: 'Ollama', logo: logoOllama },
] as const

// -----------------------------------------------------------------------------
// PromptsView (main)
// -----------------------------------------------------------------------------

export function PromptsView() {
  const { t } = useTranslation('workspace')
  const [tasks, setTasks] = useState<string[]>([])
  const [selectedTask, setSelectedTask] = useState<string | null>(null)
  const [taskPrompts, setTaskPrompts] = useState<PromptDto[]>([])
  const [activeProvider, setActiveProvider] = useState('base')
  const [editorContent, setEditorContent] = useState('')
  const [originalContent, setOriginalContent] = useState('')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [versions, setVersions] = useState<PromptVersionDto[]>([])
  const [showVersions, setShowVersions] = useState(false)
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)

  // The prompt for the active provider tab
  const activePrompt = taskPrompts.find((p) => p.provider === activeProvider)
  const basePrompt = taskPrompts.find((p) => p.provider === 'base')

  // Helper to get task label
  const getTaskLabel = useCallback((task: string) => {
    const key = TASK_LABEL_KEYS[task]
    return key ? t(key) : task
  }, [t])

  // Load tasks on mount
  useEffect(() => {
    loadTasks()
  }, [])

  const loadTasks = useCallback(async () => {
    try {
      setLoading(true)
      const result = await tauriApi.listPromptTasks()
      setTasks(result)
      if (result.length > 0) {
        selectTask(result[0])
      }
    } catch (e) {
      console.error('Failed to load tasks:', e)
    } finally {
      setLoading(false)
    }
  }, [])

  const selectTask = useCallback(async (task: string) => {
    setSelectedTask(task)
    setActiveProvider('base')
    setShowVersions(false)
    try {
      const prompts = await tauriApi.getTaskPrompts(task)
      setTaskPrompts(prompts)
      // Load base prompt content
      const base = prompts.find((p) => p.provider === 'base')
      if (base) {
        setEditorContent(base.content)
        setOriginalContent(base.content)
      }
    } catch (e) {
      console.error('Failed to load task prompts:', e)
    }
  }, [])

  const handleProviderChange = useCallback(
    (provider: string) => {
      setActiveProvider(provider)
      setShowVersions(false)
      const prompt = taskPrompts.find((p) => p.provider === provider)
      if (prompt) {
        setEditorContent(prompt.content)
        setOriginalContent(prompt.content)
      } else {
        // No override exists — show empty (placeholder will render instead)
        setEditorContent('')
        setOriginalContent('')
      }
    },
    [taskPrompts],
  )

  const handleSave = useCallback(async () => {
    if (!activePrompt || editorContent === originalContent) return
    try {
      setSaving(true)
      const updated = await tauriApi.updatePrompt({
        id: activePrompt.id,
        content: editorContent,
      })
      setOriginalContent(updated.content)
      setEditorContent(updated.content)
      setTaskPrompts((prev) =>
        prev.map((p) => (p.id === updated.id ? updated : p)),
      )
    } catch (e) {
      console.error('Failed to save prompt:', e)
    } finally {
      setSaving(false)
    }
  }, [activePrompt, editorContent, originalContent])

  const handleReset = useCallback(async () => {
    if (!activePrompt) return
    try {
      setSaving(true)
      const reset = await tauriApi.resetPrompt(activePrompt.id)
      setEditorContent(reset.content)
      setOriginalContent(reset.content)
      setTaskPrompts((prev) =>
        prev.map((p) => (p.id === reset.id ? reset : p)),
      )
    } catch (e) {
      console.error('Failed to reset prompt:', e)
    } finally {
      setSaving(false)
    }
  }, [activePrompt])

  const handleCustomize = useCallback(async () => {
    if (!selectedTask || !basePrompt) return
    try {
      setSaving(true)
      const created = await tauriApi.saveTaskPrompt({
        category: selectedTask,
        provider: activeProvider,
        content: basePrompt.content,
      })
      setTaskPrompts((prev) => [...prev, created])
      setEditorContent(created.content)
      setOriginalContent(created.content)
    } catch (e) {
      console.error('Failed to create override:', e)
    } finally {
      setSaving(false)
    }
  }, [selectedTask, activeProvider, basePrompt])

  const handleShowVersions = useCallback(async () => {
    if (!activePrompt) return
    if (showVersions) {
      setShowVersions(false)
      return
    }
    try {
      const result = await tauriApi.listPromptVersions(activePrompt.id)
      setVersions(result)
      setShowVersions(true)
    } catch (e) {
      console.error('Failed to load versions:', e)
    }
  }, [activePrompt, showVersions])

  const handleEditorMount: OnMount = useCallback(
    (editor, monaco) => {
      editorRef.current = editor
      // eslint-disable-next-line no-bitwise
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => handleSave())
    },
    [handleSave],
  )

  const isDirty = editorContent !== originalContent

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="w-5 h-5 animate-spin text-foreground-subtle" />
      </div>
    )
  }

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Left sidebar — task list */}
      <div className="w-[220px] shrink-0 flex flex-col border-r border-border bg-background-secondary overflow-hidden">
        <div className="px-3 py-2.5 border-b border-border/50">
          <span className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted">
            {t('prompts.tasksLabel')}
          </span>
        </div>
        <div className="flex-1 overflow-y-auto">
          {tasks.map((task) => (
            <button
              key={task}
              onClick={() => selectTask(task)}
              className={cn(
                'w-full text-left px-3 py-2.5 border-b border-border/30 transition-colors text-sm',
                selectedTask === task
                  ? 'bg-background-tertiary border-l-2 border-l-brand font-medium text-foreground'
                  : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent text-foreground-muted',
              )}
            >
              {getTaskLabel(task)}
            </button>
          ))}
        </div>
      </div>

      {/* Right panel — provider tabs + editor */}
      {selectedTask ? (
        <div className="flex-1 flex flex-col overflow-hidden">
          <Tabs
            value={activeProvider}
            onValueChange={handleProviderChange}
            className="flex-1 flex flex-col min-h-0"
          >
            {/* Provider tab bar */}
            <TabsList>
              {PROVIDER_TABS.map((tab) => (
                <TabsTrigger key={tab.id} value={tab.id} className="gap-1.5">
                  {tab.logo && (
                    <img
                      src={tab.logo}
                      alt={tab.labelKey}
                      className="w-4 h-4 rounded-sm"
                    />
                  )}
                  {tab.labelKey}
                </TabsTrigger>
              ))}
            </TabsList>

            {/* Tab content area */}
            <div className="flex-1 relative min-h-0">
              {PROVIDER_TABS.map((tab) => (
                <TabsContent
                  key={tab.id}
                  value={tab.id}
                  className="absolute inset-0 flex flex-col data-[state=inactive]:hidden"
                >
                  <ProviderTabContent
                    provider={tab.id}
                    providerLabel={tab.labelKey}
                    prompt={taskPrompts.find((p) => p.provider === tab.id) ?? null}
                    taskLabel={getTaskLabel(selectedTask)}
                    editorContent={editorContent}
                    originalContent={originalContent}
                    isDirty={isDirty}
                    saving={saving}
                    showVersions={showVersions}
                    versions={versions}
                    onEditorChange={setEditorContent}
                    onSave={handleSave}
                    onReset={handleReset}
                    onCustomize={handleCustomize}
                    onToggleVersions={handleShowVersions}
                    onEditorMount={handleEditorMount}
                  />
                </TabsContent>
              ))}
            </div>
          </Tabs>
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-foreground-subtle text-sm">
          {t('prompts.selectTask')}
        </div>
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// ProviderTabContent
// -----------------------------------------------------------------------------

function ProviderTabContent({
  provider,
  providerLabel,
  prompt,
  taskLabel,
  editorContent,
  originalContent,
  isDirty,
  saving,
  showVersions,
  versions,
  onEditorChange,
  onSave,
  onReset,
  onCustomize,
  onToggleVersions,
  onEditorMount,
}: {
  provider: string
  providerLabel: string
  prompt: PromptDto | null
  taskLabel: string
  editorContent: string
  originalContent: string
  isDirty: boolean
  saving: boolean
  showVersions: boolean
  versions: PromptVersionDto[]
  onEditorChange: (value: string) => void
  onSave: () => void
  onReset: () => void
  onCustomize: () => void
  onToggleVersions: () => void
  onEditorMount: OnMount
}) {
  const { t } = useTranslation('workspace')

  // No override exists for this provider
  if (!prompt) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-4 text-foreground-subtle">
        <p className="text-sm">
          {t('prompts.usingBasePrompt')}{' '}
          <span className="font-medium text-foreground">{providerLabel}</span>.
        </p>
        <button
          onClick={onCustomize}
          disabled={saving}
          className={cn(
            'flex items-center gap-2 px-4 py-2 text-sm rounded-md transition-colors',
            'bg-brand text-brand-foreground hover:bg-brand/90',
          )}
        >
          {saving ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Plus className="w-4 h-4" />
          )}
          {t('prompts.customizeFor', { provider: providerLabel })}
        </button>
      </div>
    )
  }

  // Has prompt — show action bar + editor
  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Action bar */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-background-secondary shrink-0">
        <span className="text-sm font-medium text-foreground truncate">
          {taskLabel} — {providerLabel}
        </span>
        <span className="text-[10px] text-foreground-subtle">{t('prompts.version', { version: prompt.version })}</span>

        <div className="flex-1" />

        {/* Version history */}
        <button
          className={cn(
            'flex items-center gap-1 px-2 py-1 text-xs rounded-md transition-colors',
            'text-foreground-subtle hover:text-foreground hover:bg-foreground/5',
            showVersions && 'bg-foreground/10 text-foreground',
          )}
          onClick={onToggleVersions}
          title={t('prompts.versionHistory')}
        >
          <History className="w-3.5 h-3.5" />
        </button>

        {/* Reset button */}
        <button
          className={cn(
            'flex items-center gap-1 px-2 py-1 text-xs rounded-md transition-colors',
            'text-foreground-subtle hover:text-foreground hover:bg-foreground/5',
            prompt.version <= 1 && 'opacity-40 pointer-events-none',
          )}
          onClick={onReset}
          disabled={prompt.version <= 1 || saving}
          title={t('prompts.resetToDefault')}
        >
          <RotateCcw className="w-3.5 h-3.5" />
          <span>{t('prompts.reset')}</span>
        </button>

        {/* Save button */}
        <button
          className={cn(
            'flex items-center gap-1 px-3 py-1 text-xs rounded-md transition-colors',
            isDirty
              ? 'bg-brand text-brand-foreground hover:bg-brand/90'
              : 'bg-foreground/5 text-foreground-subtle pointer-events-none',
          )}
          onClick={onSave}
          disabled={!isDirty || saving}
        >
          {saving ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <Save className="w-3.5 h-3.5" />
          )}
          <span>{t('prompts.save')}</span>
        </button>
      </div>

      {/* Editor or version history */}
      <div className="flex-1 overflow-hidden">
        {showVersions ? (
          <div className="h-full overflow-y-auto p-4 space-y-3">
            <div className="text-xs text-foreground-subtle mb-2">
              {t('prompts.versionHistoryFor', { name: prompt.name })}
            </div>
            {versions.length === 0 ? (
              <div className="text-xs text-foreground-subtle">
                {t('prompts.noVersionHistory')}
              </div>
            ) : (
              versions.map((v) => (
                <div
                  key={v.id}
                  className="border border-border rounded-lg p-3 bg-background-secondary"
                >
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-xs font-medium">{t('prompts.version', { version: v.version })}</span>
                    <span className="text-[10px] text-foreground-subtle">
                      {new Date(v.createdAt).toLocaleString()}
                    </span>
                  </div>
                  <pre className="text-[11px] text-foreground-subtle whitespace-pre-wrap max-h-40 overflow-y-auto font-mono">
                    {v.content.slice(0, 500)}
                    {v.content.length > 500 && '...'}
                  </pre>
                </div>
              ))
            )}
          </div>
        ) : (
          <Editor
            defaultLanguage="markdown"
            value={editorContent}
            onChange={(value) => onEditorChange(value ?? '')}
            onMount={onEditorMount}
            theme="vs-dark"
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              lineNumbers: 'on',
              wordWrap: 'on',
              scrollBeyondLastLine: false,
              padding: { top: 12 },
              renderWhitespace: 'selection',
            }}
          />
        )}
      </div>
    </div>
  )
}
