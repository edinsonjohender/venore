// =============================================================================
// DepthLevelSelector - Depth level selection component
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { DepthLevel } from '@/lib/wizard/types'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface DepthOption {
  level: DepthLevel
  default?: boolean
}

interface DepthLevelSelectorProps {
  value: DepthLevel
  onChange: (level: DepthLevel) => void
  className?: string
}

// -----------------------------------------------------------------------------
// Options
// -----------------------------------------------------------------------------

const DEPTH_OPTIONS: DepthOption[] = [
  { level: 'minimal' },
  { level: 'normal', default: true },
  { level: 'detailed' },
  { level: 'expert' },
]

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function DepthLevelSelector({
  value,
  onChange,
  className,
}: DepthLevelSelectorProps) {
  const { t } = useTranslation('wizard')

  return (
    <div className={cn('space-y-3', className)}>
      {DEPTH_OPTIONS.map((option) => (
        <Card
          key={option.level}
          className={cn(
            'cursor-pointer transition-all hover:border-border-hover',
            value === option.level &&
              'border-brand bg-brand/5 hover:border-brand'
          )}
          onClick={() => onChange(option.level)}
        >
          <CardContent className="flex items-center gap-4 p-4">
            {/* Radio Indicator */}
            <div
              className={cn(
                'flex h-5 w-5 shrink-0 items-center justify-center rounded-full border-2 transition-colors',
                value === option.level
                  ? 'border-brand'
                  : 'border-border'
              )}
            >
              {value === option.level && (
                <div className="h-3 w-3 rounded-full bg-brand" />
              )}
            </div>

            {/* Content */}
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <span className="font-medium text-foreground">
                  {t(`shared.depthLevel.${option.level}`)}
                </span>
                {option.default && (
                  <span className="text-xs text-foreground-muted">
                    {t('shared.depthLevel.recommended')}
                  </span>
                )}
              </div>
              <p className="text-sm text-foreground-muted">
                {t(`shared.depthLevel.${option.level}Description`)}
              </p>
            </div>

            {/* Token Count */}
            <div className="shrink-0 text-right">
              <div className="text-xs text-foreground-subtle">
                {t(`shared.depthLevel.${option.level}Tokens`)}
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
