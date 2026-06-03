import { useEffect, useState } from 'react'
import { Lightbulb, Plus } from 'lucide-react'

import { Button } from '../ui/button'
import { Input } from '../ui/input'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '../ui/dialog'

export type CreateNodeKind = 'node' | 'lighthouse'

export interface CreateNodeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  targetCell: { col: number; row: number } | null
  /** What kind of entity is being created — drives copy and icon. Defaults to 'node'. */
  kind?: CreateNodeKind
  /** Called when the user confirms; component does not create the entity itself. */
  onConfirm: (name: string) => void
}

const COPY: Record<CreateNodeKind, {
  title: string
  placeholder: string
  iconClass: string
}> = {
  node: {
    title: 'Nuevo nodo',
    placeholder: 'ej. Estrategia de auth',
    iconClass: 'text-emerald-400',
  },
  lighthouse: {
    title: 'Nuevo faro',
    placeholder: 'ej. Planes',
    iconClass: 'text-amber-400',
  },
}

export function CreateNodeDialog({
  open,
  onOpenChange,
  targetCell,
  kind = 'node',
  onConfirm,
}: CreateNodeDialogProps) {
  const [name, setName] = useState('')
  const copy = COPY[kind]
  const Icon = kind === 'lighthouse' ? Lightbulb : Plus

  useEffect(() => {
    if (open) setName('')
  }, [open])

  const trimmed = name.trim()
  const canSubmit = trimmed.length > 0

  const handleConfirm = () => {
    if (!canSubmit) return
    onConfirm(trimmed)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <Icon className={`w-5 h-5 ${copy.iconClass}`} />
            <DialogTitle>{copy.title}</DialogTitle>
          </div>
          <DialogDescription>
            {targetCell
              ? `Will be created at cell (${targetCell.col}, ${targetCell.row}).`
              : 'Select an empty cell on the Ocean.'}
          </DialogDescription>
        </DialogHeader>
        <div className="py-2">
          <Input
            placeholder={copy.placeholder}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleConfirm()
            }}
            autoFocus
          />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancelar
          </Button>
          <Button
            onClick={handleConfirm}
            disabled={!canSubmit}
            className="bg-emerald-500 hover:bg-emerald-600 text-white"
          >
            Crear
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
