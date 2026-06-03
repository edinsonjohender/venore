import { useEffect, useState } from 'react'
import { Pencil } from 'lucide-react'

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

export interface RenameNodeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  currentName: string
  /** Called when the user confirms; component does not rename the node itself. */
  onConfirm: (newName: string) => void
}

export function RenameNodeDialog({
  open,
  onOpenChange,
  currentName,
  onConfirm,
}: RenameNodeDialogProps) {
  const [name, setName] = useState(currentName)

  useEffect(() => {
    if (open) setName(currentName)
  }, [open, currentName])

  const trimmed = name.trim()
  const canSubmit = trimmed.length > 0 && trimmed !== currentName

  const handleConfirm = () => {
    if (!canSubmit) return
    onConfirm(trimmed)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <Pencil className="w-5 h-5 text-emerald-400" />
            <DialogTitle>Renombrar nodo</DialogTitle>
          </div>
          <DialogDescription>
            Cambia el nombre del nodo. El resto de sus datos se conservan.
          </DialogDescription>
        </DialogHeader>
        <div className="py-2">
          <Input
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
            Guardar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
