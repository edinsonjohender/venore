// =============================================================================
// useActiveToolCalls - Derives active tool calls from the current agent burst
// =============================================================================
// Scans assistant messages in reverse until a user message is found.
// Returns tool calls with their parent messageId (needed for revert).

import { useMemo } from 'react'
import { useChatStore } from '@/stores/chatStore'
import type { ToolCallInfo } from '@/stores/chatStore'

export interface ActiveToolCall {
  toolCall: ToolCallInfo
  messageId: string
}

export function useActiveToolCalls(): ActiveToolCall[] {
  const messages = useChatStore((s) => s.messages)

  return useMemo(() => {
    const result: ActiveToolCall[] = []
    for (let i = messages.length - 1; i >= 0; i--) {
      const msg = messages[i]
      if (msg.role === 'user') break
      if (msg.role === 'assistant' && msg.toolCalls) {
        for (const tc of msg.toolCalls) {
          result.push({ toolCall: tc, messageId: msg.id })
        }
      }
    }
    return result
  }, [messages])
}
