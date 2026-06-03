// =============================================================================
// TeamsTab — Team list + detail editor (CRUD)
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Users, Loader2, AlertCircle, Plus, Trash2, Save } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { Checkbox } from '@/components/ui/checkbox'
import { tauriApi } from '@/lib/tauri'
import { STAGE_COLORS } from './types'
import type { AgentTeam, AgentProfile, AgentStage } from './types'

// -----------------------------------------------------------------------------
// TeamListItem
// -----------------------------------------------------------------------------

function TeamListItem({
  team, isSelected, onSelect,
}: {
  team: AgentTeam
  isSelected: boolean
  onSelect: () => void
}) {
  const { t } = useTranslation('agents')

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left px-3 py-2.5 border-b border-border/30 transition-colors',
        isSelected
          ? 'bg-background-tertiary border-l-2 border-l-brand'
          : 'hover:bg-background-tertiary/50 border-l-2 border-l-transparent',
      )}
    >
      <div className="flex items-center gap-2 mb-1">
        <Users className="w-3.5 h-3.5 text-foreground-muted shrink-0" />
        <span className="text-xs font-medium text-foreground truncate flex-1">
          {team.name || t('teams.untitled')}
        </span>
      </div>
      <div className="text-[10px] text-foreground-muted/60 pl-[22px]">
        {t('teams.agentCount', { count: team.profileIds.length })}
      </div>
    </button>
  )
}

// -----------------------------------------------------------------------------
// FieldLabel
// -----------------------------------------------------------------------------

function FieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <Label className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
      {children}
    </Label>
  )
}

// -----------------------------------------------------------------------------
// TeamDetail
// -----------------------------------------------------------------------------

function TeamDetail({
  team, allProfiles, onUpdate, onDelete,
}: {
  team: AgentTeam
  allProfiles: AgentProfile[]
  onUpdate: (draft: AgentTeam) => void
  onDelete: (id: string) => void
}) {
  const { t } = useTranslation('agents')
  const [draft, setDraft] = useState<AgentTeam>(team)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  useEffect(() => {
    setDraft(team)
    setSaving(false)
    setConfirmDelete(false)
  }, [team.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const patch = useCallback(<K extends keyof AgentTeam>(key: K, value: AgentTeam[K]) => {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }, [])

  const isDraft = team.id.startsWith('draft-')
  const isDirty = isDraft || JSON.stringify(draft) !== JSON.stringify(team)

  const handleSave = async () => {
    if (!isDirty || saving) return
    setSaving(true)
    await onUpdate(draft)
    setSaving(false)
  }

  const handleDeleteClick = () => {
    if (team.isTemplate) return
    if (confirmDelete) {
      setConfirmDelete(false)
      onDelete(team.id)
    } else {
      setConfirmDelete(true)
      setTimeout(() => setConfirmDelete(false), 2000)
    }
  }

  const toggleMember = (profileId: string) => {
    setDraft((prev) => {
      const has = prev.profileIds.includes(profileId)
      return {
        ...prev,
        profileIds: has
          ? prev.profileIds.filter((id) => id !== profileId)
          : [...prev.profileIds, profileId],
      }
    })
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Action bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/40">
        <span className="text-xs font-medium text-foreground truncate">
          {draft.name || t('teams.untitled')}
        </span>
        <div className="flex items-center gap-2">
          <button
            onClick={handleSave}
            disabled={!isDirty || saving}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors',
              isDirty
                ? 'bg-brand/15 text-brand hover:bg-brand/25'
                : 'bg-background-tertiary text-foreground-muted/40 cursor-default',
              saving && 'opacity-60',
            )}
          >
            {saving
              ? <Loader2 className="w-3 h-3 animate-spin" />
              : <Save className="w-3 h-3" />}
            {saving ? t('teams.saving') : t('teams.save')}
          </button>
          <button
            onClick={handleDeleteClick}
            disabled={team.isTemplate}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-colors disabled:opacity-30 disabled:cursor-not-allowed',
              confirmDelete
                ? 'bg-red-500/15 text-red-400 hover:bg-red-500/25'
                : 'text-foreground-muted/60 hover:text-foreground hover:bg-background-tertiary',
            )}
            title={team.isTemplate ? t('teams.cannotDeleteTemplate') : confirmDelete ? t('teams.clickToConfirm') : t('teams.deleteTeamTitle')}
          >
            <Trash2 className="w-3 h-3" />
            {confirmDelete ? t('teams.confirm') : t('teams.deleteBtn')}
          </button>
        </div>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Name */}
        <div className="space-y-1.5">
          <FieldLabel>{t('teams.name')}</FieldLabel>
          <Input
            value={draft.name}
            onChange={(e) => patch('name', e.target.value)}
            className="text-xs"
          />
        </div>

        {/* Description */}
        <div className="space-y-1.5">
          <FieldLabel>{t('teams.description')}</FieldLabel>
          <Textarea
            value={draft.description}
            onChange={(e) => patch('description', e.target.value)}
            className="min-h-[80px] text-xs resize-none"
          />
        </div>

        {/* Members */}
        {allProfiles.length > 0 && (
          <div className="space-y-1.5">
            <FieldLabel>{t('teams.members', { count: draft.profileIds.length })}</FieldLabel>
            <div className="border border-border/40 rounded-lg p-2 space-y-0.5 max-h-[300px] overflow-y-auto">
              {allProfiles.map((profile) => {
                const checked = draft.profileIds.includes(profile.id)
                const colors = STAGE_COLORS[profile.stage]
                return (
                  <div
                    key={profile.id}
                    className="flex items-center gap-2.5 px-1.5 py-1.5 rounded hover:bg-background-tertiary/50 cursor-pointer"
                    onClick={() => toggleMember(profile.id)}
                  >
                    <Checkbox
                      checked={checked}
                      onCheckedChange={() => toggleMember(profile.id)}
                      className="shrink-0"
                    />
                    <span className="text-xs text-foreground truncate flex-1">{profile.name}</span>
                    <span className={cn('text-[9px] px-1.5 py-0.5 rounded-full font-medium shrink-0', colors.bg, colors.text)}>
                      {profile.stage}
                    </span>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function mapDtoToTeam(d: Awaited<ReturnType<typeof tauriApi.listAgentTeams>>[number]): AgentTeam {
  return {
    id: d.id,
    name: d.name,
    description: d.description,
    profileIds: d.profileIds,
    isTemplate: d.isTemplate,
  }
}

function mapDtoToProfile(d: Awaited<ReturnType<typeof tauriApi.listAgentProfiles>>[number]): AgentProfile {
  let ruleIds: string[] = []
  let toolIds: string[] = []
  try { ruleIds = JSON.parse(d.rulesJson || '[]') } catch { /* keep empty */ }
  try { toolIds = JSON.parse(d.toolsJson || '[]') } catch { /* keep empty */ }

  return {
    id: d.id,
    name: d.name,
    description: d.description,
    stage: d.stage as AgentStage,
    provider: d.provider,
    model: d.model,
    temperature: d.temperature,
    systemPrompt: d.systemPrompt,
    maxTokensPerRun: d.maxTokensPerRun,
    isTemplate: d.isTemplate,
    isEnabled: d.isEnabled,
    ruleIds,
    toolIds,
  }
}

// -----------------------------------------------------------------------------
// TeamsTab
// -----------------------------------------------------------------------------

export function TeamsTab() {
  const { t } = useTranslation('agents')
  const [teams, setTeams] = useState<AgentTeam[]>([])
  const [allProfiles, setAllProfiles] = useState<AgentProfile[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draftIds, setDraftIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    Promise.all([
      tauriApi.listAgentTeams(),
      tauriApi.listAgentProfiles(),
    ])
      .then(([teamsData, profilesData]) => {
        const mappedTeams = teamsData.map(mapDtoToTeam)
        const mappedProfiles = profilesData.map(mapDtoToProfile)
        setTeams(mappedTeams)
        setAllProfiles(mappedProfiles)
        if (mappedTeams.length > 0) setSelectedId(mappedTeams[0].id)
      })
      .catch((err) => setError(err.message ?? 'Failed to load teams'))
      .finally(() => setLoading(false))
  }, [])

  const handleCreate = () => {
    // Don't allow creating another draft while one exists unsaved
    if (draftIds.size > 0) {
      const existingDraft = [...draftIds][0]
      setSelectedId(existingDraft)
      return
    }

    const tempId = `draft-${crypto.randomUUID()}`
    const draft: AgentTeam = {
      id: tempId,
      name: '',
      description: '',
      profileIds: [],
      isTemplate: false,
    }
    setTeams((prev) => [...prev, draft])
    setDraftIds((prev) => new Set(prev).add(tempId))
    setSelectedId(tempId)
  }

  const handleUpdate = useCallback(async (draft: AgentTeam) => {
    const isDraft = draft.id.startsWith('draft-')
    try {
      if (isDraft) {
        // First save — create in backend
        const dto = await tauriApi.createAgentTeam({
          name: draft.name,
          description: draft.description,
          profileIds: draft.profileIds,
        })
        const created = mapDtoToTeam(dto)
        setTeams((prev) => prev.map((t) => t.id === draft.id ? created : t))
        setDraftIds((prev) => {
          const next = new Set(prev)
          next.delete(draft.id)
          return next
        })
        setSelectedId(created.id)
      } else {
        // Normal update
        const dto = await tauriApi.updateAgentTeam({
          id: draft.id,
          name: draft.name,
          description: draft.description,
          profileIds: draft.profileIds,
        })
        const updated = mapDtoToTeam(dto)
        setTeams((prev) => prev.map((t) => t.id === updated.id ? updated : t))
      }
    } catch {
      // Silently fail
    }
  }, [])

  const handleDelete = useCallback(async (id: string) => {
    try {
      // Only call backend if it's a persisted team
      if (!id.startsWith('draft-')) {
        await tauriApi.deleteAgentTeam(id)
      }
      setTeams((prev) => {
        const next = prev.filter((t) => t.id !== id)
        if (selectedId === id) {
          const idx = prev.findIndex((t) => t.id === id)
          const nextTeam = next[Math.min(idx, next.length - 1)]
          setSelectedId(nextTeam?.id ?? null)
        }
        return next
      })
      setDraftIds((prev) => {
        const next = new Set(prev)
        next.delete(id)
        return next
      })
    } catch {
      // Silently fail
    }
  }, [selectedId])

  const selectedTeam = teams.find((t) => t.id === selectedId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">{t('teams.loading')}</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center text-red-400/80">
        <AlertCircle className="w-5 h-5 mr-2" />
        <span className="text-xs">{error}</span>
      </div>
    )
  }

  return (
    <div className="flex-1 flex min-w-0 overflow-hidden">
      {/* Left — Team list */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40 flex items-center justify-between">
          <div>
            <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
              {t('innerTabs.teams')}
            </span>
            <span className="text-[10px] text-foreground-muted/50 ml-1.5">
              ({teams.length})
            </span>
          </div>
          <button
            onClick={handleCreate}
            className="p-1 rounded hover:bg-background-tertiary text-foreground-muted/60 hover:text-foreground transition-colors"
            title={t('teams.createTeamTitle')}
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          {teams.map((team) => (
            <TeamListItem
              key={team.id}
              team={team}
              isSelected={team.id === selectedId}
              onSelect={() => setSelectedId(team.id)}
            />
          ))}
        </div>
      </div>

      {/* Right — Team detail or empty state */}
      <div className="flex-1 min-w-0 flex flex-col">
        {selectedTeam ? (
          <TeamDetail
            team={selectedTeam}
            allProfiles={allProfiles}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted/40">
            <Users className="w-10 h-10 mb-3 opacity-20" />
            <span className="text-xs mb-3">{t('teams.noSelected')}</span>
            <button
              onClick={handleCreate}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              {t('teams.create')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
