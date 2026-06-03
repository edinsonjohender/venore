// =============================================================================
// ChatContextSelector - Popover for selecting module context for chat
// =============================================================================
// Triggered by the @ button in ChatInput. Shows available modules with
// .context.md files that can be included as context for the LLM.

import { useEffect, useState, useRef, useMemo } from 'react'
import { X, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { tauriApi } from '@/lib/tauri'
import type { ChatContextOptionDto } from '@/lib/tauri'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'
import { Badge } from '@/components/ui/badge'

interface ChatContextSelectorProps {
  projectPath: string | null
  selectedModules: Array<{ name: string; path: string }>
  onToggleModule: (module: { name: string; path: string }) => void
  onClose: () => void
}

export function ChatContextSelector({
  projectPath,
  selectedModules,
  onToggleModule,
  onClose,
}: ChatContextSelectorProps) {
  const { t } = useTranslation('chat')
  const [modules, setModules] = useState<ChatContextOptionDto[]>([])
  const [loading, setLoading] = useState(false)
  const panelRef = useRef<HTMLDivElement>(null)
  const connections = useAIConnectionStore((s) => s.connections)
  const panels = useNodeFloatingStore((s) => s.panels)

  // Paths of modules connected via Sparkles — these are checked and disabled
  const connectedPaths = useMemo(() => {
    const paths = new Set<string>()
    for (const p of panels) {
      if (connections[p.panelId]?.active) paths.add(p.data.modulePath)
    }
    return paths
  }, [panels, connections])

  // Load available modules
  useEffect(() => {
    if (!projectPath) return

    setLoading(true)
    tauriApi
      .getChatContextOptions(projectPath)
      .then(setModules)
      .catch((err) => console.error('Failed to load context options:', err))
      .finally(() => setLoading(false))
  }, [projectPath])

  // Close on click outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  const isSelected = (path: string) => selectedModules.some((m) => m.path === path)

  if (!projectPath) {
    return (
      <div
        ref={panelRef}
        className="absolute bottom-full left-0 right-0 mb-1 z-50 bg-background border border-border rounded-lg shadow-lg p-3"
      >
        <p className="text-xs text-foreground-muted text-center">
          {t('contextSelector.noProject')}
        </p>
      </div>
    )
  }

  return (
    <div
      ref={panelRef}
      className="absolute bottom-full left-0 right-0 mb-1 z-50 bg-background border border-border rounded-lg shadow-lg max-h-[250px] overflow-y-auto"
    >
      <div className="px-3 py-2 border-b border-border">
        <span className="text-xs font-medium text-foreground">{t('contextSelector.title')}</span>
      </div>

      {loading ? (
        <div className="px-3 py-4 text-center text-xs text-foreground-muted">{t('contextSelector.scanning')}</div>
      ) : modules.length === 0 ? (
        <div className="px-3 py-4 text-center text-xs text-foreground-muted">
          {t('contextSelector.noModules')}
        </div>
      ) : (
        <div className="py-1">
          {modules.map((mod) => {
            const isConnected = connectedPaths.has(mod.path)
            const selected = isConnected || isSelected(mod.path)
            return (
              <button
                key={mod.path}
                type="button"
                onClick={() => !isConnected && onToggleModule({ name: mod.name, path: mod.path })}
                disabled={isConnected}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors',
                  isConnected
                    ? 'bg-brand/5 opacity-70 cursor-default'
                    : selected
                      ? 'bg-brand/10 hover:bg-background-secondary'
                      : 'hover:bg-background-secondary',
                )}
              >
                <div
                  className={cn(
                    'w-3.5 h-3.5 rounded border flex items-center justify-center shrink-0',
                    selected ? 'bg-brand border-brand' : 'border-border',
                  )}
                >
                  {selected && (
                    <svg className="w-2.5 h-2.5 text-background" viewBox="0 0 12 12" fill="none">
                      <path d="M2 6L5 9L10 3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  )}
                </div>
                <span className="text-xs text-foreground truncate">{mod.name}</span>
                {isConnected && <Sparkles className="w-3 h-3 text-brand shrink-0 ml-auto" />}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

// =============================================================================
// ContextChips - Small badges showing selected context modules
// =============================================================================

interface ContextChipsProps {
  modules: Array<{ name: string; path: string }>
  connectedModules?: Array<{ name: string; path: string }>
  onRemove: (path: string) => void
}

export function ContextChips({ modules, connectedModules = [], onRemove }: ContextChipsProps) {
  if (modules.length === 0 && connectedModules.length === 0) return null

  return (
    <div className="flex flex-wrap gap-1 px-3 pt-2">
      {/* Connected modules (Sparkles) — not removable from chat */}
      {connectedModules.map((mod) => (
        <Badge
          key={`ai:${mod.path}`}
          variant="outline"
          className="text-[10px] px-1.5 py-0 gap-1 cursor-default border-brand/40 text-brand"
        >
          <Sparkles className="w-2.5 h-2.5" />
          {mod.name}
        </Badge>
      ))}
      {/* Manually selected modules — removable */}
      {modules.map((mod) => (
        <Badge
          key={mod.path}
          variant="outline"
          className="text-[10px] px-1.5 py-0 gap-1 cursor-default"
        >
          {mod.name}
          <button
            type="button"
            onClick={() => onRemove(mod.path)}
            className="hover:text-foreground transition-colors"
          >
            <X className="w-2.5 h-2.5" />
          </button>
        </Badge>
      ))}
    </div>
  )
}
