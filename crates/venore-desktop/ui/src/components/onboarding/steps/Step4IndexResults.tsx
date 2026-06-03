// =============================================================================
// Step4IndexResults - Show code intelligence index stats
// =============================================================================

import { useTranslation } from 'react-i18next'
import { Database, Boxes, GitBranch, FileCode, Link } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useWizardDataStore } from '@/stores/wizardDataStore'

export function Step4IndexResults() {
  const { t } = useTranslation('wizard')
  const indexResult = useWizardDataStore((s) => s.indexResult)
  const detectedModules = useWizardDataStore((s) => s.step3.detectedModules)

  if (!indexResult) {
    return (
      <div className="p-6 flex flex-col items-center justify-center min-h-[300px]">
        <Database size={40} className="text-muted-foreground mb-3" />
        <p className="text-sm text-muted-foreground">{t('step4.noIndexResults')}</p>
      </div>
    )
  }

  return (
    <div className="p-6 space-y-6">
      {/* Title */}
      <div>
        <h3 className="text-lg font-semibold">{t('step4.title')}</h3>
        <p className="text-sm text-muted-foreground mt-1">{t('step4.description')}</p>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-3 gap-4">
        <Card>
          <CardContent className="pt-6 text-center">
            <FileCode size={24} className="mx-auto mb-2 text-primary" />
            <p className="text-2xl font-bold mb-1">{indexResult.indexed + indexResult.skipped}</p>
            <p className="text-xs text-muted-foreground">{t('step4.filesIndexed')}</p>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="pt-6 text-center">
            <Boxes size={24} className="mx-auto mb-2 text-primary" />
            <p className="text-2xl font-bold mb-1">{detectedModules.length}</p>
            <p className="text-xs text-muted-foreground">{t('step4.modulesMapped')}</p>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="pt-6 text-center">
            <GitBranch size={24} className="mx-auto mb-2 text-primary" />
            <p className="text-2xl font-bold mb-1">{indexResult.depsCreated}</p>
            <p className="text-xs text-muted-foreground">{t('step4.dependenciesMapped')}</p>
          </CardContent>
        </Card>
      </div>

      {/* Additional stats */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm">{t('step4.indexDetails')}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="flex items-center gap-2">
              <Link size={14} className="text-muted-foreground" />
              <span className="text-muted-foreground">{t('step4.symbolRefs')}</span>
              <span className="font-medium">{indexResult.refsCreated}</span>
            </div>
            <div className="flex items-center gap-2">
              <FileCode size={14} className="text-muted-foreground" />
              <span className="text-muted-foreground">{t('step4.skippedFiles')}</span>
              <span className="font-medium">{indexResult.skipped}</span>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Detected Modules List */}
      {detectedModules.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">
              {t('step4.detectedModules', { count: detectedModules.length })}
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div className="max-h-[200px] overflow-y-auto">
              {detectedModules.map((mod, idx) => (
                <div
                  key={mod.id}
                  className={`flex items-center justify-between px-4 py-2 text-sm ${
                    idx !== detectedModules.length - 1 ? 'border-b border-border' : ''
                  }`}
                >
                  <div className="min-w-0">
                    <p className="truncate font-medium">{mod.name}</p>
                    <p className="text-xs text-muted-foreground truncate">{mod.path}</p>
                  </div>
                  <span className="text-xs text-muted-foreground shrink-0 ml-2">
                    {mod.fileCount} {t('step4.files')}
                  </span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
