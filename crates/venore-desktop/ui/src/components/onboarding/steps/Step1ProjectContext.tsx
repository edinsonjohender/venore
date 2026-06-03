// =============================================================================
// Step1ProjectContext - Project Context Collection
// =============================================================================

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FileText, ChevronDown, ChevronUp } from 'lucide-react'
import { Textarea } from '@/components/ui/textarea'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import type { ProjectGoal, ProjectState, TeamSize } from '@/lib/wizard/types'

const PROJECT_STATE_VALUES: ProjectState[] = ['planning', 'active', 'maintenance', 'legacy', 'archived']
const TEAM_SIZE_VALUES: TeamSize[] = ['solo', 'small', 'medium', 'large']
const OBJECTIVE_VALUES: ProjectGoal[] = ['onboarding', 'understand', 'refactor', 'document', 'audit', 'maintain']

export function Step1ProjectContext() {
  const { t } = useTranslation('wizard')
  const [showAdditional, setShowAdditional] = useState(false)

  const projectName = useWizardDataStore((s) => s.step1.name)
  const description = useWizardDataStore((s) => s.step1.description)
  const projectState = useWizardDataStore((s) => s.step1.projectState)
  const teamSize = useWizardDataStore((s) => s.step1.teamSize)
  const goals = useWizardDataStore((s) => s.step1.goals)
  const architecture = useWizardDataStore((s) => s.step1.architecture)
  const techDebt = useWizardDataStore((s) => s.step1.techDebt)

  const setDescription = useWizardDataStore((s) => s.setProjectDescription)
  const setProjectState = useWizardDataStore((s) => s.setProjectState)
  const setTeamSize = useWizardDataStore((s) => s.setTeamSize)
  const toggleGoal = useWizardDataStore((s) => s.toggleProjectGoal)
  const setArchitecture = useWizardDataStore((s) => s.setArchitecture)
  const setTechDebt = useWizardDataStore((s) => s.setTechDebt)

  const projectPath = useWizardDataStore((s) => s.step2.projectPath)

  const charCount = description.trim().length
  const isValid = charCount >= 20

  return (
    <div className="p-6 space-y-6">
      {/* Project info */}
      <Card>
        <CardContent className="flex items-start gap-3 pt-4">
          <FileText size={20} className="text-muted-foreground mt-0.5" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">{projectName}</p>
            <p className="text-xs text-muted-foreground truncate">{projectPath}</p>
          </div>
        </CardContent>
      </Card>

      {/* Main description */}
      <div className="space-y-2">
        <Label htmlFor="description">
          {t('step1.tellMeAboutProject')}
        </Label>
        <Textarea
          id="description"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder={t('step1.descriptionPlaceholder')}
          className="h-40"
        />
        <div className="flex items-center justify-between">
          <p className="text-xs text-muted-foreground">
            {t('step1.tip')}
          </p>
          <p className={`text-xs ${isValid ? 'text-muted-foreground' : 'text-yellow-600'}`}>
            {t('step1.charCount', { count: charCount })}
          </p>
        </div>
      </div>

      {/* Additional context (collapsible) */}
      <Card>
        <button
          onClick={() => setShowAdditional(!showAdditional)}
          className="w-full flex items-center justify-between px-4 py-3 hover:bg-accent transition-colors"
        >
          <span className="text-sm text-muted-foreground">
            {t('step1.additionalContext')}
          </span>
          {showAdditional ? (
            <ChevronUp size={16} className="text-muted-foreground" />
          ) : (
            <ChevronDown size={16} className="text-muted-foreground" />
          )}
        </button>

        {showAdditional && (
          <>
            <Separator />
            <CardContent className="space-y-5 pt-4">
              {/* Project state */}
              <div className="space-y-2">
                <Label className="text-xs uppercase tracking-wide">
                  {t('step1.projectState')}
                </Label>
                <div className="flex flex-wrap gap-2">
                  {PROJECT_STATE_VALUES.map((value) => (
                    <button
                      key={value}
                      onClick={() => setProjectState(value)}
                      className={`
                        px-3 py-1.5 rounded-md text-xs border transition-colors
                        ${projectState === value
                          ? 'border-primary bg-primary/10 text-primary'
                          : 'border-input bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        }
                      `}
                    >
                      {t(`step1.projectStates.${value}`)}
                    </button>
                  ))}
                </div>
              </div>

              {/* Team size */}
              <div className="space-y-2">
                <Label className="text-xs uppercase tracking-wide">
                  {t('step1.teamSize')}
                </Label>
                <div className="flex flex-wrap gap-2">
                  {TEAM_SIZE_VALUES.map((value) => (
                    <button
                      key={value}
                      onClick={() => setTeamSize(value)}
                      className={`
                        px-3 py-1.5 rounded-md text-xs border transition-colors
                        ${teamSize === value
                          ? 'border-primary bg-primary/10 text-primary'
                          : 'border-input bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        }
                      `}
                    >
                      {t(`step1.teamSizes.${value}`)}
                    </button>
                  ))}
                </div>
              </div>

              {/* Objectives */}
              <div className="space-y-2">
                <Label className="text-xs uppercase tracking-wide">
                  {t('step1.goalWithVenore')}
                </Label>
                <div className="flex flex-wrap gap-2">
                  {OBJECTIVE_VALUES.map((value) => (
                    <button
                      key={value}
                      onClick={() => toggleGoal(value)}
                      className={`
                        px-3 py-1.5 rounded-md text-xs border transition-colors
                        ${goals?.includes(value)
                          ? 'border-primary bg-primary/10 text-primary'
                          : 'border-input bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        }
                      `}
                    >
                      {t(`step1.objectives.${value}`)}
                    </button>
                  ))}
                </div>
              </div>

              {/* Architecture */}
              <div className="space-y-2">
                <Label htmlFor="architecture" className="text-xs uppercase tracking-wide">
                  {t('step1.knownArchitecture')}
                </Label>
                <Input
                  id="architecture"
                  type="text"
                  value={architecture || ''}
                  onChange={(e) => setArchitecture(e.target.value)}
                  placeholder={t('step1.architecturePlaceholder')}
                />
              </div>

              {/* Tech debt */}
              <div className="space-y-2">
                <Label htmlFor="techDebt" className="text-xs uppercase tracking-wide">
                  {t('step1.knownTechDebt')}
                </Label>
                <Input
                  id="techDebt"
                  type="text"
                  value={techDebt || ''}
                  onChange={(e) => setTechDebt(e.target.value)}
                  placeholder={t('step1.techDebtPlaceholder')}
                />
              </div>
            </CardContent>
          </>
        )}
      </Card>
    </div>
  )
}
