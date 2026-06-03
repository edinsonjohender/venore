// =============================================================================
// StepIndicator - Official step progress indicator for wizards (v1 exact copy)
// =============================================================================

import { useEffect, useRef } from 'react'
import { Check, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface Step {
  num: number
  label: string
}

export interface StepIndicatorProps {
  steps: Step[]
  currentStep: number
  variant?: 'warning' | 'brand'
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function StepIndicator({ steps, currentStep, variant = 'brand' }: StepIndicatorProps) {
  const activeStepRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to current step
  useEffect(() => {
    if (activeStepRef.current) {
      activeStepRef.current.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'center',
      })
    }
  }, [currentStep])

  const getContainerClasses = (stepNum: number) => {
    const base = 'flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-colors'

    if (currentStep === stepNum) {
      return variant === 'brand'
        ? `${base} bg-brand/20 text-brand`
        : `${base} bg-semantic-warning/20 text-semantic-warning`
    }
    if (currentStep > stepNum) {
      return `${base} text-foreground-muted`
    }
    return `${base} text-foreground-subtle`
  }

  const getCircleClasses = (stepNum: number) => {
    const base = 'w-5 h-5 rounded-full flex items-center justify-center text-[10px]'

    if (currentStep > stepNum) {
      return variant === 'brand'
        ? `${base} bg-brand/20 text-brand`
        : `${base} bg-semantic-warning/20 text-semantic-warning`
    }
    if (currentStep === stepNum) {
      return variant === 'brand'
        ? `${base} bg-brand text-background`
        : `${base} bg-semantic-warning text-background`
    }
    return `${base} bg-background-tertiary text-foreground-muted`
  }

  return (
    <>
      <style>{`
        .step-indicator-scroll::-webkit-scrollbar {
          display: none;
        }
      `}</style>
      <div
        className="overflow-x-auto border-b border-border bg-background-tertiary/50 step-indicator-scroll flex-shrink-0"
        style={{
          scrollbarWidth: 'none',
          msOverflowStyle: 'none',
        }}
      >
        <div className="flex items-center gap-2 py-3 px-4 min-w-max" style={{ minHeight: '56px' }}>
          {steps.map((step, idx) => (
            <div
              key={step.num}
              className="flex items-center shrink-0"
              ref={currentStep === step.num ? activeStepRef : null}
            >
              <div className={getContainerClasses(step.num)}>
                <span className={getCircleClasses(step.num)}>
                  {currentStep > step.num ? <Check className="w-3 h-3" /> : step.num}
                </span>
                <span className="whitespace-nowrap">{step.label}</span>
              </div>
              {idx < steps.length - 1 && (
                <ChevronRight className="w-4 h-4 text-foreground-subtle mx-1 shrink-0" />
              )}
            </div>
          ))}
        </div>
      </div>
    </>
  )
}
