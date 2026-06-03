// =============================================================================
// PickIslandColorDialog — Closed palette for per-island color overrides
// =============================================================================
// 12 preset swatches + a "Default" option that clears the override and
// returns the island to the id-derived color.

import { useEffect, useState } from 'react'
import { Check, Palette, X } from 'lucide-react'

import { Button } from '../ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog'
import { tauriApi } from '@/lib/tauri'
import { ISLAND_PALETTE, derivedIslandColor } from './island-utils'
import { cn } from '@/lib/utils'

export interface PickIslandColorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The lighthouse whose color we're editing. Falsy = dialog hidden. */
  source: { lighthouse_id: string; name: string } | null
  /** Currently effective color (override if any, else derived). */
  currentColor: string | null
  /** True when there's an explicit override (so we can show "actual" hint). */
  hasOverride: boolean
  projectPath: string
}

export function PickIslandColorDialog({
  open,
  onOpenChange,
  source,
  currentColor,
  hasOverride,
  projectPath,
}: PickIslandColorDialogProps) {
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (!open) setBusy(false)
  }, [open])

  if (!source) return null

  const apply = async (color: string | null) => {
    if (busy) return
    setBusy(true)
    try {
      await tauriApi.setLighthouseColor({
        project_path: projectPath,
        lighthouse_id: source.lighthouse_id,
        color,
      })
      onOpenChange(false)
    } catch (err) {
      console.error('Set lighthouse color failed:', err)
    } finally {
      setBusy(false)
    }
  }

  const derived = derivedIslandColor(source.lighthouse_id)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette className="h-4 w-4 text-foreground-muted" />
            Color de {source.name}
          </DialogTitle>
          <DialogDescription>
            Elige uno de la paleta. "Por defecto" devuelve la isla a su color derivado.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-6 gap-2 py-2">
          {ISLAND_PALETTE.map((color) => {
            const isCurrent = currentColor === color
            return (
              <button
                key={color}
                type="button"
                onClick={() => apply(color)}
                disabled={busy}
                title={color}
                className={cn(
                  'relative h-8 w-8 rounded border-2 transition-transform hover:scale-110',
                  isCurrent ? 'border-foreground' : 'border-transparent',
                  busy && 'opacity-50 cursor-wait',
                )}
                style={{ backgroundColor: color }}
              >
                {isCurrent && (
                  <Check className="absolute inset-0 m-auto h-4 w-4 text-black drop-shadow" />
                )}
              </button>
            )
          })}
        </div>

        <div className="flex items-center justify-between gap-2 pt-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => apply(null)}
            disabled={busy || !hasOverride}
            className="gap-2"
            title={
              hasOverride
                ? 'Quitar el override y volver al color derivado'
                : 'No hay override que quitar'
            }
          >
            <span
              className="h-3 w-3 rounded-sm border border-border"
              style={{ backgroundColor: derived }}
              aria-hidden
            />
            Por defecto
          </Button>
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            <X className="mr-1 h-4 w-4" />
            Cerrar
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
