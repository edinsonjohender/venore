import { Lightbulb, X } from 'lucide-react'

import { Button } from '../ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '../ui/dialog'

export interface LighthouseOption {
  id: string
  name: string
}

export interface PickLighthouseDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Lighthouses available in the current Ocean. */
  lighthouses: LighthouseOption[]
  /** The lighthouse this node is currently assigned to (null = loose). */
  currentLighthouseId: string | null
  /** Called with the picked lighthouse id, or null to detach. */
  onPick: (lighthouseId: string | null) => void
}

export function PickLighthouseDialog({
  open,
  onOpenChange,
  lighthouses,
  currentLighthouseId,
  onPick,
}: PickLighthouseDialogProps) {
  const hasLighthouses = lighthouses.length > 0

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <Lightbulb className="w-5 h-5 text-amber-400" />
            <DialogTitle>Move to a lighthouse</DialogTitle>
          </div>
          <DialogDescription>
            Choose the lighthouse this node belongs to. The association is grouping only;
            it doesn't affect its position on the Ocean.
          </DialogDescription>
        </DialogHeader>

        <div className="py-2 max-h-72 overflow-y-auto">
          {!hasLighthouses ? (
            <div className="text-sm text-zinc-400 py-3 text-center">
              No lighthouses created yet. Create one with right-click on the Ocean.
            </div>
          ) : (
            <ul className="flex flex-col gap-1">
              {lighthouses.map((lh) => {
                const isCurrent = lh.id === currentLighthouseId
                return (
                  <li key={lh.id}>
                    <button
                      type="button"
                      onClick={() => onPick(lh.id)}
                      disabled={isCurrent}
                      className={`flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm transition-colors ${
                        isCurrent
                          ? 'bg-amber-500/20 text-amber-300 cursor-default'
                          : 'text-zinc-200 hover:bg-zinc-800'
                      }`}
                    >
                      <Lightbulb className="h-4 w-4 text-amber-400" />
                      <span className="flex-1 truncate">{lh.name}</span>
                      {isCurrent ? (
                        <span className="text-xs text-amber-400/80">actual</span>
                      ) : null}
                    </button>
                  </li>
                )
              })}
            </ul>
          )}
        </div>

        <DialogFooter className="flex flex-row items-center justify-between gap-2 sm:justify-between">
          {currentLighthouseId !== null ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => onPick(null)}
              className="text-zinc-300 hover:text-zinc-100"
            >
              <X className="mr-1 h-4 w-4" />
              Quitar del faro actual
            </Button>
          ) : (
            <span />
          )}
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancelar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
