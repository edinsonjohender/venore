// =============================================================================
// PipelineTab — Pipeline run history + flow visualization
// =============================================================================

import { useState, useRef, useLayoutEffect, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Play, Workflow, Shield, Zap, Search, FileText, Bug, Brain, Bot, Loader2, AlertCircle, X, ChevronDown, ChevronUp, Layers } from 'lucide-react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import { formatTimeAgo } from '@/lib/time'
import { tauriApi } from '@/lib/tauri'
import type {
  PipelineRunStartedPayload,
  PipelineConsolePayload,
  PipelineRunCompletedPayload,
  PipelineRunFailedPayload,
  PipelineStepDto,
  RunAnalysisContextDto,
} from '@/lib/tauri'
import { BottomPanel } from './BottomPanel'
import {
  STAGE_COLORS, RUN_STATUS_COLORS,
} from './types'
import type { AgentProfile, AgentTeam, PipelineRun, PipelineRunStatus, AgentStage, ConsoleEntry } from './types'

// Map agent IDs to icons for visual variety
const AGENT_ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  'triager-general': Search,
  'triager-priority': Zap,
  'spec-architecture': Brain,
  'spec-security': Shield,
  'spec-performance': Zap,
  'spec-testing': Bug,
  'spec-documentation': FileText,
  'spec-patterns': Search,
  'reporter-summary': FileText,
  'reporter-context': FileText,
}

// Stage line colors (raw hex for SVG stroke)
const STAGE_LINE_COLORS: Record<AgentStage, string> = {
  triager: '#60a5fa',    // blue-400
  specialist: '#fbbf24', // amber-400
  reporter: '#4ade80',   // green-400
  subagent: '#c084fc',   // purple-400
}

// -----------------------------------------------------------------------------
// RunHistoryItem
// -----------------------------------------------------------------------------

function RunHistoryItem({
  run, isSelected, onSelect,
}: {
  run: PipelineRun
  isSelected: boolean
  onSelect: () => void
}) {
  const { t } = useTranslation('agents')
  const statusColors = RUN_STATUS_COLORS[run.status]
  const timeAgo = formatTimeAgo(run.startedAt)
  const duration = run.durationMs > 0 ? formatDuration(run.durationMs) : t('pipeline.inProgress')

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
        <span className="text-xs font-medium text-foreground truncate flex-1">
          {run.title}
        </span>
        <span className={cn('text-[10px] px-1.5 py-0.5 rounded-full font-medium', statusColors.bg, statusColors.text)}>
          {t('common:statusLabels.' + run.status, { defaultValue: run.status })}
        </span>
      </div>
      <div className="flex items-center gap-2 text-[10px] text-foreground-muted/60">
        <span>{run.teamName}</span>
        <span>·</span>
        <span>{timeAgo}</span>
        <span>·</span>
        <span>{duration}</span>
        {run.depthLevel && run.depthLevel !== 'normal' && (
          <>
            <span>·</span>
            <span className="text-brand/70">{t('common:depthLevels.' + run.depthLevel, { defaultValue: run.depthLevel })}</span>
          </>
        )}
      </div>
    </button>
  )
}

// -----------------------------------------------------------------------------
// AgentCard — Collectible-style card with data-agent-id for SVG connections
// -----------------------------------------------------------------------------

function AgentCard({
  agent, isSelected, onSelect,
}: {
  agent: AgentProfile
  isSelected: boolean
  onSelect: () => void
}) {
  const { t } = useTranslation('agents')
  const Icon = AGENT_ICONS[agent.id] ?? Bot
  const colors = STAGE_COLORS[agent.stage]

  return (
    <button
      data-agent-id={agent.id}
      onClick={onSelect}
      className={cn(
        'flex items-center gap-2.5 pl-1.5 pr-4 py-1.5 rounded-xl',
        'bg-white/[0.06] border border-white/[0.08]',
        'hover:bg-white/[0.09] transition-colors cursor-pointer text-left',
        isSelected && 'ring-1 ring-brand/50 bg-white/[0.09]',
      )}
    >
      {/* Icon */}
      <div className={cn(
        'w-8 h-8 rounded-lg flex items-center justify-center shrink-0',
        colors.bg,
      )}>
        <Icon className={cn('w-4 h-4', colors.text)} />
      </div>

      {/* Text */}
      <div className="min-w-0">
        <div className="text-[11px] font-medium text-foreground truncate leading-tight">
          {agent.name}
        </div>
        <div className="text-[9px] text-foreground-muted/50 leading-tight mt-0.5">
          {t('pipeline.' + agent.stage)}
        </div>
      </div>
    </button>
  )
}

// -----------------------------------------------------------------------------
// SVG connection helpers
// -----------------------------------------------------------------------------

interface CardRect {
  id: string
  stage: AgentStage
  cx: number  // center x relative to container
  cy: number  // center y relative to container
  left: number
  right: number
  top: number
  bottom: number
}

/** Measure all agent card positions relative to the container */
function measureCards(container: HTMLElement, profiles: AgentProfile[]): CardRect[] {
  const containerRect = container.getBoundingClientRect()
  const cards = container.querySelectorAll<HTMLElement>('[data-agent-id]')
  const result: CardRect[] = []

  cards.forEach((card) => {
    const r = card.getBoundingClientRect()
    const agentId = card.getAttribute('data-agent-id') ?? ''
    const profile = profiles.find((p) => p.id === agentId)
    if (!profile) return

    result.push({
      id: agentId,
      stage: profile.stage,
      cx: r.left + r.width / 2 - containerRect.left,
      cy: r.top + r.height / 2 - containerRect.top,
      left: r.left - containerRect.left,
      right: r.right - containerRect.left,
      top: r.top - containerRect.top,
      bottom: r.bottom - containerRect.top,
    })
  })

  return result
}

/** Build cubic bezier path: right-center of source → left-center of target */
function bezierPath(src: CardRect, dst: CardRect): string {
  const sx = src.right
  const sy = src.cy
  const ex = dst.left
  const ey = dst.cy
  const cpLen = Math.abs(ex - sx) * 0.5
  return `M ${sx} ${sy} C ${sx + cpLen} ${sy}, ${ex - cpLen} ${ey}, ${ex} ${ey}`
}

// -----------------------------------------------------------------------------
// PipelineFlow — Cards in 3 columns with SVG bezier connections
// -----------------------------------------------------------------------------

function PipelineFlow({
  team, profiles, selectedAgentId, onSelectAgent,
}: {
  team: AgentTeam
  profiles: AgentProfile[]
  selectedAgentId: string | null
  onSelectAgent: (id: string) => void
}) {
  const { t } = useTranslation('agents')
  const containerRef = useRef<HTMLDivElement>(null)
  const [paths, setPaths] = useState<{ d: string; color: string }[]>([])

  const stages: AgentStage[] = ['triager', 'specialist', 'reporter']

  const teamProfiles = team.profileIds
    .map((id) => profiles.find((p) => p.id === id))
    .filter(Boolean) as AgentProfile[]

  const grouped = stages.map((stage) => ({
    stage,
    agents: teamProfiles.filter((p) => p.stage === stage),
  }))

  const computePaths = useCallback(() => {
    const el = containerRef.current
    if (!el) return

    const cards = measureCards(el, profiles)
    const newPaths: { d: string; color: string }[] = []

    // Connect triagers → specialists
    const triagers = cards.filter((c) => c.stage === 'triager')
    const specialists = cards.filter((c) => c.stage === 'specialist')
    const reporters = cards.filter((c) => c.stage === 'reporter')

    for (const src of triagers) {
      for (const dst of specialists) {
        newPaths.push({ d: bezierPath(src, dst), color: STAGE_LINE_COLORS.triager })
      }
    }

    // Connect specialists → reporters
    for (const src of specialists) {
      for (const dst of reporters) {
        newPaths.push({ d: bezierPath(src, dst), color: STAGE_LINE_COLORS.specialist })
      }
    }

    setPaths(newPaths)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [team.id])

  useLayoutEffect(() => {
    // Defer measurement to next frame so cards are painted
    const raf = requestAnimationFrame(computePaths)

    // Recompute on resize
    const observer = new ResizeObserver(computePaths)
    if (containerRef.current) observer.observe(containerRef.current)
    return () => {
      cancelAnimationFrame(raf)
      observer.disconnect()
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [team.id])

  return (
    <div ref={containerRef} className="flex-1 overflow-auto relative flex items-center justify-evenly p-8">
        {/* SVG connection layer */}
        <svg className="absolute inset-0 w-full h-full pointer-events-none">
          {paths.map((p, i) => (
            <path
              key={i}
              d={p.d}
              fill="none"
              stroke={p.color}
              strokeWidth="1.5"
              strokeOpacity="0.25"
              strokeLinecap="round"
              strokeDasharray="6 4"
            />
          ))}
        </svg>

        {/* Stage columns */}
        {grouped.map(({ stage, agents }) => {
          const colors = STAGE_COLORS[stage]
          return (
            <div key={stage} className="relative flex flex-col items-center gap-4">
              {/* Stage header */}
              <div className={cn('text-[10px] font-medium uppercase tracking-wider px-2.5 py-1 rounded', colors.bg, colors.text)}>
                {t('pipeline.' + stage)}
              </div>

              {/* Agent cards */}
              {agents.length === 0 ? (
                <div className="text-[10px] text-foreground-muted/40 italic">
                  {t('pipeline.noAgents')}
                </div>
              ) : (
                <div className="flex flex-col gap-2.5">
                  {agents.map((agent) => (
                    <AgentCard
                      key={agent.id}
                      agent={agent}
                      isSelected={agent.id === selectedAgentId}
                      onSelect={() => onSelectAgent(agent.id)}
                    />
                  ))}
                </div>
              )}
            </div>
          )
        })}
    </div>
  )
}

// -----------------------------------------------------------------------------
// AgentDetailPanel — Shows details of the selected agent
// -----------------------------------------------------------------------------

function AgentDetailPanel({
  agent, onClose,
}: {
  agent: AgentProfile
  onClose: () => void
}) {
  const { t } = useTranslation('agents')
  const colors = STAGE_COLORS[agent.stage]
  const Icon = AGENT_ICONS[agent.id] ?? Bot
  const [promptExpanded, setPromptExpanded] = useState(false)

  return (
    <div className="w-[280px] shrink-0 border-l border-border bg-background-secondary/30 overflow-y-auto">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border/40 flex items-center gap-3">
        <div className={cn('w-8 h-8 rounded-lg flex items-center justify-center shrink-0', colors.bg)}>
          <Icon className={cn('w-4 h-4', colors.text)} />
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-xs font-medium text-foreground truncate">{agent.name}</div>
          <div className={cn('text-[10px]', colors.text)}>{t('pipeline.' + agent.stage)}</div>
        </div>
        <button
          onClick={onClose}
          className="w-6 h-6 rounded flex items-center justify-center text-foreground-muted hover:text-foreground hover:bg-white/[0.06] transition-colors"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="p-4 space-y-4">
        {/* Description */}
        {agent.description && (
          <div>
            <div className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted mb-1">{t('pipeline.description')}</div>
            <div className="text-xs text-foreground-muted/80 leading-relaxed">{agent.description}</div>
          </div>
        )}

        {/* Model info */}
        <div>
          <div className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted mb-1.5">{t('pipeline.model')}</div>
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-foreground-muted/60">{t('pipeline.provider')}</span>
              <span className="text-[11px] text-foreground">{agent.provider || '—'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-foreground-muted/60">{t('pipeline.model')}</span>
              <span className="text-[11px] text-foreground">{agent.model || '—'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-foreground-muted/60">{t('pipeline.temperature')}</span>
              <span className="text-[11px] text-foreground">{agent.temperature}</span>
            </div>
          </div>
        </div>

        {/* Status */}
        <div>
          <div className="text-[10px] font-medium uppercase tracking-wider text-foreground-muted mb-1.5">{t('pipeline.status')}</div>
          <div className="flex items-center gap-2">
            <div className={cn('w-1.5 h-1.5 rounded-full', agent.isEnabled ? 'bg-green-400' : 'bg-foreground-muted/30')} />
            <span className="text-[11px] text-foreground">{agent.isEnabled ? t('pipeline.enabled') : t('pipeline.disabled')}</span>
            {agent.isTemplate && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-foreground-muted/10 text-foreground-muted/60 ml-auto">
                {t('pipeline.template')}
              </span>
            )}
          </div>
        </div>

        {/* System prompt */}
        {agent.systemPrompt && (
          <div>
            <button
              onClick={() => setPromptExpanded(!promptExpanded)}
              className="flex items-center gap-1 text-[10px] font-medium uppercase tracking-wider text-foreground-muted mb-1.5 hover:text-foreground-muted/80 transition-colors"
            >
              {t('pipeline.systemPrompt')}
              {promptExpanded
                ? <ChevronUp className="w-3 h-3" />
                : <ChevronDown className="w-3 h-3" />
              }
            </button>
            <div className={cn(
              'text-[10px] text-foreground-muted/70 font-mono bg-black/20 rounded p-2 leading-relaxed whitespace-pre-wrap',
              !promptExpanded && 'max-h-[80px] overflow-hidden',
            )}>
              {agent.systemPrompt}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// StepDetailPanel — Shows output of pipeline steps for selected run
// -----------------------------------------------------------------------------

function StepDetailPanel({
  steps, onClose,
}: {
  steps: PipelineStepDto[]
  onClose: () => void
}) {
  const { t } = useTranslation('agents')
  const [expandedStep, setExpandedStep] = useState<string | null>(
    steps.length > 0 ? steps[0].id : null,
  )

  return (
    <div className="w-[340px] shrink-0 border-l border-border bg-background-secondary/30 overflow-y-auto">
      <div className="px-4 py-3 border-b border-border/40 flex items-center justify-between">
        <span className="text-xs font-medium text-foreground">{t('pipeline.stepResults')}</span>
        <button
          onClick={onClose}
          className="w-6 h-6 rounded flex items-center justify-center text-foreground-muted hover:text-foreground hover:bg-white/[0.06] transition-colors"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="divide-y divide-border/30">
        {steps.map((step) => {
          const isExpanded = step.id === expandedStep
          const isFailed = step.status === 'failed'
          const stageColors = STAGE_COLORS[step.stage as AgentStage] ?? STAGE_COLORS.specialist

          return (
            <div key={step.id}>
              <button
                onClick={() => setExpandedStep(isExpanded ? null : step.id)}
                className="w-full text-left px-4 py-2.5 hover:bg-white/[0.03] transition-colors"
              >
                <div className="flex items-center gap-2">
                  <div className={cn('w-1.5 h-1.5 rounded-full', isFailed ? 'bg-red-400' : step.status === 'completed' ? 'bg-green-400' : 'bg-amber-400')} />
                  <span className="text-xs font-medium text-foreground flex-1 truncate">{step.profileName}</span>
                  <span className={cn('text-[10px] px-1.5 py-0.5 rounded', stageColors.bg, stageColors.text)}>
                    {t('pipeline.' + step.stage, { defaultValue: step.stage })}
                  </span>
                  {isExpanded ? <ChevronUp className="w-3 h-3 text-foreground-muted" /> : <ChevronDown className="w-3 h-3 text-foreground-muted" />}
                </div>
                <div className="flex items-center gap-2 mt-1 text-[10px] text-foreground-muted/60">
                  <span>{step.provider}/{step.model}</span>
                  <span>·</span>
                  <span>{t('pipeline.tokens', { count: step.totalTokens })}</span>
                  <span>·</span>
                  <span>{formatDuration(step.durationMs)}</span>
                </div>
              </button>

              {isExpanded && (
                <div className="px-4 pb-3">
                  {isFailed && step.error && (
                    <div className="text-[11px] text-red-400 bg-red-500/10 rounded p-2 mb-2">
                      {step.error}
                    </div>
                  )}
                  {step.output ? (
                    <div className="text-[11px] text-foreground-muted/80 font-mono bg-black/20 rounded p-2 leading-relaxed whitespace-pre-wrap max-h-[400px] overflow-y-auto">
                      {step.output}
                    </div>
                  ) : (
                    <div className="text-[10px] text-foreground-muted/40 italic">{t('pipeline.noOutput')}</div>
                  )}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// PipelineTab
// -----------------------------------------------------------------------------

export function PipelineTab() {
  const { t } = useTranslation('agents')
  const [profiles, setProfiles] = useState<AgentProfile[]>([])
  const [teams, setTeams] = useState<AgentTeam[]>([])
  const [runs, setRuns] = useState<PipelineRun[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null)
  const [consoleEntries, setConsoleEntries] = useState<ConsoleEntry[]>([])
  const [selectedSteps, setSelectedSteps] = useState<PipelineStepDto[] | null>(null)
  const [analysisContext, setAnalysisContext] = useState<RunAnalysisContextDto | null>(null)
  const [bottomTab, setBottomTab] = useState<'console' | 'report'>('console')

  // Load initial data
  useEffect(() => {
    Promise.all([
      tauriApi.listAgentProfiles(),
      tauriApi.listAgentTeams(),
      tauriApi.listPipelineRuns(),
    ])
      .then(([profilesData, teamsData, runsData]) => {
        const mappedProfiles: AgentProfile[] = profilesData.map((d) => {
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
        })
        const mappedTeams: AgentTeam[] = teamsData.map((t) => ({
          id: t.id,
          name: t.name,
          description: t.description,
          profileIds: t.profileIds,
          isTemplate: t.isTemplate,
        }))
        const mappedRuns: PipelineRun[] = runsData.map((r) => ({
          id: r.id,
          teamId: r.teamId,
          teamName: r.teamName,
          taskType: r.taskType,
          title: r.title,
          status: r.status as PipelineRunStatus,
          prNumber: r.prNumber,
          projectPath: r.projectPath,
          startedAt: r.startedAt,
          finishedAt: r.finishedAt,
          durationMs: r.durationMs,
          totalTokens: r.totalTokens,
          createdAt: r.createdAt,
          prAuthor: r.prAuthor,
          prAuthorAvatar: r.prAuthorAvatar,
          prAdditions: r.prAdditions,
          prDeletions: r.prDeletions,
          prChangedFiles: r.prChangedFiles,
          depthLevel: r.depthLevel,
        }))

        setProfiles(mappedProfiles)
        setTeams(mappedTeams)
        setRuns(mappedRuns)

        if (mappedRuns.length > 0) {
          setSelectedRunId(mappedRuns[0].id)
        }
      })
      .catch((err) => setError(err.message ?? 'Failed to load pipeline data'))
      .finally(() => setLoading(false))
  }, [])

  // Listen to pipeline events
  useEffect(() => {
    let cancelled = false
    const unlisteners: UnlistenFn[] = []

    const setup = async () => {
      const listeners = await Promise.all([
        listen<PipelineRunStartedPayload>('pipeline:run-started', (event) => {
          const p = event.payload
          const newRun: PipelineRun = {
            id: p.runId,
            teamId: '',
            teamName: p.teamName,
            taskType: 'pr-analysis',
            title: p.title,
            status: 'running',
            prNumber: null,
            projectPath: '',
            startedAt: new Date().toISOString(),
            finishedAt: null,
            durationMs: 0,
            totalTokens: 0,
            createdAt: new Date().toISOString(),
            prAuthor: null,
            prAuthorAvatar: null,
            prAdditions: null,
            prDeletions: null,
            prChangedFiles: null,
            depthLevel: null,
          }
          setRuns((prev) => [newRun, ...prev])
          setSelectedRunId(p.runId)
          setConsoleEntries([])
          setSelectedSteps(null)
          setAnalysisContext(null)
          setBottomTab('console')
        }),

        listen<PipelineConsolePayload>('pipeline:console', (event) => {
          const p = event.payload
          setConsoleEntries((prev) => [...prev, {
            timestamp: new Date().toISOString(),
            agentName: p.agentName,
            stage: p.stage,
            message: p.message,
          }])
        }),

        listen<PipelineRunCompletedPayload>('pipeline:run-completed', async (event) => {
          const p = event.payload
          // Load steps + analysis context (ctx.run has full PR metadata)
          try {
            const [steps, ctx] = await Promise.all([
              tauriApi.getPipelineSteps(p.runId),
              tauriApi.getRunAnalysisContext(p.runId),
            ])
            // Rehydrate run with full data from backend (prAuthor, additions, etc.)
            const fullRun = ctx.run
            setRuns((prev) =>
              prev.map((r) =>
                r.id === p.runId
                  ? {
                      ...r,
                      status: 'completed' as PipelineRunStatus,
                      durationMs: p.durationMs,
                      totalTokens: p.totalTokens,
                      finishedAt: new Date().toISOString(),
                      prAuthor: fullRun.prAuthor ?? r.prAuthor,
                      prAuthorAvatar: fullRun.prAuthorAvatar ?? r.prAuthorAvatar,
                      prAdditions: fullRun.prAdditions ?? r.prAdditions,
                      prDeletions: fullRun.prDeletions ?? r.prDeletions,
                      prChangedFiles: fullRun.prChangedFiles ?? r.prChangedFiles,
                      depthLevel: fullRun.depthLevel ?? r.depthLevel,
                    }
                  : r
              )
            )
            setSelectedSteps(steps)
            setAnalysisContext(ctx)
          } catch {
            // Fallback: at least update status
            setRuns((prev) =>
              prev.map((r) =>
                r.id === p.runId
                  ? { ...r, status: 'completed' as PipelineRunStatus, durationMs: p.durationMs, totalTokens: p.totalTokens, finishedAt: new Date().toISOString() }
                  : r
              )
            )
          }
          setBottomTab('report')
        }),

        listen<PipelineRunFailedPayload>('pipeline:run-failed', (event) => {
          const p = event.payload
          setRuns((prev) =>
            prev.map((r) =>
              r.id === p.runId
                ? { ...r, status: 'failed' as PipelineRunStatus, finishedAt: new Date().toISOString() }
                : r
            )
          )
          setConsoleEntries((prev) => [...prev, {
            timestamp: new Date().toISOString(),
            agentName: 'Pipeline',
            stage: 'reporter',
            message: `Pipeline failed: ${p.error}`,
          }])
        }),
      ])

      // If effect was cleaned up while awaiting, immediately unlisten
      if (cancelled) {
        listeners.forEach((fn) => fn())
      } else {
        unlisteners.push(...listeners)
      }
    }

    setup()

    return () => {
      cancelled = true
      unlisteners.forEach((fn) => fn())
    }
  }, [])

  // Load steps when clicking a completed/failed run
  const handleSelectRun = useCallback(async (runId: string) => {
    setSelectedRunId(runId)
    setSelectedAgentId(null)

    const run = runs.find((r) => r.id === runId)
    if (run && run.status !== 'running') {
      try {
        const [steps, ctx] = await Promise.all([
          tauriApi.getPipelineSteps(runId),
          tauriApi.getRunAnalysisContext(runId),
        ])
        setSelectedSteps(steps)
        setAnalysisContext(ctx)
        setBottomTab('report')
      } catch {
        setSelectedSteps(null)
        setAnalysisContext(null)
      }
    } else {
      setSelectedSteps(null)
      setAnalysisContext(null)
      setBottomTab('console')
    }
  }, [runs])

  const selectedRun = runs.find((r) => r.id === selectedRunId)
  const defaultTeam = teams.length > 0 ? teams[0] : null
  const selectedAgent = profiles.find((p) => p.id === selectedAgentId) ?? null

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-foreground-muted/50">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        <span className="text-xs">{t('pipeline.loading')}</span>
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
    <div className="flex-1 flex">
      {/* Left — Run history list */}
      <div className="w-[250px] shrink-0 border-r border-border overflow-hidden flex flex-col">
        <div className="px-3 py-2 border-b border-border/40">
          <span className="text-[11px] font-medium uppercase tracking-wider text-foreground-muted">
            {t('pipeline.runHistory')}
          </span>
        </div>
        <div className="flex-1 overflow-y-auto">
          {runs.length === 0 ? (
            <div className="flex-1 flex flex-col items-center justify-center py-8 text-foreground-muted/40">
              <Workflow className="w-8 h-8 mb-2 opacity-30" />
              <span className="text-xs">{t('pipeline.noRuns')}</span>
              <span className="text-[10px] mt-1">{t('pipeline.noRunsHint')}</span>
            </div>
          ) : (
            runs.map((run) => (
              <RunHistoryItem
                key={run.id}
                run={run}
                isSelected={run.id === selectedRunId}
                onSelect={() => handleSelectRun(run.id)}
              />
            ))
          )}
        </div>
      </div>

      {/* Middle — Flow + console */}
      <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
        <div className="flex-1 relative min-h-0">
          {selectedRun && defaultTeam ? (
            <PipelineFlow
              team={defaultTeam}
              profiles={profiles}
              selectedAgentId={selectedAgentId}
              onSelectAgent={setSelectedAgentId}
            />
          ) : (
            <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
              {t('pipeline.selectRun')}
            </div>
          )}
        </div>

        <BottomPanel
          entries={consoleEntries}
          steps={selectedSteps}
          run={selectedRun ?? null}
          analysisContext={analysisContext}
          activeTab={bottomTab}
          onTabChange={setBottomTab}
        />
      </div>

      {/* Right — Step detail panel (when viewing completed run) */}
      {selectedSteps && selectedSteps.length > 0 && !selectedAgent && (
        <StepDetailPanel
          steps={selectedSteps}
          onClose={() => setSelectedSteps(null)}
        />
      )}

      {/* Right — Agent detail panel */}
      {selectedAgent && (
        <AgentDetailPanel
          agent={selectedAgent}
          onClose={() => setSelectedAgentId(null)}
        />
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function formatDuration(ms: number): string {
  const secs = Math.floor(ms / 1000)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  const remSecs = secs % 60
  return `${mins}m ${remSecs}s`
}
