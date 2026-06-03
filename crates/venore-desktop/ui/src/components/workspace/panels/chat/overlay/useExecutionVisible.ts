// =============================================================================
// useExecutionVisible - Controls visibility of the execution status overlay
// =============================================================================
// Visible when there's execution content AND (streaming OR running items OR linger).
// Lingers 2s after all activity stops to avoid flicker.

import { useState, useEffect, useRef } from 'react'
import { useChatStore } from '@/stores/chatStore'
import type { ActiveToolCall } from './useActiveToolCalls'
import type { SubAgentPayload, TaskItemPayload } from '@/stores/chatStore'

const LINGER_MS = 2000

export function useExecutionVisible(
  toolCalls: ActiveToolCall[],
  subAgents: SubAgentPayload[],
  tasks: TaskItemPayload[],
): boolean {
  const isStreaming = useChatStore((s) => s.isStreaming)
  const [lingering, setLingering] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const hasContent = toolCalls.length > 0 || subAgents.length > 0 || tasks.length > 0
  const hasRunning =
    toolCalls.some((tc) => tc.toolCall.status === 'running') ||
    subAgents.some((sa) => sa.status === 'started') ||
    tasks.some((t) => t.status === 'in_progress')

  const isActive = hasContent && (isStreaming || hasRunning)

  useEffect(() => {
    if (isActive) {
      // Clear any pending linger timer — we're active again
      if (timerRef.current) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
      setLingering(true)
    } else if (lingering && !isActive) {
      // Activity stopped — start linger countdown
      timerRef.current = setTimeout(() => {
        setLingering(false)
        timerRef.current = null
      }, LINGER_MS)
    }

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current)
        timerRef.current = null
      }
    }
  }, [isActive]) // eslint-disable-line react-hooks/exhaustive-deps

  return hasContent && (isActive || lingering)
}
