// =============================================================================
// NodeExports — Exported symbols (functions, classes, interfaces, etc.)
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { SymbolInfoDto } from '@/lib/tauri'

interface NodeExportsProps {
  exports: SymbolInfoDto[]
}

const KIND_ICONS: Record<string, string> = {
  function: '\u0192',  // f
  class: '\u25A0',     // filled square
  interface: '\u25C7', // diamond
  enum: '\u25CB',      // circle
  type: '\u25B3',      // triangle
  constant: '\u25CF',  // filled circle
}

function kindIcon(kind: string): string {
  return KIND_ICONS[kind.toLowerCase()] ?? '\u2022' // bullet
}

export function NodeExports({ exports }: NodeExportsProps) {
  const { t } = useTranslation('project')
  const [open, setOpen] = useState(exports.length > 0)

  if (exports.length === 0) return null

  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1 w-full px-3 py-1.5 text-left hover:bg-foreground/5 transition-colors"
      >
        <ChevronRight
          className={cn(
            'w-3 h-3 text-foreground-muted/50 transition-transform',
            open && 'rotate-90',
          )}
        />
        <span className="text-[10px] font-semibold text-foreground-muted uppercase tracking-wider">
          {t('nodeExports.exportsHeader')}
        </span>
        <span className="text-[10px] text-foreground-muted/50">({exports.length})</span>
      </button>

      {open && (
        <ul className="px-3 pb-2 space-y-0.5">
          {exports.map((sym, i) => (
            <li key={`${sym.name}-${i}`} className="flex items-center gap-1.5 text-xs">
              <span className="text-accent/70 w-3 text-center text-[10px] shrink-0">
                {kindIcon(sym.kind)}
              </span>
              <span className="text-foreground truncate">{sym.name}</span>
              <span className="text-foreground-muted/40 text-[10px] truncate ml-auto">
                {sym.file}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
