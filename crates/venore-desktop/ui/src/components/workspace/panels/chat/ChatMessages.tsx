// =============================================================================
// ChatMessages - Scrollable message list with smart auto-scroll
// =============================================================================

import { useRef, useEffect, useCallback } from 'react'
import { useChatStore } from '@/stores/chatStore'
import { ChatMessage } from './ChatMessage'
import { ChatEmptyState } from './ChatEmptyState'
import { AlertCircle } from 'lucide-react'
import { getErrorInfo } from '@/lib/error-messages'

export function ChatMessages() {
  const messages = useChatStore((s) => s.messages)
  const error = useChatStore((s) => s.error)
  const containerRef = useRef<HTMLDivElement>(null)
  const bottomRef = useRef<HTMLDivElement>(null)
  const isNearBottomRef = useRef(true)

  const handleScroll = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    const threshold = 100
    isNearBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < threshold
  }, [])

  useEffect(() => {
    if (isNearBottomRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages])

  if (messages.length === 0) {
    return <ChatEmptyState />
  }

  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      className="flex-1 overflow-y-auto select-text"
    >
      <div className="flex flex-col gap-5 px-4 py-4">
        {messages.map((msg) => (
          <ChatMessage key={msg.id} message={msg} />
        ))}

        {/* Error banner */}
        {error && (() => {
          const info = getErrorInfo(error.code, error.message)
          return (
            <div className="flex items-start gap-2 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
              <AlertCircle className="w-4 h-4 text-red-400 mt-0.5 shrink-0" />
              <div className="flex flex-col gap-0.5">
                <span className="text-xs text-red-400">{info.message}</span>
                {info.suggestion && (
                  <span className="text-xs text-red-400/70">{info.suggestion}</span>
                )}
              </div>
            </div>
          )
        })()}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}
