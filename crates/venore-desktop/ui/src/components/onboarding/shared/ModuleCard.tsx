// =============================================================================
// ModuleCard - Module display card with selection
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { CheckCircle2, Circle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ModuleInfo } from '@/lib/wizard/types'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ModuleCardProps {
  module: ModuleInfo
  selected: boolean
  onToggle: () => void
  className?: string
}

// -----------------------------------------------------------------------------
// Helper Functions
// -----------------------------------------------------------------------------

function getModuleTypeColor(type: string): 'default' | 'info' | 'warning' {
  switch (type) {
    case 'package':
    case 'crate':
      return 'info'
    case 'component':
      return 'default'
    case 'service':
    case 'library':
      return 'warning'
    default:
      return 'default'
  }
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function ModuleCard({
  module,
  selected,
  onToggle,
  className,
}: ModuleCardProps) {
  const { t } = useTranslation('wizard')

  return (
    <Card
      className={cn(
        'cursor-pointer transition-all hover:border-border-hover',
        selected && 'border-brand bg-brand/5 hover:border-brand',
        className
      )}
      onClick={onToggle}
    >
      <CardContent className="flex items-start gap-3 p-4">
        {/* Checkbox Indicator */}
        <div className="shrink-0 pt-1">
          {selected ? (
            <CheckCircle2 className="h-5 w-5 text-brand" />
          ) : (
            <Circle className="h-5 w-5 text-border" />
          )}
        </div>

        {/* Module Info */}
        <div className="min-w-0 flex-1">
          {/* Module Name */}
          <div className="mb-1 flex items-center gap-2">
            <h4 className="font-medium text-foreground">{module.name}</h4>
            {module.hasExistingContext && (
              <Badge variant="warning" className="text-xs">
                {t('shared.moduleCard.hasContext')}
              </Badge>
            )}
          </div>

          {/* Module Path */}
          <p className="mb-2 truncate text-sm text-foreground-muted">
            {module.path}
          </p>

          {/* Module Metadata */}
          <div className="flex flex-wrap gap-2">
            <Badge variant={getModuleTypeColor(module.moduleType)}>
              {module.moduleType}
            </Badge>
            <Badge variant="outline">
              {t('shared.moduleCard.file', { count: module.fileCount })}
            </Badge>
            {module.confidence !== undefined && (
              <Badge variant="outline">
                {typeof module.confidence === 'number'
                  ? t('shared.moduleCard.confidence', { percent: Math.round(module.confidence * 100) })
                  : `${module.confidence} confidence`
                }
              </Badge>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
