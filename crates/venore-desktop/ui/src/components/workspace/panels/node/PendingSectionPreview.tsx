// =============================================================================
// PendingSectionPreview — review surface for AI-proposed section writes
// =============================================================================
// Renders inside the node panel when the user selects a pending entry from
// the sidebar. For Edit kind, the unified diff is shown via the same
// DiffViewer we already use in the GitHub PR detail view (line-level
// red/green with shiki highlighting). For Create, the proposed markdown
// is rendered with the regular MarkdownRenderer so the preview matches a
// real section. The chat itself never displays this content.

import { useState } from 'react'
import { Check, RefreshCw, Sparkles, Trash2, X } from 'lucide-react'

import { tauriApi, type PendingWriteDto } from '@/lib/tauri'
import { Button } from '@/components/ui/button'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'
import { DiffViewer } from '@/components/workspace/canvas/pr-detail/DiffViewer'
import { formatTimeAgo } from '@/lib/time'

interface Props {
  write: PendingWriteDto
  /** Called after a successful accept/discard so the parent can refetch. */
  onResolved?: () => void
}

export function PendingSectionPreview({ write, onResolved }: Props) {
  const [busy, setBusy] = useState<null | 'accept' | 'discard' | 'regenerate'>(null)
  const [error, setError] = useState<string | null>(null)

  const isEdit = write.kind === 'edit'

  const accept = async () => {
    setBusy('accept')
    setError(null)
    try {
      await tauriApi.acceptPendingWrite({ write_id: write.write_id })
      onResolved?.()
    } catch (err) {
      setError(typeof err === 'string' ? err : 'No se pudo aceptar la propuesta')
    } finally {
      setBusy(null)
    }
  }

  const discard = async () => {
    setBusy('discard')
    setError(null)
    try {
      await tauriApi.discardPendingWrite({ write_id: write.write_id })
      onResolved?.()
    } catch (err) {
      setError(typeof err === 'string' ? err : 'No se pudo descartar la propuesta')
    } finally {
      setBusy(null)
    }
  }

  const regenerate = async () => {
    setBusy('regenerate')
    setError(null)
    try {
      await tauriApi.regeneratePendingWrite({ write_id: write.write_id })
      // The backend re-emits ai-write-proposed; the parent's listener will
      // refetch and pass us a fresh PendingWriteDto.
    } catch (err) {
      setError(typeof err === 'string' ? err : 'No se pudo regenerar la propuesta')
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="shrink-0 flex items-center gap-2 px-4 py-2 border-b border-border bg-background-secondary">
        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium bg-amber-500/15 text-amber-300 border border-amber-500/40">
          <Sparkles className="w-3 h-3" />
          Pending
        </span>
        <span className="text-[11px] text-foreground-muted uppercase tracking-wide">
          {isEdit ? 'Proposed edit' : 'New section'}
        </span>
        <span className="text-sm text-foreground truncate flex-1">{write.name}</span>
        <span className="text-[11px] text-foreground-muted/70 shrink-0">
          {write.ai_model} · {formatTimeAgo(write.created_at * 1000)}
        </span>
      </div>

      {/* Body */}
      <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
        {isEdit && write.diff_patch ? (
          <DiffViewer
            file={{
              filename: write.name,
              status: 'modified',
              additions: write.additions,
              deletions: write.deletions,
              patch: write.diff_patch,
            }}
          />
        ) : (
          <div className="flex-1 overflow-auto px-4 py-3">
            <MarkdownRenderer content={write.content_markdown} />
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="shrink-0 border-t border-border bg-background-secondary px-3 py-2 flex items-center gap-2">
        {error && (
          <span className="text-[11px] text-red-400 flex-1 truncate" title={error}>
            {error}
          </span>
        )}
        <span className="flex-1" />
        <Button
          variant="ghost"
          size="sm"
          onClick={regenerate}
          disabled={busy !== null}
          title="Pedir a la AI que vuelva a generar el contenido"
        >
          <RefreshCw className={busy === 'regenerate' ? 'w-3.5 h-3.5 animate-spin' : 'w-3.5 h-3.5'} />
          Regenerar
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={discard}
          disabled={busy !== null}
        >
          {busy === 'discard' ? <X className="w-3.5 h-3.5" /> : <Trash2 className="w-3.5 h-3.5" />}
          Descartar
        </Button>
        <Button
          variant="default"
          size="sm"
          onClick={accept}
          disabled={busy !== null}
        >
          <Check className="w-3.5 h-3.5" />
          Aceptar
        </Button>
      </div>
    </div>
  )
}
