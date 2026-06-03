// =============================================================================
// ChatInput - Card-style input with attachments, textarea, and toolbar
// =============================================================================
// Orchestrates: file attachments (dialog + drag-drop + paste), context chips,
// skill palette, model selector, and send/stop actions.

import { useState, useRef, useEffect, useCallback, useMemo, type KeyboardEvent, type ClipboardEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { X } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { useChatStore } from '@/stores/chatStore'
import { useChatSessionStore } from '@/stores/chatSessionStore'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { ChatContextSelector, ContextChips } from './ChatContextSelector'
import { SkillPalette } from './SkillPalette'
import { ChatOverlayOrchestrator } from './overlay'
import { AttachmentPreview } from './AttachmentPreview'
import { ChatToolbar } from './ChatToolbar'
import { PendingWritesBar } from './PendingWritesBar'
import { useSessionPendingWrites } from '@/hooks/useSessionPendingWrites'
import { useAttachments } from '@/hooks/useAttachments'
import { useDropZone } from '@/hooks/useDropZone'
import { tauriApi } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import type { SkillDto } from '@/lib/tauri'

interface ChatInputProps {
  projectPath?: string
  projectId?: string
}

export function ChatInput({ projectPath, projectId }: ChatInputProps) {
  const { t } = useTranslation('chat')
  const [value, setValue] = useState('')
  const [showContextSelector, setShowContextSelector] = useState(false)
  const [contextModules, setContextModules] = useState<Array<{ name: string; path: string }>>([])
  const [skills, setSkills] = useState<SkillDto[]>([])
  const [showSkillPalette, setShowSkillPalette] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const hasOverlay = useChatStore((s) => !!(s.pendingConfirm || s.pendingAskUser || s.pendingPlan))
  const sendMessage = useChatStore((s) => s.sendMessage)
  const stopStreaming = useChatStore((s) => s.stopStreaming)
  const activeSessionId = useChatSessionStore((s) => s.activeSessionId)
  const activeDevSessionId = useChatSessionStore((s) => s.activeDevSessionId)
  const getOrCreateSendableSession = useChatSessionStore((s) => s.getOrCreateSendableSession)
  const connections = useAIConnectionStore((s) => s.connections)
  const toggleConnection = useAIConnectionStore((s) => s.toggleConnection)
  const { writes: sessionPendings, refresh: refreshPendings } =
    useSessionPendingWrites(activeSessionId ?? null)
  const panels = useNodeFloatingStore((s) => s.panels)
  const activeKnowledgeFeatureId = useCanvasTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeTabId)
    return tab?.type === 'knowledge' ? tab.data?.featureId : undefined
  })

  // Attachments
  const { attachments, addFromPaths, addFromClipboard, remove, clear: clearAttachments, toInputArray } = useAttachments()

  // Drag-and-drop
  const { isDragging } = useDropZone({
    onDrop: addFromPaths,
    disabled: isStreaming,
  })

  // Code modules connected via Sparkles — these still flow through the
  // legacy `context_modules` request field because the backend uses path
  // resolution for them. Knowledge nodes / hex skip this path: their
  // content is resolved server-side from the AI-connection registry, no
  // need to ship it through the request body.
  const connectedModules = useMemo(() => {
    const out: Array<{ name: string; path: string }> = []
    for (const p of panels) {
      const entry = connections[p.panelId]
      if (!entry?.active) continue
      if (entry.target.kind === 'code_module' && entry.target.module_path) {
        out.push({ name: p.data.moduleName, path: p.data.modulePath })
      }
    }
    return out
  }, [panels, connections])

  // Surface every active attachment (regardless of kind) so the user sees
  // what's traveling as context this turn. The name comes from the
  // target's `display_name` (or `module_name` for code modules) which is
  // captured at register time and survives popout — using the in-app
  // panels list as the source would lose the name when the source panel
  // is in its own OS window.
  const activeAttachments = useMemo(() => {
    return Object.entries(connections)
      .filter(([, entry]) => entry.active)
      .map(([id, entry]) => {
        const label =
          entry.target.kind === 'knowledge_node'
            ? 'node'
            : entry.target.kind === 'hexagon'
            ? 'hex'
            : 'module'
        const name =
          entry.target.kind === 'code_module'
            ? entry.target.module_name
            : entry.target.display_name || (
                entry.target.kind === 'knowledge_node'
                  ? entry.target.node_id.slice(0, 8)
                  : entry.target.hexagon_id.slice(0, 8)
              )
        return { id, kind: label, name }
      })
  }, [connections])

  // Auto-focus textarea when the panel opens (component mounts on docked/floating)
  useEffect(() => {
    requestAnimationFrame(() => textareaRef.current?.focus())
  }, [])

  // Auto-focus textarea when streaming ends (disabled attr removes focus; re-focus on true→false)
  const wasStreamingRef = useRef(false)
  useEffect(() => {
    if (wasStreamingRef.current && !isStreaming) {
      requestAnimationFrame(() => textareaRef.current?.focus())
    }
    wasStreamingRef.current = isStreaming
  }, [isStreaming])

  const canSend = (value.trim().length > 0 || attachments.length > 0) && !isStreaming

  // Load skills once
  useEffect(() => {
    tauriApi.listSkills().then(setSkills).catch(() => {})
  }, [])

  // Show skill palette when input starts with "/"
  const skillFilter = useMemo(() => {
    if (!value.startsWith('/')) return null
    return value.slice(1)
  }, [value])

  useEffect(() => {
    setShowSkillPalette(skillFilter !== null && !isStreaming)
  }, [skillFilter, isStreaming])

  const handleSkillSelect = useCallback((skill: SkillDto) => {
    setValue(skill.prompt)
    setShowSkillPalette(false)
    requestAnimationFrame(() => textareaRef.current?.focus())
  }, [])

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = Math.min(el.scrollHeight, 120) + 'px'
  }, [value])

  const handleSend = useCallback(async () => {
    const trimmed = value.trim()
    if ((!trimmed && attachments.length === 0) || isStreaming) return

    // Materialize a persisted session (turns the in-memory draft into a real
    // DB row on first send). Lazy persistence: empty chats never hit the DB.
    const sessionId = await getOrCreateSendableSession(projectId)

    // Merge connected (Sparkles) + manually selected modules, dedup by path
    const allModules = [...connectedModules]
    for (const m of contextModules) {
      if (!allModules.some((c) => c.path === m.path)) {
        allModules.push(m)
      }
    }

    // Prepare attachments for backend
    const attInput = toInputArray()

    sendMessage(
      trimmed || (attInput.length > 0 ? `[${attInput.length} attachment(s)]` : ''),
      sessionId,
      projectPath,
      allModules.length > 0 ? allModules : undefined,
      activeDevSessionId,
      attInput.length > 0 ? attInput : undefined,
      activeKnowledgeFeatureId,
    )
    setValue('')
    clearAttachments()
    requestAnimationFrame(() => textareaRef.current?.focus())
  }, [value, isStreaming, sendMessage, projectPath, projectId, contextModules, connectedModules, activeDevSessionId, activeKnowledgeFeatureId, getOrCreateSendableSession, attachments.length, toInputArray, clearAttachments])

  const handleStop = useCallback(() => {
    stopStreaming()
  }, [stopStreaming])

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Let SkillPalette handle keys when open
      if (showSkillPalette && ['ArrowDown', 'ArrowUp', 'Tab', 'Escape'].includes(e.key)) return
      if (showSkillPalette && e.key === 'Enter') return

      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      }
    },
    [handleSend, showSkillPalette],
  )

  // Handle paste for images
  const handlePaste = useCallback(
    (e: ClipboardEvent<HTMLTextAreaElement>) => {
      const items = e.clipboardData?.items
      if (!items) return

      // Check if any item is an image
      const hasImage = Array.from(items).some((item) => item.type.startsWith('image/'))
      if (hasImage) {
        e.preventDefault()
        addFromClipboard(items)
      }
    },
    [addFromClipboard],
  )

  // Open file dialog
  const handleAttach = useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: 'All Files',
            extensions: [
              'png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp',
              'pdf', 'txt', 'md', 'json', 'csv', 'xml', 'html',
              'css', 'js', 'ts', 'tsx', 'rs', 'py', 'go', 'java',
              'yaml', 'yml', 'toml', 'log',
            ],
          },
        ],
      })
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected]
        await addFromPaths(paths)
      }
    } catch {
      // User cancelled
    }
    requestAnimationFrame(() => textareaRef.current?.focus())
  }, [addFromPaths])

  const toggleModule = useCallback((mod: { name: string; path: string }) => {
    setContextModules((prev) => {
      const exists = prev.some((m) => m.path === mod.path)
      if (exists) return prev.filter((m) => m.path !== mod.path)
      return [...prev, mod]
    })
  }, [])

  const removeModule = useCallback((path: string) => {
    setContextModules((prev) => prev.filter((m) => m.path !== path))
  }, [])

  return (
    <div className="p-3 shrink-0 relative">
      {/* Floating overlays (tool confirm, ask user, plan approval) */}
      <ChatOverlayOrchestrator />

      {/* Skill palette */}
      {showSkillPalette && !hasOverlay && skillFilter !== null && (
        <SkillPalette
          skills={skills}
          filter={skillFilter}
          onSelect={handleSkillSelect}
          onClose={() => { setShowSkillPalette(false) }}
        />
      )}

      {/* Context selector popover */}
      {showContextSelector && (
        <ChatContextSelector
          projectPath={projectPath ?? null}
          selectedModules={contextModules}
          onToggleModule={toggleModule}
          onClose={() => setShowContextSelector(false)}
        />
      )}

      <div
        className={cn(
          'rounded-xl border bg-background-tertiary transition-all duration-200',
          isDragging
            ? 'border-brand border-dashed bg-brand/5'
            : 'border-border focus-within:border-brand/50',
        )}
      >
        {/* Attachment previews */}
        <AttachmentPreview attachments={attachments} onRemove={remove} />

        {/* Drag-drop hint overlay */}
        {isDragging && (
          <div className="px-3 py-2 text-xs text-brand text-center animate-in fade-in-0 duration-150">
            {t('input.dragDropHint')}
          </div>
        )}

        {/* Bulk accept/discard for all pending AI writes in this session.
            Renders only when there's at least one pending — collapses
            cleanly when the user clears them. */}
        <PendingWritesBar writes={sessionPendings} onResolved={refreshPendings} />

        {/* AI-connection attachments — chips for what's being shipped as
            context this turn (knowledge nodes / hex / code modules pinned
            via Sparkles). Clicking the X toggles `active` off so the
            panel's ✨ button stays in sync; the registry entry survives
            so the user can re-enable from the panel without re-clicking. */}
        {activeAttachments.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-3 pt-2.5 pb-0.5 items-center">
            <span className="text-[11px] text-foreground-muted/70 mr-1">
              {t('input.attachmentsLabel', 'Adjuntos:')}
            </span>
            {activeAttachments.map((a) => (
              <div
                key={a.id}
                className={cn(
                  'group flex items-center gap-1.5 px-2 py-1 rounded-lg border border-border bg-background-secondary',
                  'animate-in fade-in-0 zoom-in-95 duration-200',
                )}
                title={`${a.kind} · ${a.id}`}
              >
                <span className="text-[10px] uppercase tracking-wide text-foreground-muted/70">
                  {a.kind}
                </span>
                <span className="text-xs text-foreground truncate max-w-[160px]">
                  {a.name}
                </span>
                <button
                  type="button"
                  onClick={() => toggleConnection(a.id)}
                  className="ml-0.5 h-4 w-4 flex items-center justify-center rounded text-foreground-muted opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground"
                  title={t('input.disconnectAttachment', 'Desconectar')}
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Context chips — connected (Sparkles) + manually selected */}
        <ContextChips
          modules={contextModules}
          connectedModules={connectedModules}
          onRemove={removeModule}
        />

        {/* Textarea area */}
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={isStreaming ? t('input.placeholderStreaming') : t('input.placeholder')}
          rows={1}
          disabled={isStreaming}
          className="w-full min-h-[40px] max-h-[120px] bg-transparent px-3.5 pt-3 pb-1 text-sm text-foreground placeholder:text-foreground-subtle/60 outline-none resize-none disabled:opacity-50"
        />

        {/* Bottom toolbar */}
        <ChatToolbar
          onAttach={handleAttach}
          onToggleContext={() => setShowContextSelector(!showContextSelector)}
          contextActive={showContextSelector || contextModules.length > 0 || connectedModules.length > 0}
          canSend={canSend}
          isStreaming={isStreaming}
          onSend={handleSend}
          onStop={handleStop}
        />
      </div>
    </div>
  )
}
