// =============================================================================
// WorkspaceLayout - Main layout container for the workspace
// =============================================================================
// Flex row: [left panels] + [canvas + toolbar + floating panels] + [right panels]
// Panel definitions live in the registry — this file just loops over them.
// Each PanelSlot calls usePanelInstance once; DockedView/FloatingView are pure renderers.

import { useRef, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { WorkspaceCanvas } from './WorkspaceCanvas'
import { WorkspaceToolbar } from './WorkspaceToolbar'
import { ActivityBar } from './ActivityBar'
import { CanvasHeader } from './CanvasHeader'
import { TerminalPanel } from './TerminalPanel'
import { AIConnectionLayer } from '@/components/ai'
import { WorkspacePanel } from './WorkspacePanel'
import { FloatingPanelWrapper } from './FloatingPanelWrapper'
import { FloatingNodePanels } from './FloatingNodePanels'
import { MeshPanel } from './MeshPanel'
import { GitHubPrDetailView } from './canvas/GitHubPrDetailView'
import { GitHubIssueDetailView } from './canvas/GitHubIssueDetailView'
import { FileEditorView } from './canvas/FileEditorView'
import { AgentProfilesView } from './canvas/AgentProfilesView'
import { SessionDetailView } from './canvas/SessionDetailView'
import { KnowledgeView } from './canvas/KnowledgeView'
import { ResizeHandle } from '@/components/ui/resize-handle'
import { usePanelInstance } from '@/hooks/usePanelInstance'
import { useAnimatedPresence, type AnimPhase } from '@/hooks/useAnimatedPresence'
import { startPanelAnim, endPanelAnim } from './panel-anim-signal'
import { useCanvasTabStore } from '@/stores/canvasTabStore'
import { LEFT_PANELS, RIGHT_PANELS, PANEL_REGISTRY } from './panels'
import type { PanelDefinition } from './panels'
import { useWorkspaceFeatureStore, FEATURE_MATRIX } from '@/stores/workspaceFeatureStore'
import type { FeatureId } from '@/stores/workspaceFeatureStore'

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const DOCKED_DURATION = 200
const FLOATING_DURATION = 150
const COLLAPSED_DURATION = 150

const DURATIONS: Record<string, number> = {
  docked: DOCKED_DURATION,
  floating: FLOATING_DURATION,
  collapsed: COLLAPSED_DURATION,
}

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface WorkspaceLayoutProps {
  projectPath: string
  projectId?: string
}

type PanelInstanceReturn = ReturnType<typeof usePanelInstance>

// -----------------------------------------------------------------------------
// PanelSlot - One hook instance per panel, renders in correct position
// -----------------------------------------------------------------------------

function PanelSlot({
  def,
  projectPath,
  projectId,
  canvasZoneRef,
  renderMode,
}: {
  def: PanelDefinition
  projectPath: string
  projectId?: string
  canvasZoneRef: React.RefObject<HTMLDivElement | null>
  renderMode: 'docked' | 'floating' | 'collapsed'
}) {
  const panel = usePanelInstance({ def, canvasZoneRef })

  const isPresent = renderMode === 'collapsed'
    ? panel.mode === 'collapsed' && !!def.collapsedContent
    : panel.mode === renderMode

  // Only docked panels signal the canvas to switch frameloop during animation.
  // Floating/collapsed are absolute overlays that don't affect canvas size.
  const animOptions = useMemo(
    () => renderMode === 'docked' ? { onAnimStart: startPanelAnim, onAnimEnd: endPanelAnim } : undefined,
    [renderMode],
  )

  const { shouldRender, phase } = useAnimatedPresence(isPresent, DURATIONS[renderMode], animOptions)

  if (!shouldRender) return null

  if (renderMode === 'collapsed') {
    const CollapsedContent = def.collapsedContent!
    const isVisible = phase === 'entering' || phase === 'idle'

    return (
      <div
        className="absolute inset-0 pointer-events-none [&>*]:pointer-events-auto"
        style={{
          opacity: isVisible ? 1 : 0,
          transform: isVisible ? 'translateY(0)' : 'translateY(-4px)',
          transition: phase !== 'idle'
            ? `opacity ${COLLAPSED_DURATION}ms ease, transform ${COLLAPSED_DURATION}ms ease`
            : undefined,
        }}
      >
        <CollapsedContent panelId={def.id} projectPath={projectPath} projectId={projectId} />
      </div>
    )
  }

  if (renderMode === 'docked') {
    return <DockedView def={def} projectPath={projectPath} projectId={projectId} panel={panel} phase={phase} />
  }

  return <FloatingView def={def} projectPath={projectPath} projectId={projectId} panel={panel} phase={phase} />
}

// -----------------------------------------------------------------------------
// DockedView - CSS width-animated docked panel
// -----------------------------------------------------------------------------
// Width animates via CSS transition. Canvas sync is handled by SyncResize
// inside the R3F Canvas (bypasses the async ResizeObserver).
// During idle, transition is OFF so resize drag works without lag.

function DockedView({
  def,
  projectPath,
  projectId,
  panel,
  phase,
}: {
  def: PanelDefinition
  projectPath: string
  projectId?: string
  panel: PanelInstanceReturn
  phase: AnimPhase
}) {
  const { t } = useTranslation()
  const Content = def.content
  const HeaderActions = def.headerActions
  const Icon = def.icon

  const targetWidth = panel.resizable.width + 4 // +4 for resize handle
  const isCollapsed = phase === 'pre-enter' || phase === 'exiting'
  const isAnimating = phase === 'entering' || phase === 'exiting'

  return (
    <div
      className="shrink-0 flex overflow-hidden"
      style={{
        width: isCollapsed ? 0 : targetWidth,
        transition: isAnimating ? `width ${DOCKED_DURATION}ms ease-out` : undefined,
      }}
    >
      {def.defaultSide === 'right' && (
        <ResizeHandle onDrag={panel.resizable.handleDrag} onDragEnd={panel.resizable.handleDragEnd} />
      )}
      <div
        className={cn('shrink-0', panel.resizable.closePending && 'opacity-40 transition-opacity')}
        style={{ width: panel.resizable.width }}
      >
        <WorkspacePanel
          title={t(def.titleKey)}
          icon={<Icon className="w-3.5 h-3.5" />}
          headerActions={HeaderActions && <HeaderActions panelId={def.id} projectPath={projectPath} projectId={projectId} />}
          onClose={panel.close}
          onUndock={() => panel.undock()}
          onUndockDrag={(mx, my) => panel.undock(mx, my)}
        >
          <Content panelId={def.id} projectPath={projectPath} projectId={projectId} />
        </WorkspacePanel>
      </div>
      {def.defaultSide === 'left' && (
        <ResizeHandle onDrag={panel.resizable.handleDrag} onDragEnd={panel.resizable.handleDragEnd} />
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// FloatingView - Animated renderer for floating panel
// -----------------------------------------------------------------------------

function FloatingView({
  def,
  projectPath,
  projectId,
  panel,
  phase,
}: {
  def: PanelDefinition
  projectPath: string
  projectId?: string
  panel: PanelInstanceReturn
  phase: AnimPhase
}) {
  const { t } = useTranslation()
  const Content = def.content
  const HeaderActions = def.headerActions
  const Icon = def.icon
  const isVisible = phase === 'entering' || phase === 'idle'

  const animStyle: React.CSSProperties = {
    opacity: isVisible ? 1 : 0,
    transform: isVisible ? 'scale(1)' : 'scale(0.96)',
    transition: phase !== 'idle'
      ? `opacity ${FLOATING_DURATION}ms ease, transform ${FLOATING_DURATION}ms ease`
      : undefined,
  }

  return (
    <FloatingPanelWrapper
      title={t(def.titleKey)}
      icon={<Icon className="w-3.5 h-3.5" />}
      headerActions={HeaderActions && <HeaderActions panelId={def.id} projectPath={projectPath} projectId={projectId} />}
      left={panel.floating.position.x}
      top={panel.floating.position.y}
      width={panel.floating.size.width}
      height={panel.floating.size.height}
      zIndex={panel.zIndex}
      onFocus={panel.focus}
      onDragStart={panel.floating.handleDragStart}
      onResizeStart={panel.floating.handleResizeStart}
      onDock={panel.dock}
      onClose={panel.close}
      animStyle={animStyle}
    >
      <Content panelId={def.id} projectPath={projectPath} projectId={projectId} />
    </FloatingPanelWrapper>
  )
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function WorkspaceLayout({ projectPath, projectId }: WorkspaceLayoutProps) {
  const canvasZoneRef = useRef<HTMLDivElement>(null)
  const projectType = useWorkspaceFeatureStore((s) => s.projectType)
  const features = FEATURE_MATRIX[projectType]

  const activeTab = useCanvasTabStore((s) => {
    const id = s.activeTabId
    return s.tabs.find((t) => t.id === id) ?? s.tabs[0]
  })

  // Filter panels by enabled features
  const leftPanels = useMemo(
    () => LEFT_PANELS.filter((d) => features.has(d.id as FeatureId)),
    [features],
  )
  const rightPanels = useMemo(
    () => RIGHT_PANELS.filter((d) => features.has(d.id as FeatureId)),
    [features],
  )
  const allPanels = useMemo(
    () => PANEL_REGISTRY.filter((d) => features.has(d.id as FeatureId)),
    [features],
  )

  const oceanEnabled = features.has('ocean')
  const isOcean = activeTab.type === 'ocean' && oceanEnabled

  // Use session worktree path when viewing a session tab, otherwise project root
  const effectiveCwd = (activeTab.type === 'session' && activeTab.data?.worktreePath)
    ? activeTab.data.worktreePath
    : projectPath

  return (
    <div className="flex-1 flex flex-row overflow-hidden">
      {/* Activity Bar — icon sidebar */}
      <ActivityBar />

      {/* Left docked panels */}
      {leftPanels.map((def) => (
        <PanelSlot
          key={def.id}
          def={def}
          projectPath={projectPath}
          projectId={projectId}
          canvasZoneRef={canvasZoneRef}
          renderMode="docked"
        />
      ))}

      {/* Canvas zone (canvas + collapsed pills + floating panels) */}
      <div ref={canvasZoneRef} className="flex-1 flex flex-col relative overflow-hidden">
        <CanvasHeader />
        {/* AI connection lines — global, works across all tabs (z-39) */}
        <AIConnectionLayer />
        <div className="flex-1 flex relative overflow-hidden">
          {isOcean ? (
            <>
              <WorkspaceCanvas projectPath={projectPath}>
                <WorkspaceToolbar />
              </WorkspaceCanvas>

              {/* Collapsed pills (z-15, above canvas, below toolbar z-20) */}
              {allPanels.map((def) => (
                <PanelSlot
                  key={`${def.id}-collapsed`}
                  def={def}
                  projectPath={projectPath}
                  canvasZoneRef={canvasZoneRef}
                  renderMode="collapsed"
                />
              ))}

              {/* Floating panels (absolute overlays, z-30+) */}
              {allPanels.map((def) => (
                <PanelSlot
                  key={`${def.id}-floating`}
                  def={def}
                  projectPath={projectPath}
                  canvasZoneRef={canvasZoneRef}
                  renderMode="floating"
                />
              ))}

              {/* Floating node panels (multi-instance, z-40+) */}
              <FloatingNodePanels canvasZoneRef={canvasZoneRef} />
            </>
          ) : activeTab.type === 'ocean' && !oceanEnabled ? (
            /* Ocean tab active but feature disabled — empty placeholder */
            <div className="flex-1 flex items-center justify-center" />
          ) : activeTab.type === 'knowledge' && activeTab.data?.featureId ? (
            <KnowledgeView featureId={activeTab.data.featureId} projectPath={projectPath} projectId={projectId} />
          ) : activeTab.type === 'pr' ? (
            <GitHubPrDetailView
              number={activeTab.data!.number!}
              title={activeTab.data!.title!}
              projectPath={projectPath}
            />
          ) : activeTab.type === 'issue' ? (
            <GitHubIssueDetailView
              number={activeTab.data!.number!}
              title={activeTab.data!.title!}
              projectPath={projectPath}
            />
          ) : activeTab.type === 'file' && features.has('file') ? (
            <FileEditorView
              relativePath={activeTab.data!.relativePath!}
              projectPath={projectPath}
              tabId={activeTab.id}
            />
          ) : activeTab.type === 'ai' ? (
            <AgentProfilesView projectPath={projectPath} projectId={projectId} />
          ) : activeTab.type === 'session' && features.has('session') ? (
            <SessionDetailView
              sessionId={activeTab.data!.sessionId!}
              projectPath={projectPath}
              projectId={projectId}
            />
          ) : null}

          {/* Mesh peer panel (z-50, renders on any tab) */}
          <MeshPanel canvasZoneRef={canvasZoneRef} />
        </div>

        {/* Terminal docked at bottom — animated height */}
        <TerminalPanel projectPath={effectiveCwd} />
      </div>

      {/* Right docked panels */}
      {rightPanels.map((def) => (
        <PanelSlot
          key={def.id}
          def={def}
          projectPath={projectPath}
          projectId={projectId}
          canvasZoneRef={canvasZoneRef}
          renderMode="docked"
        />
      ))}
    </div>
  )
}
