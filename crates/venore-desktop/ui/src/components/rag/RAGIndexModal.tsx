// =============================================================================
// RAGIndexModal - Modal wrapper for RAG code index management
// =============================================================================
// Follows AIConfigModal pattern. Shows index status, allows triggering
// indexing, and displays real-time progress.

import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Database, Loader2 } from 'lucide-react'
import { Modal } from '../ui/modal'
import { Button } from '../ui/button'
import { RAGIndexPanel, type RAGPanelStatus } from './RAGIndexPanel'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface RAGIndexModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  projectPath: string
  projectId?: string
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function RAGIndexModal({ open, onOpenChange, projectPath, projectId }: RAGIndexModalProps) {
  const { t } = useTranslation('workspace')
  const [panelStatus, setPanelStatus] = useState<RAGPanelStatus>('idle')
  const [isIndexed, setIsIndexed] = useState(false)
  const [triggerIndex, setTriggerIndex] = useState(0)

  const handleStatusChange = useCallback((s: RAGPanelStatus, indexed: boolean) => {
    setPanelStatus(s)
    setIsIndexed(indexed)
  }, [])

  const handleAction = () => {
    if (panelStatus === 'complete') {
      onOpenChange(false)
      return
    }
    // Trigger indexing via counter bump
    setTriggerIndex((n) => n + 1)
  }

  const handleClose = () => {
    onOpenChange(false)
  }

  // Determine footer button label and state
  const isIndexing = panelStatus === 'indexing'
  const isComplete = panelStatus === 'complete'
  const isError = panelStatus === 'error'

  let actionLabel: string
  if (isIndexing) actionLabel = t('ragIndex.indexing')
  else if (isComplete) actionLabel = t('ragIndex.done')
  else if (isError) actionLabel = t('ragIndex.retry')
  else if (isIndexed) actionLabel = t('ragIndex.reIndex')
  else actionLabel = t('ragIndex.indexNow')

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      icon={<Database className="w-4 h-4 text-foreground-muted" />}
      title={t('ragIndex.title')}
      description={t('ragIndex.description')}
      footer={
        <>
          <Button variant="ghost" onClick={handleClose}>
            {t('ragIndex.cancel')}
          </Button>
          <Button onClick={handleAction} disabled={isIndexing}>
            {isIndexing && <Loader2 className="w-3.5 h-3.5 mr-1.5 animate-spin" />}
            {actionLabel}
          </Button>
        </>
      }
    >
      <RAGIndexPanel
        projectPath={projectPath}
        projectId={projectId}
        onStatusChange={handleStatusChange}
        triggerIndex={triggerIndex}
      />
    </Modal>
  )
}
