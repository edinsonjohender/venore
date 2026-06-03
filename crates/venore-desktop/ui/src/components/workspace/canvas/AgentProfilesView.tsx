// =============================================================================
// AgentProfilesView — AI Profiles & Agent Engine
// =============================================================================
// Main view for managing AI agent profiles, teams, and pipeline executions.

import { AgentInnerTabs } from './agent-profiles'

interface AgentProfilesViewProps {
  projectPath: string
  projectId?: string
}

export function AgentProfilesView({ projectPath, projectId }: AgentProfilesViewProps) {
  return (
    <div className="absolute inset-0 flex flex-col">
      <AgentInnerTabs projectPath={projectPath} projectId={projectId} />
    </div>
  )
}
