// =============================================================================
// PrInnerTabs — Internal tabs for PR detail view
// =============================================================================
// Tabs: Files / Conversation

import { useState } from 'react'
import { FileCode, MessageSquare } from 'lucide-react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { FileTree } from './FileTree'
import { DiffViewer } from './DiffViewer'
import { ConversationTab } from './ConversationTab'
import type {
  GitHubPrFileDto, GitHubCommentDto, GitHubReviewCommentDto,
  GitHubPullRequestDto, GitHubPrDetailResponse,
} from '@/lib/tauri'

interface PrInnerTabsProps {
  files: GitHubPrFileDto[]
  comments: GitHubCommentDto[]
  reviewComments: GitHubReviewCommentDto[]
  projectPath: string
  prNumber: number
  prData?: GitHubPullRequestDto
  prDetail?: GitHubPrDetailResponse | null
}

export function PrInnerTabs({
  files, comments, reviewComments, projectPath, prNumber, prData, prDetail,
}: PrInnerTabsProps) {
  const [selectedFile, setSelectedFile] = useState<string | null>(
    files.length > 0 ? files[0].filename : null,
  )

  const selectedFileData = files.find((f) => f.filename === selectedFile) ?? null

  return (
    <Tabs defaultValue="files" className="flex-1 flex flex-col min-h-0">
      <TabsList>
        <TabsTrigger value="files" className="gap-1">
          <FileCode className="w-3.5 h-3.5" />
          Files
          <span className="text-foreground-muted/60 text-[10px]">({files.length})</span>
        </TabsTrigger>
        <TabsTrigger value="conversation" className="gap-1">
          <MessageSquare className="w-3.5 h-3.5" />
          Conversation
          <span className="text-foreground-muted/60 text-[10px]">({comments.length + reviewComments.length})</span>
        </TabsTrigger>
      </TabsList>

      {/* Tab content area — relative container so absolute children fill it */}
      <div className="flex-1 relative min-h-0">
        {/* Files Tab: Split layout */}
        <TabsContent value="files" className="absolute inset-0 flex data-[state=inactive]:hidden">
          <div className="w-[250px] shrink-0 border-r border-border overflow-hidden">
            <FileTree
              files={files}
              selectedFile={selectedFile}
              onSelectFile={setSelectedFile}
            />
          </div>
          <div className="flex-1 min-w-0 flex flex-col">
            {selectedFileData ? (
              <DiffViewer file={selectedFileData} />
            ) : (
              <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
                Select a file to view diff
              </div>
            )}
          </div>
        </TabsContent>

        {/* Conversation Tab */}
        <TabsContent value="conversation" className="absolute inset-0 overflow-y-auto data-[state=inactive]:hidden">
          <ConversationTab
            prData={prData}
            prDetail={prDetail}
            comments={comments}
            reviewComments={reviewComments}
          />
        </TabsContent>

      </div>
    </Tabs>
  )
}
