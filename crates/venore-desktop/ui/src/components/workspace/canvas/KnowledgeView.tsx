// =============================================================================
// KnowledgeView — Tab container for Config / Research / Activity / Report
// =============================================================================
// Follows the same pattern as AgentInnerTabs (Radix Tabs, absolute content).
// Floating hex panels render on top of the tab content area.

import { useRef } from 'react'
import { Hexagon, Microscope, Settings2, Clock, FileBarChart, Loader2 } from 'lucide-react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { ResearchGraph } from './knowledge/ResearchGraph'
import { ConfigTab } from './knowledge/ConfigTab'
import { ActivityTab } from './knowledge/ActivityTab'
import { ReportTab } from './knowledge/ReportTab'
import { ResearchFooter } from './knowledge/ResearchFooter'
import { FloatingHexPanels } from './knowledge/FloatingHexPanels'
import { AIConnectionLayer } from '@/components/ai'
import { useKnowledgeFeature } from './knowledge/useKnowledgeData'

interface KnowledgeViewProps {
  featureId: string
  projectPath?: string
  projectId?: string
}

export function KnowledgeView({ featureId, projectPath, projectId }: KnowledgeViewProps) {
  const boundsRef = useRef<HTMLDivElement>(null)
  const { feature, loading, reload } = useKnowledgeFeature(featureId)

  if (loading) {
    return (
      <div className="absolute inset-0 flex flex-col items-center justify-center text-foreground-subtle">
        <Loader2 className="w-6 h-6 mb-3 opacity-40 animate-spin" />
        <span className="text-xs">Loading feature…</span>
      </div>
    )
  }

  if (!feature) {
    return (
      <div className="absolute inset-0 flex flex-col items-center justify-center text-foreground-subtle">
        <Hexagon className="w-8 h-8 mb-3 opacity-30" />
        <span className="text-xs">Feature not found</span>
      </div>
    )
  }

  return (
    <Tabs defaultValue="config" className="absolute inset-0 flex flex-col min-h-0">
      <TabsList>
        <TabsTrigger value="config" className="gap-1">
          <Settings2 className="w-3.5 h-3.5" />
          Config
        </TabsTrigger>
        <TabsTrigger value="research" className="gap-1">
          <Microscope className="w-3.5 h-3.5" />
          Research
        </TabsTrigger>
        <TabsTrigger value="activity" className="gap-1">
          <Clock className="w-3.5 h-3.5" />
          Activity
        </TabsTrigger>
        <TabsTrigger value="report" className="gap-1">
          <FileBarChart className="w-3.5 h-3.5" />
          Report
        </TabsTrigger>
      </TabsList>

      {/* Content area — relative container for tabs + floating panels */}
      <div ref={boundsRef} className="flex-1 relative min-h-0">
        <TabsContent value="config" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ConfigTab feature={feature} onSaved={reload} />
        </TabsContent>
        <TabsContent value="research" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ResearchGraph feature={feature} />
        </TabsContent>
        <TabsContent value="activity" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ActivityTab feature={feature} />
        </TabsContent>
        <TabsContent value="report" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <ReportTab feature={feature} />
        </TabsContent>

        {/* Floating hex panels (z-40+, same layer as node panels) */}
        <FloatingHexPanels boundsRef={boundsRef} projectPath={projectPath ?? ''} />
      </div>

      {/* Footer — always visible across all tabs */}
      <ResearchFooter feature={feature} />
    </Tabs>
  )
}
