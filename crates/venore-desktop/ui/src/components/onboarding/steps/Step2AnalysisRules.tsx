// =============================================================================
// Step2AnalysisRules - Analysis rules: depth + layers + exclusions
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Gauge, Layers, FolderX, Plus, X } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Checkbox } from '@/components/ui/checkbox'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import type { AnalysisDepth, Layer } from '@/lib/wizard/types'

const DEPTH_LEVEL_VALUES: AnalysisDepth[] = ['minimal', 'normal', 'detailed', 'expert']

// `context` layer was removed from this set: it analyzed per-module
// `.context.md` files, which the new flow doesn't generate (RAG + project
// memory replace them). Keeping it on would always report "missing" and
// pollute the canvas glow color.
const LAYER_OPTIONS: { value: Layer; required?: boolean }[] = [
  { value: 'status' },
  { value: 'connections' },
  { value: 'tests' },
  { value: 'documentation' },
]

export function Step2AnalysisRules() {
  const { t } = useTranslation('wizard')
  const [newExclusion, setNewExclusion] = useState('')

  const depthLevel = useWizardDataStore((s) => s.step2.depthLevel)
  const layersToGenerate = useWizardDataStore((s) => s.step2.layersToGenerate)
  const exclusions = useWizardDataStore((s) => s.step2.exclusions)

  const setDepthLevel = useWizardDataStore((s) => s.setDepthLevel)
  const setLayersToGenerate = useWizardDataStore((s) => s.setLayersToGenerate)
  const setExclusions = useWizardDataStore((s) => s.setExclusions)

  const handleAddExclusion = () => {
    const path = newExclusion.trim()
    if (path && !exclusions.includes(path)) {
      const normalized = path.endsWith('/') ? path : `${path}/`
      setExclusions([...exclusions, normalized])
      setNewExclusion('')
    }
  }

  const handleRemoveExclusion = (path: string) => {
    setExclusions(exclusions.filter((e) => e !== path))
  }

  const handleToggleLayer = (layer: Layer) => {
    const current = layersToGenerate
    if (current.includes(layer)) {
      setLayersToGenerate(current.filter((l) => l !== layer))
    } else {
      setLayersToGenerate([...current, layer])
    }
  }

  return (
    <div className="p-6 space-y-6">
      {/* Depth level */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-2">
            <Gauge size={16} className="text-muted-foreground" />
            <CardTitle className="text-sm">{t('step2.depthLevel')}</CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          <RadioGroup value={depthLevel} onValueChange={(v) => setDepthLevel(v as AnalysisDepth)}>
            {DEPTH_LEVEL_VALUES.map((value) => (
              <label
                key={value}
                htmlFor={`depth-${value}`}
                className={`
                  flex items-start gap-3 p-3 rounded-lg cursor-pointer transition-colors
                  ${depthLevel === value
                    ? 'bg-primary/5 border border-primary/30'
                    : 'bg-accent/50 border border-transparent hover:border-border'
                  }
                `}
              >
                <RadioGroupItem value={value} id={`depth-${value}`} className="mt-0.5" />
                <div>
                  <p className="text-sm font-medium">{t(`step2.depthLevels.${value}`)}</p>
                  <p className="text-xs text-muted-foreground">{t(`step2.depthLevels.${value}Description`)}</p>
                </div>
              </label>
            ))}
          </RadioGroup>
        </CardContent>
      </Card>

      {/* Layers to generate */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-2">
            <Layers size={16} className="text-muted-foreground" />
            <CardTitle className="text-sm">{t('step2.layersToGenerate')}</CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2">
            {LAYER_OPTIONS.map(({ value, required }) => {
              const isSelected = layersToGenerate.includes(value)
              return (
                <label
                  key={value}
                  htmlFor={`layer-${value}`}
                  className={`
                    flex items-start gap-3 p-3 rounded-lg transition-colors
                    ${isSelected
                      ? 'bg-primary/5 border border-primary/30'
                      : 'bg-accent/50 border border-transparent hover:border-border'
                    }
                    ${required ? 'cursor-default' : 'cursor-pointer'}
                  `}
                >
                  <Checkbox
                    id={`layer-${value}`}
                    checked={isSelected}
                    onCheckedChange={() => !required && handleToggleLayer(value)}
                    disabled={required}
                    className="mt-0.5"
                  />
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium">{t(`step2.layers.${value}`)}</p>
                      {required && (
                        <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                          {t('step2.required')}
                        </Badge>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground">{t(`step2.layers.${value}Description`)}</p>
                  </div>
                </label>
              )
            })}
          </div>
        </CardContent>
      </Card>

      {/* Exclusions */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-2">
            <FolderX size={16} className="text-muted-foreground" />
            <CardTitle className="text-sm">{t('step2.exclusions')}</CardTitle>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex gap-2">
            <Input
              type="text"
              value={newExclusion}
              onChange={(e) => setNewExclusion(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAddExclusion()}
              placeholder={t('step2.exclusionPlaceholder')}
              className="flex-1"
            />
            <Button
              onClick={handleAddExclusion}
              disabled={!newExclusion.trim()}
              variant="outline"
              size="icon"
            >
              <Plus size={16} />
            </Button>
          </div>
          {exclusions.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {exclusions.map((path) => (
                <Badge key={path} variant="secondary" className="gap-1.5">
                  {path}
                  <button
                    onClick={() => handleRemoveExclusion(path)}
                    className="hover:text-foreground transition-colors"
                  >
                    <X size={12} />
                  </button>
                </Badge>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
