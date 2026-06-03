// =============================================================================
// UpdateReportDialog - Shows affected modules and lets user trigger regeneration
// =============================================================================

import { useState } from 'react'
import { ChevronDown, ChevronRight, GitCommit, FileText } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/modal'
import { Checkbox } from '@/components/ui/checkbox'
import { useUpdaterStore } from '@/stores/updaterStore'

interface UpdateReportDialogProps {
  open: boolean
  onClose: () => void
  onRegenerate: (moduleNames: string[]) => void
}

export function UpdateReportDialog({
  open,
  onClose,
  onRegenerate,
}: UpdateReportDialogProps) {
  const { t } = useTranslation('updater')
  const report = useUpdaterStore((s) => s.updateReport)
  const selectedModules = useUpdaterStore((s) => s.selectedModules)
  const toggleModule = useUpdaterStore((s) => s.toggleModule)
  const selectAllModules = useUpdaterStore((s) => s.selectAllModules)
  const deselectAllModules = useUpdaterStore((s) => s.deselectAllModules)
  const [expandedModules, setExpandedModules] = useState<Set<string>>(new Set())

  if (!report) return null

  const toggleExpanded = (name: string) => {
    setExpandedModules((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  const allSelected = selectedModules.size === report.affected_modules.length
  const noneSelected = selectedModules.size === 0

  return (
    <Modal
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose()
      }}
      title={t('report.title')}
      description={t('report.description', {
        commits: report.commits.length,
        branch: report.commits[0]?.short_hash ?? '?',
      })}
      maxWidth="max-w-[640px]"
      footer={
        <div className="flex items-center justify-between w-full">
          <Button variant="ghost" onClick={onClose}>
            {t('report.dismiss')}
          </Button>
          <Button
            disabled={noneSelected}
            onClick={() => onRegenerate(Array.from(selectedModules))}
          >
            {t('report.regenerate', { count: selectedModules.size })}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        {/* Commits summary */}
        <div className="rounded-lg border border-border bg-background-secondary p-3">
          <div className="flex items-center gap-2 text-sm font-medium mb-2">
            <GitCommit className="h-4 w-4 text-muted-foreground" />
            {t('report.commits', { count: report.commits.length })}
          </div>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {report.commits.map((commit) => (
              <div
                key={commit.hash}
                className="flex items-center gap-2 text-xs"
              >
                <code className="rounded bg-muted px-1 py-0.5 font-mono text-muted-foreground">
                  {commit.short_hash}
                </code>
                <span className="truncate text-foreground">
                  {commit.message}
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* Module selection */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium">
              {t('report.modules', { count: report.affected_modules.length })}
            </span>
            <button
              className="text-xs text-muted-foreground hover:text-foreground transition-colors"
              onClick={allSelected ? deselectAllModules : selectAllModules}
            >
              {allSelected ? t('report.deselectAll') : t('report.selectAll')}
            </button>
          </div>
          <div className="space-y-1 max-h-64 overflow-y-auto">
            {report.affected_modules.map((mod) => {
              const isExpanded = expandedModules.has(mod.name)
              return (
                <div
                  key={mod.name}
                  className="rounded-lg border border-border bg-background-secondary"
                >
                  <div className="flex items-center gap-2 p-2">
                    <Checkbox
                      checked={selectedModules.has(mod.name)}
                      onCheckedChange={() => toggleModule(mod.name)}
                    />
                    <button
                      className="flex items-center gap-1 text-muted-foreground hover:text-foreground"
                      onClick={() => toggleExpanded(mod.name)}
                    >
                      {isExpanded ? (
                        <ChevronDown className="h-3 w-3" />
                      ) : (
                        <ChevronRight className="h-3 w-3" />
                      )}
                    </button>
                    <span className="text-sm font-medium">{mod.name}</span>
                    <span className="text-xs text-muted-foreground ml-auto">
                      {t('report.filesChanged', {
                        count: mod.changed_files.length,
                      })}
                    </span>
                  </div>
                  {isExpanded && (
                    <div className="border-t border-border px-8 py-2 space-y-0.5">
                      {mod.changed_files.map((file) => (
                        <div
                          key={file}
                          className="flex items-center gap-1.5 text-xs text-muted-foreground"
                        >
                          <FileText className="h-3 w-3" />
                          <span className="font-mono">{file}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </div>
      </div>
    </Modal>
  )
}
