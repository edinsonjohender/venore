// =============================================================================
// OverlayToolConfirm - Floating overlay for AI tool execution permission
// =============================================================================

import { useState, useCallback } from 'react'
import { ShieldAlert } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useChatStore } from '@/stores/chatStore'
import { FloatingOverlay } from './FloatingOverlay'
import { FloatingOverlayHeader } from './FloatingOverlayHeader'

/** Human-readable label for a tool name (derived from identifier) */
function toolLabel(name: string): string {
  return name
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

/** Trim long strings for inline display. */
function clip(s: string, max = 140): string {
  if (s.length <= max) return s
  return `${s.slice(0, max)}…`
}

/** Short prefix for a UUID so node ids stay legible inline. */
function shortId(id: unknown): string {
  if (typeof id !== 'string' || id.length < 8) return String(id ?? '?')
  return id.slice(0, 8)
}

/**
 * Human-friendly preview of what a tool is about to do, formatted per
 * tool. Falls back to a compact key=value list before resorting to raw
 * JSON — the goal is to never dump a 4kb content_markdown blob into the
 * approval modal.
 */
function formatToolPreview(toolName: string, args: Record<string, unknown>): string {
  const get = (k: string) => (args[k] === undefined ? undefined : String(args[k]))

  switch (toolName) {
    // ── Terminal / app
    case 'run_terminal_command':
      return get('command') ?? '(empty command)'
    case 'run_app':
      return clip(JSON.stringify(args), 200)

    // ── File edits
    case 'read_file':
    case 'write_file':
    case 'edit_file':
    case 'multi_edit_file':
    case 'list_files':
      return get('file_path') ?? get('path') ?? '(no path)'

    // ── Web
    case 'web_fetch':
      return get('url') ?? '(no url)'
    case 'web_search':
      return `"${get('query') ?? ''}"`

    // ── Logbook content
    case 'propose_logbook_write': {
      const name = get('name') ?? '(no name)'
      const content = get('content_markdown') ?? ''
      const node = shortId(args['node_id'])
      const isEdit = !!args['edit_section_id']
      const verb = isEdit ? 'edit' : 'create'
      return `${verb} section "${name}" in node ${node}\n${clip(content, 240)}`
    }

    // ── Structure
    case 'create_lighthouse':
      return `new lighthouse "${get('name') ?? '?'}"`
    case 'create_knowledge_node': {
      const name = get('name') ?? '?'
      const lh = args['lighthouse_id'] ? ` → faro ${shortId(args['lighthouse_id'])}` : ' (sin isla)'
      return `nuevo nodo "${name}"${lh}`
    }
    case 'create_connection':
      return `${shortId(args['from_node_id'])} → ${shortId(args['to_node_id'])}`
    case 'rename_node':
      return `${shortId(args['node_id'])} → "${get('new_name') ?? '?'}"`
    case 'promote_to_lighthouse':
      return `promueve ${shortId(args['node_id'])} a faro`
    case 'set_node_lighthouse': {
      const node = shortId(args['node_id'])
      const lh = args['lighthouse_id'] && get('lighthouse_id') !== ''
        ? `faro ${shortId(args['lighthouse_id'])}`
        : '(sin isla)'
      return `${node} → ${lh}`
    }

    // ── Knowledge research
    case 'plan_hexagons':
    case 'update_hexagon':
    case 'add_evidence':
    case 'mark_dead_end':
    case 'generate_report':
      return clip(JSON.stringify(args), 200)

    // ── Sub-agent
    case 'spawn_agent':
      return `${get('type') ?? '?'}: ${clip(get('task') ?? '', 160)}`

    // ── Generic fallback: compact key:value list, no nested JSON dump.
    default: {
      const entries = Object.entries(args).map(([k, v]) => {
        if (v === null || v === undefined) return `${k}=∅`
        const s = typeof v === 'string' ? v : JSON.stringify(v)
        return `${k}=${clip(s, 60)}`
      })
      return clip(entries.join(' · '), 240)
    }
  }
}

export function OverlayToolConfirm() {
  const { t } = useTranslation('chat')
  const pendingConfirm = useChatStore((s) => s.pendingConfirm)
  const approveToolCall = useChatStore((s) => s.approveToolCall)
  const [collapsed, setCollapsed] = useState(false)
  const handleToggle = useCallback(() => setCollapsed((c) => !c), [])

  if (!pendingConfirm) return null

  const preview =
    pendingConfirm.resource ??
    formatToolPreview(pendingConfirm.tool_name, pendingConfirm.arguments)

  return (
    <FloatingOverlay
      accentColor="amber"
      onDismiss={() => approveToolCall(pendingConfirm.tool_call_id, false)}
    >
      <FloatingOverlayHeader
        icon={ShieldAlert}
        title={t('toolConfirm.title')}
        accentColor="amber"
        badge={toolLabel(pendingConfirm.tool_name)}
        isCollapsed={collapsed}
        onToggleCollapse={handleToggle}
        onClose={() => approveToolCall(pendingConfirm.tool_call_id, false)}
      />

      {!collapsed && (
        <>
          {/* Tool preview — per-tool formatter (see formatToolPreview).
              Multi-line for tools whose preview spans more than one line
              (e.g. propose_logbook_write shows section name + content). */}
          <div className="px-3 py-2.5">
            <div className="rounded bg-background-tertiary/80 px-2.5 py-1.5 overflow-x-auto max-h-32 overflow-y-auto">
              <pre className="text-xs font-mono text-foreground whitespace-pre-wrap break-words m-0">
                {preview}
              </pre>
            </div>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2 px-3 pb-3">
            <button
              onClick={() => approveToolCall(pendingConfirm.tool_call_id, true, false)}
              className="px-3 py-1 text-xs font-medium rounded bg-brand text-background hover:bg-brand-hover transition-colors"
            >
              {t('toolConfirm.allowOnce')}
            </button>
            <button
              onClick={() => approveToolCall(pendingConfirm.tool_call_id, true, true)}
              className="px-3 py-1 text-xs font-medium rounded border border-brand/40 text-brand hover:bg-brand/10 transition-colors"
            >
              {t('toolConfirm.allowSession')}
            </button>
            <button
              onClick={() => approveToolCall(pendingConfirm.tool_call_id, false)}
              className="px-3 py-1 text-xs font-medium rounded border border-border text-foreground-muted hover:text-foreground hover:bg-background-tertiary transition-colors"
            >
              {t('toolConfirm.deny')}
            </button>
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}
