// =============================================================================
// ChatMessage - Clean message styles (user / assistant / system)
// =============================================================================
// User: right-aligned with brand tint, image attachments, markdown rendering
// Assistant: left-aligned, full markdown + code highlighting
// System: centered divider

import type { ChatMessage as ChatMessageType } from '@/stores/chatStore'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'
import { formatTimeAgo } from '@/lib/time'
import { cn } from '@/lib/utils'
import { FileText } from 'lucide-react'

interface ChatMessageProps {
  message: ChatMessageType
}

// -----------------------------------------------------------------------------
// Streaming dots animation
// -----------------------------------------------------------------------------

function StreamingDots() {
  return (
    <span className="inline-flex items-center gap-[3px] h-4 ml-0.5 align-middle">
      <span className="w-1 h-1 rounded-full bg-brand animate-[dotPulse_1.4s_ease-in-out_0s_infinite]" />
      <span className="w-1 h-1 rounded-full bg-brand animate-[dotPulse_1.4s_ease-in-out_0.2s_infinite]" />
      <span className="w-1 h-1 rounded-full bg-brand animate-[dotPulse_1.4s_ease-in-out_0.4s_infinite]" />
    </span>
  )
}

// -----------------------------------------------------------------------------
// Tool-marker sanitizer
// -----------------------------------------------------------------------------
// Some models (notably Gemini) occasionally echo a fabricated tool call as
// literal text — e.g. "[tool: read_file path=...]" — instead of issuing a
// real function call. Real tool calls are rendered separately as chips
// (message.toolCalls), so any such bracketed marker in the text content is
// noise that renders as broken pseudo-code. Strip it defensively. The system
// prompt also instructs the model not to emit these, so this is belt-and-
// suspenders for the cases where it slips through.
const TOOL_MARKER_RE = /\[tool:[^\]]*\]/gi

function stripToolMarkers(content: string): string {
  if (!content.includes('[tool:')) return content
  return content
    .replace(TOOL_MARKER_RE, '')
    // Collapse blank lines left behind by removed markers.
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function ChatMessage({ message }: ChatMessageProps) {
  // ── System message ──────────────────────────────────────────────────────
  if (message.role === 'system') {
    return (
      <div className="flex items-center gap-2 px-2">
        <div className="h-px flex-1 bg-border" />
        <span className="text-[10px] font-mono text-foreground-subtle tracking-wide uppercase shrink-0">
          {message.content}
        </span>
        <div className="h-px flex-1 bg-border" />
      </div>
    )
  }

  // ── User message ────────────────────────────────────────────────────────
  if (message.role === 'user') {
    const imageAttachments = message.attachments?.filter((a) => a.thumbnailUrl) ?? []
    const fileAttachments = message.attachments?.filter((a) => !a.thumbnailUrl) ?? []

    return (
      <div className="flex flex-col items-end gap-1">
        <div
          className={cn(
            'max-w-[85%]',
            'bg-brand/[0.06] border border-brand/20',
            'rounded-2xl rounded-br-md',
            'px-4 py-3',
          )}
        >
          {/* Image attachments */}
          {imageAttachments.length > 0 && (
            <div className="flex flex-wrap gap-2 mb-2">
              {imageAttachments.map((att, i) => (
                <img
                  key={i}
                  src={att.thumbnailUrl!}
                  alt={att.name}
                  className="w-48 max-h-64 object-contain rounded-lg border border-border"
                />
              ))}
            </div>
          )}

          {/* File attachment chips */}
          {fileAttachments.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mb-2">
              {fileAttachments.map((att, i) => (
                <span
                  key={i}
                  className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-background-tertiary/50 text-[11px] text-foreground-muted font-mono"
                >
                  <FileText className="w-3 h-3" />
                  {att.name}
                </span>
              ))}
            </div>
          )}

          {/* Content rendered as markdown */}
          {message.content && (
            <MarkdownRenderer content={message.content} />
          )}
        </div>
        <span className="text-[10px] text-foreground-subtle/50 pr-1">
          {formatTimeAgo(message.timestamp)}
        </span>
      </div>
    )
  }

  // ── Assistant message ───────────────────────────────────────────────────
  return (
    <div className="flex flex-col items-start gap-1">
      <div className="max-w-[95%]">
        {/* Tool calls */}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mb-1.5">
            {message.toolCalls.map((tc) => (
              <span
                key={tc.id}
                className={cn(
                  'inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono',
                  tc.status === 'completed' && 'bg-emerald-500/10 text-emerald-400/80',
                  tc.status === 'error' && 'bg-red-500/10 text-red-400/80',
                  tc.status === 'running' && 'bg-blue-500/10 text-blue-400/80',
                  tc.status === 'pending' && 'bg-foreground-subtle/10 text-foreground-subtle',
                )}
              >
                {tc.name.replace(/_/g, ' ')}
              </span>
            ))}
          </div>
        )}

        {/* Content (tool markers stripped — real tool calls render as chips above) */}
        {(() => {
          const sanitized = stripToolMarkers(message.content ?? '')
          return (
            <div className="text-foreground">
              {sanitized ? (
                <MarkdownRenderer content={sanitized} />
              ) : message.isStreaming ? (
                <StreamingDots />
              ) : null}
              {message.isStreaming && sanitized && <StreamingDots />}
            </div>
          )
        })()}
      </div>

      <span className="text-[10px] text-foreground-subtle/50">
        {formatTimeAgo(message.timestamp)}
      </span>
    </div>
  )
}
