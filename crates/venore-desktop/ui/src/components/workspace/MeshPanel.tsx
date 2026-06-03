// =============================================================================
// MeshPanel — Floating panel for mesh peer discovery and connection
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Network, RefreshCw, Loader2, AlertTriangle } from 'lucide-react'
import { useFloatingPanel } from '@/hooks/useFloatingPanel'
import { FloatingPanelWrapper } from './FloatingPanelWrapper'
import { useMeshStore } from '@/stores/meshStore'
import { cn } from '@/lib/utils'
import type { MeshPeerInfo } from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Outer wrapper — only renders when panel is open
// -----------------------------------------------------------------------------

interface MeshPanelProps {
  canvasZoneRef: React.RefObject<HTMLDivElement | null>
}

export function MeshPanel({ canvasZoneRef }: MeshPanelProps) {
  const panelOpen = useMeshStore((s) => s.panelOpen)
  if (!panelOpen) return null
  return <MeshPanelInner canvasZoneRef={canvasZoneRef} />
}

// -----------------------------------------------------------------------------
// Inner panel — floating position, data flows from backend events → store → UI
// -----------------------------------------------------------------------------

function MeshPanelInner({ canvasZoneRef }: MeshPanelProps) {
  const { t } = useTranslation('workspace')
  const { position, size, handleDragStart, handleResizeStart } = useFloatingPanel({
    initialSize: { width: 300, height: 350 },
    initialPosition: { x: 60, y: 60 },
    boundsRef: canvasZoneRef,
  })

  const peers = useMeshStore((s) => s.peers)
  const connectedPeerIds = useMeshStore((s) => s.connectedPeerIds)
  const connectingPeerId = useMeshStore((s) => s.connectingPeerId)
  const transportRunning = useMeshStore((s) => s.transportRunning)
  const transportPort = useMeshStore((s) => s.transportPort)
  const error = useMeshStore((s) => s.error)
  const refreshPeers = useMeshStore((s) => s.refreshPeers)
  const refreshStatus = useMeshStore((s) => s.refreshStatus)
  const connectPeer = useMeshStore((s) => s.connectPeer)
  const disconnectPeer = useMeshStore((s) => s.disconnectPeer)
  const togglePanel = useMeshStore((s) => s.togglePanel)
  const setError = useMeshStore((s) => s.setError)

  return (
    <FloatingPanelWrapper
      title={t('mesh.title')}
      icon={<Network className="w-3.5 h-3.5" />}
      headerActions={
        <div className="flex items-center gap-1">
          <button
            onClick={() => { refreshPeers(); refreshStatus() }}
            className="p-0.5 text-foreground-muted hover:text-foreground transition-colors"
            title={t('mesh.refresh')}
          >
            <RefreshCw className="w-3 h-3" />
          </button>
        </div>
      }
      left={position.x}
      top={position.y}
      width={size.width}
      height={size.height}
      zIndex={50}
      hideDock
      onFocus={() => {}}
      onDragStart={handleDragStart}
      onResizeStart={handleResizeStart}
      onDock={() => {}}
      onClose={togglePanel}
    >
      <div className="flex flex-col h-full">
        {/* Error banner */}
        {error && (
          <div className="px-3 py-1.5 bg-red-500/10 border-b border-red-500/20 flex items-center gap-1.5">
            <AlertTriangle className="w-3 h-3 text-red-400 shrink-0" />
            <span className="text-[10px] text-red-400 truncate">{error}</span>
            <button
              onClick={() => setError(null)}
              className="ml-auto text-[10px] text-red-400 hover:text-red-300 shrink-0"
            >
              &times;
            </button>
          </div>
        )}

        {/* Status bar */}
        <div className="px-3 py-2 border-b border-border text-[11px] text-foreground-muted">
          {transportRunning ? (
            <span>
              {t('mesh.transportRunning', { port: transportPort })}
            </span>
          ) : (
            <span>{t('mesh.transportStopped')}</span>
          )}
        </div>

        {/* Peer list */}
        <div className="flex-1 overflow-y-auto">
          {peers.length === 0 ? (
            <div className="flex items-center justify-center h-full text-[11px] text-foreground-subtle">
              {t('mesh.noPeersFound')}
            </div>
          ) : (
            <div className="py-1">
              {peers.map((peer) => (
                <PeerRow
                  key={peer.project_id}
                  peer={peer}
                  isConnected={connectedPeerIds.includes(peer.project_id)}
                  isConnecting={connectingPeerId === peer.project_id}
                  onConnect={() => connectPeer(peer.project_id)}
                  onDisconnect={() => disconnectPeer(peer.project_id)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </FloatingPanelWrapper>
  )
}

// -----------------------------------------------------------------------------
// PeerRow — Single discovered peer
// -----------------------------------------------------------------------------

function PeerRow({
  peer,
  isConnected,
  isConnecting,
  onConnect,
  onDisconnect,
}: {
  peer: MeshPeerInfo
  isConnected: boolean
  isConnecting: boolean
  onConnect: () => void
  onDisconnect: () => void
}) {
  const { t } = useTranslation('workspace')
  const isBusy = isConnecting
  const profile = peer.profile

  return (
    <div className="px-3 py-2 hover:bg-background-tertiary transition-colors">
      <div className="flex items-center justify-between">
        <div className="min-w-0 flex-1">
          {/* Row 1: name + language badge */}
          <div className="flex items-center gap-1.5">
            {isConnected && (
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0" />
            )}
            <span
              className="text-xs font-medium text-foreground truncate"
              title={profile?.description ?? undefined}
            >
              {peer.project_name}
            </span>
            {profile?.language && (
              <span className="text-[9px] px-1 py-px bg-background-secondary border border-border rounded shrink-0">
                {profile.language}
              </span>
            )}
          </div>

          {/* Row 2: technologies + module count (only when profile exists) */}
          {profile && profile.technologies.length > 0 && (
            <div className="flex items-center gap-1 mt-0.5">
              {profile.technologies.slice(0, 3).map((tech) => (
                <span
                  key={tech}
                  className="text-[9px] px-1 py-px bg-background-tertiary rounded text-foreground-muted"
                >
                  {tech}
                </span>
              ))}
              {profile.technologies.length > 3 && (
                <span className="text-[9px] text-foreground-subtle">
                  +{profile.technologies.length - 3}
                </span>
              )}
              {profile.total_modules > 0 && (
                <span className="text-[9px] text-foreground-subtle ml-auto">
                  {t('mesh.moduleCount', { count: profile.total_modules })}
                </span>
              )}
            </div>
          )}

          {/* Row 3: path */}
          <div className="text-[10px] text-foreground-subtle truncate mt-0.5">
            {peer.project_path}
          </div>
        </div>
        <button
          onClick={isConnected ? onDisconnect : onConnect}
          disabled={isBusy}
          className={cn(
            'shrink-0 ml-2 px-2 py-0.5 text-[10px] rounded transition-colors',
            isConnected
              ? 'text-foreground-muted hover:text-red-400 hover:bg-red-500/10 cursor-pointer'
              : isBusy
                ? 'text-foreground-subtle cursor-wait'
                : 'text-foreground-muted hover:text-foreground hover:bg-background-secondary cursor-pointer',
          )}
        >
          {isConnecting ? (
            <span className="flex items-center gap-1">
              <Loader2 className="w-2.5 h-2.5 animate-spin" />
              {t('mesh.connecting')}
            </span>
          ) : isConnected ? (
            t('mesh.disconnect')
          ) : (
            t('mesh.connect')
          )}
        </button>
      </div>
    </div>
  )
}
