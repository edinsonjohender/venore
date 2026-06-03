// =============================================================================
// SessionInnerTabs — Tabs: Changes | Summary | Activity
// =============================================================================

import { useTranslation } from 'react-i18next'
import { FileCode, FileText, Activity } from 'lucide-react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { SessionChangesTab } from './SessionChangesTab'
import { SessionSummaryTab } from './SessionSummaryTab'
import { SessionActivityTab } from './SessionActivityTab'
import type { SessionDto, SessionDiffFileDto, SessionActivityDto } from '@/lib/tauri'

interface SessionInnerTabsProps {
  files: SessionDiffFileDto[]
  session: SessionDto
  activity: SessionActivityDto | null
  selectedFile: string | null
  onSelectFile: (filename: string) => void
  onRevert?: (commitHash: string, toolCallId: string) => void
}

export function SessionInnerTabs({ files, session, activity, selectedFile, onSelectFile, onRevert }: SessionInnerTabsProps) {
  const { t } = useTranslation('sessions')

  const toolCallCount = activity?.tool_calls.length ?? 0

  return (
    <Tabs defaultValue="changes" className="flex-1 flex flex-col min-h-0">
      <TabsList>
        <TabsTrigger value="changes" className="gap-1">
          <FileCode className="w-3.5 h-3.5" />
          {t('innerTabs.changes')}
          <span className="text-foreground-muted/60 text-[10px]">({files.length})</span>
        </TabsTrigger>
        <TabsTrigger value="summary" className="gap-1">
          <FileText className="w-3.5 h-3.5" />
          {t('innerTabs.summary')}
        </TabsTrigger>
        <TabsTrigger value="activity" className="gap-1">
          <Activity className="w-3.5 h-3.5" />
          {t('innerTabs.activity')}
          {toolCallCount > 0 && (
            <span className="text-foreground-muted/60 text-[10px]">({toolCallCount})</span>
          )}
        </TabsTrigger>
      </TabsList>

      <div className="flex-1 relative min-h-0">
        <TabsContent value="changes" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <SessionChangesTab
            files={files}
            selectedFile={selectedFile}
            onSelectFile={onSelectFile}
          />
        </TabsContent>

        <TabsContent value="summary" className="absolute inset-0 overflow-y-auto data-[state=inactive]:hidden">
          <SessionSummaryTab session={session} />
        </TabsContent>

        <TabsContent value="activity" className="absolute inset-0 overflow-y-auto data-[state=inactive]:hidden">
          {activity ? (
            <SessionActivityTab activity={activity} onRevert={onRevert} />
          ) : (
            <div className="flex items-center justify-center h-full text-foreground-muted/40 text-xs">
              Loading...
            </div>
          )}
        </TabsContent>
      </div>
    </Tabs>
  )
}
