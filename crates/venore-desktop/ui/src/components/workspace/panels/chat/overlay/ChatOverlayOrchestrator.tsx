// =============================================================================
// ChatOverlayOrchestrator - Decides which floating overlay to show
// =============================================================================
// Priority (mutually exclusive by backend design):
// 1. pendingConfirm (tool permission)
// 2. pendingAskUser (agent question)
// 3. pendingPlan   (plan approval)

import { useChatStore } from '@/stores/chatStore'
import { OverlayToolConfirm } from './OverlayToolConfirm'
import { OverlayAskUser } from './OverlayAskUser'
import { OverlayPlanView } from './OverlayPlanView'
import { OverlayExecutionStatus } from './OverlayExecutionStatus'

export function ChatOverlayOrchestrator() {
  const pendingConfirm = useChatStore((s) => s.pendingConfirm)
  const pendingAskUser = useChatStore((s) => s.pendingAskUser)
  const pendingPlan = useChatStore((s) => s.pendingPlan)

  if (pendingConfirm) return <OverlayToolConfirm />
  if (pendingAskUser) return <OverlayAskUser />
  if (pendingPlan) return <OverlayPlanView />

  return <OverlayExecutionStatus />
}
