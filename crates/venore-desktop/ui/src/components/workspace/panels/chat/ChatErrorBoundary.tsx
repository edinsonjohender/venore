import React from 'react'
import { useChatStore } from '@/stores/chatStore'

interface State {
  hasError: boolean
  error: string | null
}

export class ChatErrorBoundary extends React.Component<
  { children: React.ReactNode },
  State
> {
  state: State = { hasError: false, error: null }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error: error.message }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('[ChatErrorBoundary]', error, info)
    useChatStore.getState().stopStreaming()
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center h-full gap-3 p-8 text-center">
          <p className="text-sm text-muted-foreground">Something went wrong in the chat.</p>
          <button
            className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90"
            onClick={() => this.setState({ hasError: false, error: null })}
          >
            Retry
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
