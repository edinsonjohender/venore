// =============================================================================
// Settings sections — declarative config
// =============================================================================
// Adding a new section is two steps:
//   1. Extend `SettingsSectionId` with the new id (compile-time enum).
//   2. Push an entry into SETTINGS_SECTIONS with its component + icon.
// Placeholder sections render a `ComingSoonPanel` and carry a "Soon" badge —
// safe to wire UI affordances against them without scaffolding the real tab.

import { Brain, Server, Puzzle, Users, Bot } from 'lucide-react'
import type { SidebarSection } from '../ui/SidebarModal'

import { AIConfigPanel } from '../ai-config/AIConfigPanel'
import { ComingSoonPanel } from './ComingSoonPanel'

export type SettingsSectionId =
  | 'ai-providers'
  | 'mcp-servers'
  | 'integrations'
  | 'collaboration'
  | 'veronica'

export const SETTINGS_SECTIONS: SidebarSection<SettingsSectionId>[] = [
  {
    id: 'ai-providers',
    label: 'AI Providers',
    icon: Brain,
    component: AIConfigPanel,
    description: 'Configure LLM providers and API keys',
  },
  {
    id: 'mcp-servers',
    label: 'MCP Servers',
    icon: Server,
    component: ComingSoonPanel,
    description: 'Manage Model Context Protocol servers',
    badge: 'Soon',
  },
  {
    id: 'integrations',
    label: 'Integrations',
    icon: Puzzle,
    component: ComingSoonPanel,
    description: 'Connect external services (GitHub, Linear, ...)',
    badge: 'Soon',
  },
  {
    id: 'collaboration',
    label: 'Collaboration',
    icon: Users,
    component: ComingSoonPanel,
    description: 'Roles, permissions, and team membership',
    badge: 'Soon',
  },
  {
    id: 'veronica',
    label: 'Veronica AI',
    icon: Bot,
    component: ComingSoonPanel,
    description: 'Tune Veronica\'s tone, depth, and assistant behavior',
    badge: 'Soon',
  },
]

export const DEFAULT_SETTINGS_SECTION: SettingsSectionId = 'ai-providers'
