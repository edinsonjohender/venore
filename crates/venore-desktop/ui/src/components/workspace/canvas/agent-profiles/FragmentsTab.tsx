// =============================================================================
// FragmentsTab — Chat-fragment prompts (editable blocks of the system prompt)
// =============================================================================
// Each fragment is a row in the `prompts` table under category `chat-fragment`.
// The chat builder pulls them at request time and renders {{var}} placeholders
// with values gathered in Rust. Toggle is_enabled to drop a block entirely
// without losing its content.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Loader2, AlertCircle, Save, RotateCcw, Eye, EyeOff } from 'lucide-react'

import { cn } from '@/lib/utils'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { tauriApi, type PromptDto } from '@/lib/tauri'

function FragmentRow({
  fragment,
  isSelected,
  onSelect,
  onToggle,
}: {
  fragment: PromptDto
  isSelected: boolean
  onSelect: () => void
  onToggle: () => void
}) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 px-3 py-2 border-b border-border/30 transition-colors cursor-pointer',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
        !fragment.isEnabled && 'opacity-50',
      )}
      onClick={onSelect}
    >
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onToggle()
        }}
        title={fragment.isEnabled ? 'Disable block' : 'Enable block'}
        className="p-1 rounded hover:bg-foreground/10"
      >
        {fragment.isEnabled ? (
          <Eye className="w-3 h-3 text-emerald-400" />
        ) : (
          <EyeOff className="w-3 h-3 text-foreground-muted/60" />
        )}
      </button>
      <div className="flex-1 min-w-0">
        <div className="text-xs font-medium text-foreground truncate">{fragment.name}</div>
        <div className="text-[10px] text-foreground-muted/60 truncate font-mono">{fragment.id}</div>
      </div>
    </div>
  )
}

export function FragmentsTab() {
  const [fragments, setFragments] = useState<PromptDto[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draftContent, setDraftContent] = useState<string>('')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const list = await tauriApi.listChatFragments()
      setFragments(list)
      if (!selectedId && list.length > 0) {
        setSelectedId(list[0].id)
        setDraftContent(list[0].content)
      } else if (selectedId) {
        const cur = list.find((p) => p.id === selectedId)
        if (cur) setDraftContent(cur.content)
      }
    } catch (err) {
      console.error('Failed to load chat fragments:', err)
      setError(typeof err === 'string' ? err : 'No se pudo cargar')
    } finally {
      setLoading(false)
    }
  }, [selectedId])

  useEffect(() => {
    load()
  }, [load])

  const selected = useMemo(
    () => fragments.find((p) => p.id === selectedId) ?? null,
    [fragments, selectedId],
  )

  const isDirty = selected !== null && draftContent !== selected.content

  const handleSelect = (id: string) => {
    const f = fragments.find((p) => p.id === id)
    if (!f) return
    setSelectedId(id)
    setDraftContent(f.content)
  }

  const handleToggle = async (id: string) => {
    const f = fragments.find((p) => p.id === id)
    if (!f) return
    try {
      const updated = await tauriApi.setPromptEnabled({ id, enabled: !f.isEnabled })
      setFragments((arr) => arr.map((p) => (p.id === id ? updated : p)))
    } catch (err) {
      console.error('Toggle failed:', err)
      setError(typeof err === 'string' ? err : 'Toggle failed')
    }
  }

  const handleSave = async () => {
    if (!selected || saving || !isDirty) return
    setSaving(true)
    setError(null)
    try {
      const updated = await tauriApi.updatePrompt({ id: selected.id, content: draftContent })
      setFragments((arr) => arr.map((p) => (p.id === updated.id ? updated : p)))
      setDraftContent(updated.content)
    } catch (err) {
      console.error('Save failed:', err)
      setError(typeof err === 'string' ? err : 'Save failed')
    } finally {
      setSaving(false)
    }
  }

  const handleReset = async () => {
    if (!selected) return
    try {
      const reset = await tauriApi.resetPrompt(selected.id)
      setFragments((arr) => arr.map((p) => (p.id === reset.id ? reset : p)))
      setDraftContent(reset.content)
    } catch (err) {
      console.error('Reset failed:', err)
      setError(typeof err === 'string' ? err : 'Reset failed')
    }
  }

  const variables = useMemo(() => {
    if (!selected || selected.variables.length === 0) return []
    return selected.variables
  }, [selected])

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="w-4 h-4 text-foreground-muted/50 animate-spin" />
      </div>
    )
  }

  return (
    <div className="flex-1 flex min-h-0">
      <aside className="w-72 shrink-0 border-r border-border flex flex-col">
        <header className="px-3 py-2 border-b border-border">
          <span className="text-[10px] uppercase tracking-wider text-foreground-muted">
            Fragments ({fragments.length})
          </span>
          <p className="text-[10px] text-foreground-muted/60 mt-1">
            Bloques editables del system prompt
          </p>
        </header>
        <div className="flex-1 overflow-y-auto">
          {fragments.map((f) => (
            <FragmentRow
              key={f.id}
              fragment={f}
              isSelected={f.id === selectedId}
              onSelect={() => handleSelect(f.id)}
              onToggle={() => handleToggle(f.id)}
            />
          ))}
        </div>
      </aside>

      <main className="flex-1 min-w-0 overflow-y-auto">
        {!selected ? (
          <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/60 h-full">
            Selecciona un fragmento
          </div>
        ) : (
          <div className="px-6 py-4 space-y-4 max-w-4xl">
            {error && (
              <div className="flex items-center gap-2 px-3 py-2 rounded border border-red-500/40 bg-red-500/10 text-red-300 text-xs">
                <AlertCircle className="w-3 h-3" />
                {error}
              </div>
            )}

            <header className="flex items-center justify-between gap-3">
              <div className="min-w-0 flex-1">
                <h2 className="text-sm font-medium text-foreground truncate">{selected.name}</h2>
                <p className="text-[10px] text-foreground-muted/60 font-mono mt-0.5">{selected.id}</p>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-foreground-muted/60">v{selected.version}</span>
                <button
                  type="button"
                  onClick={handleReset}
                  title="Reset to original"
                  className="p-1.5 rounded hover:bg-foreground/5 text-foreground-muted hover:text-foreground"
                >
                  <RotateCcw className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={handleSave}
                  disabled={!isDirty || saving}
                  className="text-xs px-3 py-1 rounded bg-emerald-500/90 hover:bg-emerald-500 text-white disabled:opacity-50 flex items-center gap-1"
                >
                  <Save className="w-3 h-3" />
                  {saving ? 'Guardando...' : 'Guardar'}
                </button>
              </div>
            </header>

            {variables.length > 0 && (
              <div>
                <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                  Variables disponibles
                </Label>
                <div className="flex flex-wrap gap-1 mt-1">
                  {variables.map((v) => (
                    <code
                      key={v}
                      className="text-[10px] px-1.5 py-0.5 rounded bg-foreground/5 border border-border text-foreground-muted font-mono"
                    >
                      {`{{${v}}}`}
                    </code>
                  ))}
                </div>
              </div>
            )}

            <div>
              <Label className="text-[10px] uppercase tracking-wider text-foreground-muted">
                Content
              </Label>
              <Textarea
                value={draftContent}
                onChange={(e) => setDraftContent(e.target.value)}
                rows={20}
                className="mt-1 font-mono text-xs"
                placeholder="Fragment content"
              />
            </div>

            {!selected.isEnabled && (
              <div className="text-[10px] text-amber-400/80 flex items-center gap-1">
                <EyeOff className="w-3 h-3" />
                This block is disabled — not included in the current prompt.
              </div>
            )}
          </div>
        )}
      </main>
    </div>
  )
}
