// =============================================================================
// RAGIndexPanel - Content panel for RAG code index status and control
// =============================================================================
// Shows current index status, progress during indexing, and results after completion.
// Listens to 'rag-index-progress' Tauri events for real-time progress.

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  Database,
  Check,
  AlertCircle,
  Loader2,
  Files,
  Layers,
  Trash2,
} from 'lucide-react'
import { Card } from '../ui/card'
import { Badge } from '../ui/badge'
import { Progress } from '../ui/progress'
import { Separator } from '../ui/separator'
import { tauriApi, type IndexStatusDto, type IndexProjectResponse, type RagIndexProgressPayload } from '@/lib/tauri'
import { formatTimeAgo } from '@/lib/time'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export type RAGPanelStatus = 'idle' | 'indexing' | 'complete' | 'error'

interface RAGIndexPanelProps {
  projectPath: string
  projectId?: string
  onStatusChange?: (status: RAGPanelStatus, isIndexed: boolean) => void
  /** Triggered externally to start indexing (from footer button) */
  triggerIndex?: number
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function RAGIndexPanel({ projectPath, projectId, onStatusChange, triggerIndex }: RAGIndexPanelProps) {
  const { t } = useTranslation('workspace')

  const [status, setStatus] = useState<RAGPanelStatus>('idle')
  const [indexStatus, setIndexStatus] = useState<IndexStatusDto | null>(null)
  const [progress, setProgress] = useState<{ current: number; total: number; currentFile: string }>({ current: 0, total: 0, currentFile: '' })
  const [result, setResult] = useState<IndexProjectResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  // Propagate status changes
  const isIndexed = indexStatus?.is_indexed ?? false
  useEffect(() => {
    onStatusChange?.(status, isIndexed)
  }, [status, isIndexed, onStatusChange])

  // Load initial index status
  useEffect(() => {
    if (!projectId) {
      setLoading(false)
      return
    }
    tauriApi.getRagIndexStatus(projectId)
      .then((data) => {
        setIndexStatus(data)
        setLoading(false)
      })
      .catch((err) => {
        console.error('[RAGIndexPanel] Failed to get index status:', err)
        setLoading(false)
      })
  }, [projectId])

  // Listen for progress events
  useEffect(() => {
    let unlisten: UnlistenFn | null = null

    listen<RagIndexProgressPayload>('rag-index-progress', (event) => {
      const p = event.payload
      setProgress({ current: p.current, total: p.total, currentFile: p.current_file })

      if (p.status === 'done' || p.status === 'completed') {
        // Indexing done — refresh status
        if (projectId) {
          tauriApi.getRagIndexStatus(projectId)
            .then(setIndexStatus)
            .catch(() => {})
        }
      }
    }).then((fn) => { unlisten = fn })

    return () => { unlisten?.() }
  }, [projectId])

  // Start indexing
  const startIndexing = useCallback(async () => {
    setStatus('indexing')
    setResult(null)
    setError(null)
    setProgress({ current: 0, total: 0, currentFile: '' })

    try {
      const res = await tauriApi.indexProjectCode({ project_path: projectPath })
      setResult(res)
      setStatus('complete')
      // Refresh status after indexing
      if (projectId) {
        tauriApi.getRagIndexStatus(projectId)
          .then(setIndexStatus)
          .catch(() => {})
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      setStatus('error')
    }
  }, [projectPath, projectId])

  // Respond to external trigger
  useEffect(() => {
    if (triggerIndex && triggerIndex > 0) {
      startIndexing()
    }
  }, [triggerIndex, startIndexing])

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="w-5 h-5 animate-spin text-foreground-muted" />
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {/* Status Section */}
      {status !== 'indexing' && (
        <>
          <h3 className="text-sm font-medium text-foreground">{t('ragIndex.status')}</h3>
          {status === 'error' ? (
            <Card className="p-3 border-semantic-error/30 bg-semantic-error/5">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-md bg-background-tertiary flex items-center justify-center shrink-0">
                  <AlertCircle className="w-4 h-4 text-semantic-error" />
                </div>
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t('ragIndex.error')}</span>
                  <p className="text-xs text-foreground-muted truncate">{error}</p>
                </div>
              </div>
            </Card>
          ) : !isIndexed ? (
            <Card className="p-3 border-semantic-warning/30 bg-semantic-warning/5">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-md bg-background-tertiary flex items-center justify-center shrink-0">
                  <Database className="w-4 h-4 text-foreground-muted" />
                </div>
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">{t('ragIndex.notIndexed')}</span>
                  <p className="text-xs text-foreground-muted">{t('ragIndex.notIndexedHint')}</p>
                </div>
                <Badge variant="outline" className="text-semantic-warning border-semantic-warning/30 shrink-0">
                  {t('ragIndex.notIndexed')}
                </Badge>
              </div>
            </Card>
          ) : (
            <Card className="p-3 border-semantic-success/30 bg-semantic-success/5">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-md bg-background-tertiary flex items-center justify-center shrink-0">
                  <Database className="w-4 h-4 text-foreground-muted" />
                </div>
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">
                    {t('ragIndex.filesAndChunks', {
                      files: indexStatus!.total_files,
                      chunks: indexStatus!.total_chunks,
                    })}
                  </span>
                  <p className="text-xs text-foreground-muted">
                    {indexStatus!.last_indexed_at
                      ? t('ragIndex.lastIndexed', { time: formatTimeAgo(indexStatus!.last_indexed_at) })
                      : ''}
                  </p>
                </div>
                <Check className="w-3.5 h-3.5 text-semantic-success shrink-0" />
              </div>
            </Card>
          )}
        </>
      )}

      {/* Progress Section */}
      {status === 'indexing' && (
        <>
          <h3 className="text-sm font-medium text-foreground">{t('ragIndex.progress')}</h3>
          <Card className="p-3">
            <div className="space-y-2">
              <Progress
                value={progress.total > 0 ? (progress.current / progress.total) * 100 : 0}
                className="h-2 bg-background-tertiary"
              />
              <div className="flex items-center justify-between">
                <p className="text-xs text-foreground-muted truncate max-w-[70%]">
                  {progress.currentFile
                    ? t('ragIndex.currentFile', { file: progress.currentFile })
                    : t('ragIndex.indexing')}
                </p>
                <span className="text-xs text-foreground-muted shrink-0">
                  {progress.current} / {progress.total}
                </span>
              </div>
            </div>
          </Card>
        </>
      )}

      {/* Result Section */}
      {status === 'complete' && result && (
        <>
          <Separator />
          <h3 className="text-sm font-medium text-foreground">{t('ragIndex.lastRun')}</h3>
          <div className="flex items-center gap-3 p-3 bg-background-secondary rounded-lg">
            <div className="flex items-center gap-2 text-xs text-foreground-muted">
              <Files className="w-3.5 h-3.5" />
              <span>{t('ragIndex.indexed', { count: result.indexed })}</span>
            </div>
            <div className="flex items-center gap-2 text-xs text-foreground-muted">
              <Layers className="w-3.5 h-3.5" />
              <span>{t('ragIndex.unchanged', { count: result.skipped })}</span>
            </div>
            <div className="flex items-center gap-2 text-xs text-foreground-muted">
              <Trash2 className="w-3.5 h-3.5" />
              <span>{t('ragIndex.removed', { count: result.removed })}</span>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
