// =============================================================================
// ChatHeaderActions - Global action buttons for the chat panel header
// =============================================================================

import { SquarePen, History } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useChatSessionStore } from '@/stores/chatSessionStore'
import { cn } from '@/lib/utils'
import type { PanelContentProps } from '../registry'

export function ChatHeaderActions({ projectId }: PanelContentProps) {
  const { t } = useTranslation('chat')
  const chatView = useChatSessionStore((s) => s.chatView)
  const setChatView = useChatSessionStore((s) => s.setChatView)
  const getOrCreateEmptySession = useChatSessionStore((s) => s.getOrCreateEmptySession)

  return (
    <div className="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6"
        onClick={async () => { setChatView('chat'); await getOrCreateEmptySession(projectId) }}
        title={t('headerActions.newChat')}
      >
        <SquarePen className="w-3.5 h-3.5" />
      </Button>

      <Button
        variant="ghost"
        size="icon"
        className={cn('h-6 w-6', chatView === 'history' && 'bg-background-tertiary')}
        onClick={() => setChatView(chatView === 'history' ? 'chat' : 'history')}
        title={t('headerActions.history')}
      >
        <History className="w-3.5 h-3.5" />
      </Button>
    </div>
  )
}
