// =============================================================================
// OverlayPlanView - Floating overlay for plan approval
// =============================================================================

import { useState, useCallback } from 'react'
import { ClipboardList, Check, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useChatStore } from '@/stores/chatStore'
import { FloatingOverlay } from './FloatingOverlay'
import { FloatingOverlayHeader } from './FloatingOverlayHeader'

export function OverlayPlanView() {
  const { t } = useTranslation('chat')
  const pendingPlan = useChatStore((s) => s.pendingPlan)
  const approvePlan = useChatStore((s) => s.approvePlan)
  const [collapsed, setCollapsed] = useState(false)
  const handleToggle = useCallback(() => setCollapsed((c) => !c), [])

  if (!pendingPlan) return null

  return (
    <FloatingOverlay accentColor="blue">
      <FloatingOverlayHeader
        icon={ClipboardList}
        title={t('planView.title')}
        accentColor="blue"
        isCollapsed={collapsed}
        onToggleCollapse={handleToggle}
        onClose={() => approvePlan(pendingPlan.tool_call_id, false)}
      />

      {!collapsed && (
        <>
          {/* Summary */}
          <div className="px-3 py-2.5">
            <p className="text-sm text-foreground">{pendingPlan.summary}</p>
          </div>

          {/* Steps */}
          {pendingPlan.steps.length > 0 && (
            <div className="px-3 pb-2">
              <ol className="space-y-1 max-h-[200px] overflow-y-auto">
                {pendingPlan.steps.map((step, i) => (
                  <li key={i} className="flex items-start gap-2 text-xs text-foreground-muted">
                    <span className="font-mono text-foreground-subtle shrink-0 mt-px">{i + 1}.</span>
                    <span>{step}</span>
                  </li>
                ))}
              </ol>
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 px-3 py-2.5 border-t border-border">
            <button
              type="button"
              onClick={() => approvePlan(pendingPlan.tool_call_id, true)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded border border-border bg-background-tertiary/50 text-foreground-muted hover:bg-emerald-500/10 hover:text-emerald-400/90 hover:border-emerald-500/30 transition-colors"
            >
              <Check className="w-3 h-3" />
              {t('planView.approve')}
            </button>
            <button
              type="button"
              onClick={() => approvePlan(pendingPlan.tool_call_id, false)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded border border-border bg-background-tertiary/50 text-foreground-muted hover:bg-red-500/10 hover:text-red-400/90 hover:border-red-500/30 transition-colors"
            >
              <X className="w-3 h-3" />
              {t('planView.reject')}
            </button>
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}
