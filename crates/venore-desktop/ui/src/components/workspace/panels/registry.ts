// =============================================================================
// Panel Registry - Central definition of all workspace panels
// =============================================================================
// Adding a new panel = 1 component + 1 entry here. Everything else is automatic:
// toolbar button, dock/undock/float/resize/snap-to-dock, z-index management.

import type { ComponentType } from 'react'
import type { LucideIcon } from 'lucide-react'
import type { PanelMode } from '@/stores/panelStore'
import { Layers, MessageSquare, Github, GitBranch, Hexagon } from 'lucide-react'
import { ProjectPanel } from './ProjectPanel'
import { ChatPanel } from './ChatPanel'
import { GitHubPanel } from './GitHubPanel'
import { SessionsPanel } from './SessionsPanel'
import { KnowledgePanel } from './KnowledgePanel'
import { ChatHeaderActions } from './chat/ChatHeaderActions'
import { GitHubHeaderActions } from './github/GitHubHeaderActions'
import { ProjectCollapsedPill } from './project/ProjectCollapsedPill'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface PanelContentProps {
  panelId: string
  projectPath: string
  projectId?: string
}

export interface PanelDefinition {
  id: string
  title: string               // fallback (English)
  titleKey: string             // i18n key in "common" namespace, e.g. "panels.project"
  icon: LucideIcon
  defaultSide: 'left' | 'right'
  size: {
    initialWidth: number
    minWidth: number
    maxWidth: number
    floatingHeight: number
  }
  content: ComponentType<PanelContentProps>
  headerActions?: ComponentType<PanelContentProps>
  order: number
  defaultMode?: PanelMode
  collapsedContent?: ComponentType<PanelContentProps>
}

// -----------------------------------------------------------------------------
// Registry
// -----------------------------------------------------------------------------

export const PANEL_REGISTRY: PanelDefinition[] = [
  {
    id: 'project',
    title: 'Project',
    titleKey: 'panels.project',
    icon: Layers,
    defaultSide: 'left',
    defaultMode: 'collapsed',
    collapsedContent: ProjectCollapsedPill,
    size: { initialWidth: 260, minWidth: 120, maxWidth: 500, floatingHeight: 400 },
    content: ProjectPanel,
    order: 0,
  },
  {
    id: 'github',
    title: 'GitHub',
    titleKey: 'panels.github',
    icon: Github,
    defaultSide: 'left',
    size: { initialWidth: 280, minWidth: 200, maxWidth: 450, floatingHeight: 450 },
    content: GitHubPanel,
    headerActions: GitHubHeaderActions,
    order: 2,
  },
  {
    id: 'sessions',
    title: 'Sessions',
    titleKey: 'panels.sessions',
    icon: GitBranch,
    defaultSide: 'left',
    size: { initialWidth: 280, minWidth: 200, maxWidth: 450, floatingHeight: 450 },
    content: SessionsPanel,
    order: 3,
  },
  {
    id: 'knowledge',
    title: 'Knowledge',
    titleKey: 'panels.knowledge',
    icon: Hexagon,
    defaultSide: 'left',
    size: { initialWidth: 260, minWidth: 180, maxWidth: 400, floatingHeight: 400 },
    content: KnowledgePanel,
    order: 4,
  },
  {
    id: 'chat',
    title: 'Chat',
    titleKey: 'panels.chat',
    icon: MessageSquare,
    defaultSide: 'right',
    size: { initialWidth: 340, minWidth: 240, maxWidth: 600, floatingHeight: 500 },
    content: ChatPanel,
    headerActions: ChatHeaderActions,
    order: 1,
  },
]

// -----------------------------------------------------------------------------
// Derived (computed once at load)
// -----------------------------------------------------------------------------

export const PANEL_MAP = new Map(PANEL_REGISTRY.map((def) => [def.id, def]))

export const LEFT_PANELS = PANEL_REGISTRY
  .filter((d) => d.defaultSide === 'left')
  .sort((a, b) => a.order - b.order)

export const RIGHT_PANELS = PANEL_REGISTRY
  .filter((d) => d.defaultSide === 'right')
  .sort((a, b) => a.order - b.order)
