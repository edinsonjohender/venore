// =============================================================================
// ChatPanel - Chat panel entry point (view switching: chat or history)
// =============================================================================

import type { PanelContentProps } from './registry'
import { useChatSessionStore } from '@/stores/chatSessionStore'
import { ChatMessages } from './chat/ChatMessages'
import { ChatInput } from './chat/ChatInput'
import { ChatSessionHistory } from './chat/ChatSessionHistory'
import { ChatSessionTabs } from './chat/ChatSessionTabs'
import { ChatErrorBoundary } from './chat/ChatErrorBoundary'

export function ChatPanel({ projectPath, projectId }: PanelContentProps) {
  const chatView = useChatSessionStore((s) => s.chatView)

  if (chatView === 'history') {
    return (
      <div className="flex flex-col h-full">
        <ChatSessionTabs projectId={projectId} projectPath={projectPath} />
        <ChatSessionHistory projectId={projectId} />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      <ChatSessionTabs projectId={projectId} projectPath={projectPath} />
      <ChatErrorBoundary>
        <ChatMessages />
        <ChatInput projectPath={projectPath} projectId={projectId} />
      </ChatErrorBoundary>
    </div>
  )
}
