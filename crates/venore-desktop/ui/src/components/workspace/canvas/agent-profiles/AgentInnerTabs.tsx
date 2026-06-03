// =============================================================================
// AgentInnerTabs — Tab container for Pipeline / Agents / Teams / Rules / Tools / Categories / Memory
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Workflow, Bot, Users, ShieldCheck, Wrench, FolderTree, Brain, ScrollText, Layers, Puzzle } from 'lucide-react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { PipelineTab } from './PipelineTab'
import { ProfilesTab } from './ProfilesTab'
import { TeamsTab } from './TeamsTab'
import { RulesTab } from './RulesTab'
import { ToolsTab } from './ToolsTab'
import { CategoriesTab } from './CategoriesTab'
import { ModesTab } from './ModesTab'
import { FragmentsTab } from './FragmentsTab'
import { MemoryTab } from './MemoryTab'
import { PromptsView } from '../PromptsView'

interface AgentInnerTabsProps {
  projectPath?: string
  projectId?: string
}

export function AgentInnerTabs({ projectPath, projectId }: AgentInnerTabsProps) {
  const { t } = useTranslation('agents')

  return (
    <Tabs defaultValue="pipeline" className="flex-1 flex flex-col min-h-0">
      <TabsList>
        <TabsTrigger value="pipeline" className="gap-1">
          <Workflow className="w-3.5 h-3.5" />
          {t('innerTabs.pipeline')}
        </TabsTrigger>
        <TabsTrigger value="modes" className="gap-1">
          <Layers className="w-3.5 h-3.5" />
          Modes
        </TabsTrigger>
        <TabsTrigger value="profiles" className="gap-1">
          <Bot className="w-3.5 h-3.5" />
          Agents
        </TabsTrigger>
        <TabsTrigger value="teams" className="gap-1">
          <Users className="w-3.5 h-3.5" />
          {t('innerTabs.teams')}
        </TabsTrigger>
        <TabsTrigger value="rules" className="gap-1">
          <ShieldCheck className="w-3.5 h-3.5" />
          {t('innerTabs.rules')}
        </TabsTrigger>
        <TabsTrigger value="tools" className="gap-1">
          <Wrench className="w-3.5 h-3.5" />
          Tools
        </TabsTrigger>
        <TabsTrigger value="categories" className="gap-1">
          <FolderTree className="w-3.5 h-3.5" />
          Categories
        </TabsTrigger>
        <TabsTrigger value="memory" className="gap-1">
          <Brain className="w-3.5 h-3.5" />
          {t('innerTabs.memory')}
        </TabsTrigger>
        <TabsTrigger value="prompts" className="gap-1">
          <ScrollText className="w-3.5 h-3.5" />
          {t('innerTabs.prompts', 'Prompts')}
        </TabsTrigger>
        <TabsTrigger value="fragments" className="gap-1">
          <Puzzle className="w-3.5 h-3.5" />
          Fragments
        </TabsTrigger>
      </TabsList>

      {/* Tab content area — relative container so absolute children fill it */}
      <div className="flex-1 relative min-h-0">
        <TabsContent value="pipeline" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <PipelineTab />
        </TabsContent>
        <TabsContent value="modes" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ModesTab />
        </TabsContent>
        <TabsContent value="profiles" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ProfilesTab />
        </TabsContent>
        <TabsContent value="teams" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <TeamsTab />
        </TabsContent>
        <TabsContent value="rules" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <RulesTab />
        </TabsContent>
        <TabsContent value="tools" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ToolsTab />
        </TabsContent>
        <TabsContent value="categories" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <CategoriesTab />
        </TabsContent>
        <TabsContent value="memory" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <MemoryTab projectPath={projectPath} projectId={projectId} />
        </TabsContent>
        <TabsContent value="prompts" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <PromptsView />
        </TabsContent>
        <TabsContent value="fragments" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <FragmentsTab />
        </TabsContent>
      </div>
    </Tabs>
  )
}
