// =============================================================================
// OverlayAskUser - Floating overlay for ask_user tool responses
// =============================================================================

import { useState, useCallback } from 'react'
import { MessageCircleQuestion, Send } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useChatStore } from '@/stores/chatStore'
import { FloatingOverlay } from './FloatingOverlay'
import { FloatingOverlayHeader } from './FloatingOverlayHeader'

export function OverlayAskUser() {
  const { t } = useTranslation('chat')
  const pendingAskUser = useChatStore((s) => s.pendingAskUser)
  const respondToAskUser = useChatStore((s) => s.respondToAskUser)
  const [customText, setCustomText] = useState('')
  const [collapsed, setCollapsed] = useState(false)
  const handleToggle = useCallback(() => setCollapsed((c) => !c), [])

  if (!pendingAskUser) return null

  const handleOptionClick = (label: string) => {
    respondToAskUser(pendingAskUser.tool_call_id, label)
  }

  const handleCustomSubmit = () => {
    const trimmed = customText.trim()
    if (!trimmed) return
    respondToAskUser(pendingAskUser.tool_call_id, trimmed)
    setCustomText('')
  }

  return (
    <FloatingOverlay accentColor="brand">
      <FloatingOverlayHeader
        icon={MessageCircleQuestion}
        title={t('askUser.title')}
        accentColor="brand"
        isCollapsed={collapsed}
        onToggleCollapse={handleToggle}
        onClose={() => respondToAskUser(pendingAskUser.tool_call_id, '')}
      />

      {!collapsed && (
        <>
          {/* Question */}
          <div className="px-3 py-2.5">
            <p className="text-sm text-foreground">{pendingAskUser.question}</p>
          </div>

          {/* Options */}
          {pendingAskUser.options.length > 0 && (
            <div className="px-3 pb-2 flex flex-wrap gap-1.5">
              {pendingAskUser.options.map((opt) => (
                <button
                  key={opt.label}
                  type="button"
                  onClick={() => handleOptionClick(opt.label)}
                  className="px-3 py-1.5 text-xs font-medium rounded-lg border border-border bg-background-tertiary hover:bg-background-secondary hover:border-brand/40 transition-colors text-foreground"
                  title={opt.description ?? undefined}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          )}

          {/* Free-text input */}
          <div className="px-3 pb-2.5 flex gap-2">
            <input
              type="text"
              value={customText}
              onChange={(e) => setCustomText(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleCustomSubmit()}
              placeholder={t('askUser.typeResponse')}
              autoFocus
              className="flex-1 h-8 px-2.5 text-xs bg-background-tertiary border border-border rounded-lg text-foreground placeholder:text-foreground-subtle/50 outline-none focus:border-brand/50"
            />
            <button
              type="button"
              onClick={handleCustomSubmit}
              disabled={!customText.trim()}
              className="h-8 w-8 flex items-center justify-center rounded-lg bg-brand text-background disabled:opacity-30 disabled:pointer-events-none hover:bg-brand-hover transition-colors"
            >
              <Send className="w-3.5 h-3.5" />
            </button>
          </div>
        </>
      )}
    </FloatingOverlay>
  )
}
