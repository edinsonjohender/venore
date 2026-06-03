// =============================================================================
// NodeConnections — Dependencies and dependents sections
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'

interface NodeConnectionsProps {
  dependencies: string[]
  dependents: string[]
}

export function NodeConnections({ dependencies, dependents }: NodeConnectionsProps) {
  const { t } = useTranslation('project')

  return (
    <div className="border-b border-border">
      <CollapsibleSection title={t('nodeConnections.dependencies')} count={dependencies.length}>
        {dependencies.length === 0 ? (
          <p className="text-[10px] text-foreground-muted/50 px-3 pb-2">{t('nodeConnections.none')}</p>
        ) : (
          <ul className="px-3 pb-2 space-y-0.5">
            {dependencies.map((dep) => (
              <li key={dep} className="flex items-center gap-1.5 text-xs text-foreground-muted">
                <span className="text-accent/60 text-[10px]">{'\u2192'}</span>
                <span className="truncate">{dep}</span>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>

      <CollapsibleSection title={t('nodeConnections.usedBy')} count={dependents.length}>
        {dependents.length === 0 ? (
          <p className="text-[10px] text-foreground-muted/50 px-3 pb-2">{t('nodeConnections.none')}</p>
        ) : (
          <ul className="px-3 pb-2 space-y-0.5">
            {dependents.map((dep) => (
              <li key={dep} className="flex items-center gap-1.5 text-xs text-foreground-muted">
                <span className="text-accent/60 text-[10px]">{'\u2190'}</span>
                <span className="truncate">{dep}</span>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>
    </div>
  )
}

// -----------------------------------------------------------------------------
// CollapsibleSection — Reusable within this file
// -----------------------------------------------------------------------------

function CollapsibleSection({
  title,
  count,
  children,
}: {
  title: string
  count: number
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(count > 0)

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
          {title}
        </span>
        <span className="text-[10px] text-foreground-muted/50">({count})</span>
      </button>
      {open && children}
    </div>
  )
}
