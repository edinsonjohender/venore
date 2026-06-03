// =============================================================================
// HexHeaderActions — AI connection button for hex floating panel headers
// =============================================================================
// Same pattern as NodeHeaderActions — reuses aiConnectionStore + panelStore.

import { Sparkles } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { usePanelStore } from '@/stores/panelStore'

export function HexHeaderActions({ panelId }: { panelId: string }) {
  const isActive = useAIConnectionStore(
    (s) => s.connections[panelId]?.active ?? false,
  )
  const toggleConnection = useAIConnectionStore((s) => s.toggleConnection)
  const setMode = usePanelStore((s) => s.setMode)

  const handleClick = () => {
    toggleConnection(panelId)
    if (!isActive) {
      setMode('chat', 'docked')
    }
  }

  return (
    <div className="flex items-center" data-connection-id={panelId}>
      {isActive ? (
        <div className="rainbow-border" onClick={handleClick} title="Disconnect AI">
          <div className="flex items-center justify-center w-6 h-6 rounded-lg bg-background-secondary cursor-pointer">
            <Sparkles className="w-3.5 h-3.5 text-foreground" />
          </div>
        </div>
      ) : (
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6"
          onClick={handleClick}
          title="Connect to AI"
        >
          <Sparkles className="w-3.5 h-3.5" />
        </Button>
      )}
    </div>
  )
}
