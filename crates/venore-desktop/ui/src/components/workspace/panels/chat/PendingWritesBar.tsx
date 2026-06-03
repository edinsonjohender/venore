// =============================================================================
// PendingWritesBar — bulk accept / discard for AI proposals in this session
// =============================================================================
// When the AI fans out N pending writes across one or more nodes (very
// common: "create a faro with 7 sub-topics" → 7 proposals), reviewing one
// at a time is friction. This bar surfaces the count and offers two
// shortcuts: accept all, discard all. Both run in parallel through
// the existing per-write commands; no new backend surface needed.
//
// Visible only when there's at least one pending write for the active
// session.

import { useState } from 'react'
import { Check, Sparkles, X } from 'lucide-react'
import { tauriApi, type PendingWriteDto } from '@/lib/tauri'
import { cn } from '@/lib/utils'

interface PendingWritesBarProps {
  writes: PendingWriteDto[]
  onResolved: () => void
}

export function PendingWritesBar({ writes, onResolved }: PendingWritesBarProps) {
  const [busy, setBusy] = useState<null | 'accept' | 'discard'>(null)
  const [error, setError] = useState<string | null>(null)

  if (writes.length === 0) return null

  const acceptAll = async () => {
    setBusy('accept')
    setError(null)
    try {
      // Sequential so a mid-batch failure doesn't leave half the writes
      // applied with no clear error surface. Volume is small (handful to
      // ~14 in the worst observed case); throughput isn't a concern.
      for (const w of writes) {
        await tauriApi.acceptPendingWrite({ write_id: w.write_id })
      }
      onResolved()
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Failed to accept all')
    } finally {
      setBusy(null)
    }
  }

  const discardAll = async () => {
    setBusy('discard')
    setError(null)
    try {
      for (const w of writes) {
        await tauriApi.discardPendingWrite({ write_id: w.write_id })
      }
      onResolved()
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Failed to discard all')
    } finally {
      setBusy(null)
    }
  }

  const nodeCount = new Set(writes.map((w) => w.node_id)).size
  const summary =
    nodeCount === 1
      ? `${writes.length} pending`
      : `${writes.length} pending across ${nodeCount} nodes`

  return (
    <div
      className={cn(
        'mx-3 mt-2 flex items-center gap-2 px-2.5 py-1.5 rounded-lg',
        'border border-amber-500/40 bg-amber-500/10 text-[11px]',
        'animate-in fade-in-0 zoom-in-95 duration-200',
      )}
    >
      <Sparkles className="w-3.5 h-3.5 text-amber-300 shrink-0" />
      <span className="text-amber-200 flex-1 truncate" title={writes.map((w) => w.name).join(' · ')}>
        {summary}
      </span>
      {error && (
        <span className="text-red-300 shrink-0" title={error}>
          error
        </span>
      )}
      <button
        type="button"
        onClick={discardAll}
        disabled={busy !== null}
        className={cn(
          'inline-flex items-center gap-1 px-2 py-0.5 rounded',
          'text-foreground-muted hover:text-foreground hover:bg-foreground/5',
          'disabled:opacity-50 disabled:cursor-not-allowed',
        )}
        title="Discard all pending in this session"
      >
        <X className="w-3 h-3" />
        Discard all
      </button>
      <button
        type="button"
        onClick={acceptAll}
        disabled={busy !== null}
        className={cn(
          'inline-flex items-center gap-1 px-2 py-0.5 rounded bg-emerald-500/80 text-white',
          'hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed',
        )}
        title="Accept all pending in this session"
      >
        <Check className="w-3 h-3" />
        Accept all
      </button>
    </div>
  )
}
