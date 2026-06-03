// =============================================================================
// SessionChangesTab — Reuses FileTree + DiffViewer from pr-detail
// =============================================================================
// SessionDiffFileDto has the same shape as GitHubPrFileDto, enabling
// structural typing reuse of FileTree and DiffViewer.

import { useTranslation } from 'react-i18next'
import { FileTree } from '../pr-detail/FileTree'
import { DiffViewer } from '../pr-detail/DiffViewer'
import type { SessionDiffFileDto } from '@/lib/tauri'

interface SessionChangesTabProps {
  files: SessionDiffFileDto[]
  selectedFile: string | null
  onSelectFile: (filename: string) => void
}

export function SessionChangesTab({ files, selectedFile, onSelectFile }: SessionChangesTabProps) {
  const { t } = useTranslation('sessions')
  const selectedFileData = files.find((f) => f.filename === selectedFile) ?? null

  if (files.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
        {t('changes.noChangesYet')}
      </div>
    )
  }

  return (
    <>
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden">
        <FileTree
          files={files}
          selectedFile={selectedFile}
          onSelectFile={onSelectFile}
        />
      </div>
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedFileData ? (
          <DiffViewer file={selectedFileData} />
        ) : (
          <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
            {t('changes.selectFile')}
          </div>
        )}
      </div>
    </>
  )
}
