// =============================================================================
// OverlayExecutionStatus - Floating overlay for tool calls, sub-agents & tasks
// =============================================================================
// Shows execution activity in a single emerald-accented overlay above the input.
// Auto-hides when empty (with 2s linger to avoid flicker).
// Supports collapse/expand and manual dismiss.

import { useState, useCallback, useEffect } from 'react'
import { Activity } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useChatStore } from '@/stores/chatStore'
import { FloatingOverlay } from './FloatingOverlay'
import { FloatingOverlayHeader } from './FloatingOverlayHeader'
import { ChatToolCall } from '../ChatToolCall'
import { ChatSubAgent } from '../ChatSubAgent'
import { ChatTaskList } from '../ChatTaskList'
import { useActiveToolCalls } from './useActiveToolCalls'
import { useExecutionVisible } from './useExecutionVisible'

export function OverlayExecutionStatus() {
  const { t } = useTranslation('chat')
  const subAgents = useChatStore((s) => s.subAgents)
  const tasks = useChatStore((s) => s.tasks)
  const toolCalls = useActiveToolCalls()
  const visible = useExecutionVisible(toolCalls, subAgents, tasks)
  const [collapsed, setCollapsed] = useState(false)
  const [dismissed, setDismissed] = useState(false)

  // Reset dismissed when new activity starts
  const isStreaming = useChatStore((s) => s.isStreaming)
  const hasRunning =
    toolCalls.some((tc) => tc.toolCall.status === 'running') ||
    subAgents.some((sa) => sa.status === 'started')

  // All hooks MUST be above any early return
  const handleDismiss = useCallback(() => setDismissed(true), [])
  const handleToggle = useCallback(() => setCollapsed((c) => !c), [])

  // If new activity starts after dismiss, un-dismiss
  useEffect(() => {
    if (dismissed && (isStreaming || hasRunning)) {
      setDismissed(false)
    }
  }, [dismissed, isStreaming, hasRunning])

  if (!visible || dismissed) return null

  const runningCount = toolCalls.filter((tc) => tc.toolCall.status === 'running').length
  const badge = runningCount > 0 ? `${runningCount} running` : undefined

  return (
    <FloatingOverlay accentColor="emerald" onDismiss={handleDismiss}>
      <FloatingOverlayHeader
        icon={Activity}
        title={t('execution.title')}
        accentColor="emerald"
        badge={badge}
        isCollapsed={collapsed}
        onToggleCollapse={handleToggle}
        onClose={handleDismiss}
      />
      {!collapsed && (
        <div className="px-2 py-1.5">
          {/* Tool calls */}
          {toolCalls.map(({ toolCall, messageId }) => (
            <ChatToolCall key={toolCall.id} toolCall={toolCall} messageId={messageId} embedded />
          ))}

          {/* Sub-agents */}
          {subAgents.map((sa) => (
            <ChatSubAgent key={sa.agent_id} payload={sa} embedded />
          ))}

          {/* Task list */}
          {tasks.length > 0 && <ChatTaskList tasks={tasks} embedded />}
        </div>
      )}
    </FloatingOverlay>
  )
}
