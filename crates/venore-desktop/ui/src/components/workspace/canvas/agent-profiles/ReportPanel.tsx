// =============================================================================
// ReportPanel — Visual dashboard for pipeline analysis results
// =============================================================================

import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { AlertTriangle, AlertCircle, Info, CheckCircle2, BarChart3, Layers, Search, GitPullRequest, Plus, Minus, FileCode } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PipelineStepDto, RunAnalysisContextDto } from '@/lib/tauri'
import type { PipelineRun, PipelineReport, ReportCategory, ReportFinding, FindingSeverity } from './types'
import { FINDING_SEVERITY_COLORS } from './types'
import { findReporterStep, parseReportFromOutput } from './reportParser'
import { MarkdownRenderer } from '@/components/ui/markdown-renderer'

// -----------------------------------------------------------------------------
// Score colors helper
// -----------------------------------------------------------------------------

function scoreColor(score: number) {
  if (score >= 80) return { stroke: '#4ade80', text: 'text-green-400', bg: 'bg-green-500/10' }
  if (score >= 50) return { stroke: '#fbbf24', text: 'text-amber-400', bg: 'bg-amber-500/10' }
  return { stroke: '#f87171', text: 'text-red-400', bg: 'bg-red-500/10' }
}

// -----------------------------------------------------------------------------
// ScoreRing — Large circular score with SVG arc progress
// -----------------------------------------------------------------------------

function ScoreRing({ score, size = 56 }: { score: number; size?: number }) {
  const r = (size - 6) / 2
  const circumference = 2 * Math.PI * r
  const offset = circumference * (1 - score / 100)
  const colors = scoreColor(score)

  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="rotate-[-90deg]">
        {/* Background ring */}
        <circle
          cx={size / 2} cy={size / 2} r={r}
          fill="none" stroke="currentColor" strokeWidth="3"
          className="text-white/[0.06]"
        />
        {/* Progress arc */}
        <circle
          cx={size / 2} cy={size / 2} r={r}
          fill="none" stroke={colors.stroke} strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          className="transition-all duration-700"
        />
      </svg>
      <div className="absolute inset-0 flex items-center justify-center">
        <span className={cn('text-base font-bold', colors.text)}>{score}</span>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// RadarChart — Custom SVG radar/spider chart
// -----------------------------------------------------------------------------

function RadarChart({ categories }: { categories: ReportCategory[] }) {
  if (categories.length < 3) return null

  const size = 220
  const cx = size / 2
  const cy = size / 2
  const radius = 80
  const n = categories.length
  const levels = [0.25, 0.5, 0.75, 1.0]

  const angle = (i: number) => (Math.PI * 2 * i) / n - Math.PI / 2
  const point = (i: number, ratio: number) => ({
    x: cx + Math.cos(angle(i)) * radius * ratio,
    y: cy + Math.sin(angle(i)) * radius * ratio,
  })

  const guidePaths = levels.map((level) => {
    const pts = Array.from({ length: n }, (_, i) => point(i, level))
    return pts.map((p) => `${p.x},${p.y}`).join(' ')
  })

  const dataPts = categories.map((cat, i) => point(i, cat.score / 100))
  const dataPath = dataPts.map((p) => `${p.x},${p.y}`).join(' ')

  const axes = Array.from({ length: n }, (_, i) => ({
    x2: point(i, 1).x,
    y2: point(i, 1).y,
  }))

  const labelOffset = 18
  const labels = categories.map((cat, i) => {
    const p = point(i, 1)
    const dx = p.x - cx
    const dy = p.y - cy
    const len = Math.sqrt(dx * dx + dy * dy)
    return {
      name: cat.name,
      x: p.x + (dx / len) * labelOffset,
      y: p.y + (dy / len) * labelOffset,
      anchor: (Math.abs(dx) < 1 ? 'middle' : dx > 0 ? 'start' : 'end') as 'start' | 'middle' | 'end',
    }
  })

  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="shrink-0">
      {guidePaths.map((pts, i) => (
        <polygon key={i} points={pts} fill="none" stroke="currentColor" strokeWidth="0.5" className="text-white/[0.08]" />
      ))}
      {axes.map((a, i) => (
        <line key={i} x1={cx} y1={cy} x2={a.x2} y2={a.y2} stroke="currentColor" strokeWidth="0.5" className="text-white/[0.08]" />
      ))}
      <polygon
        points={dataPath}
        fill="rgba(99, 102, 241, 0.12)"
        stroke="rgba(129, 140, 248, 0.6)"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      {dataPts.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r="2.5" fill="rgba(129, 140, 248, 0.85)" />
      ))}
      {labels.map((l, i) => (
        <text key={i} x={l.x} y={l.y} textAnchor={l.anchor} dominantBaseline="middle" className="fill-foreground-muted/70 text-[9px]">
          {l.name}
        </text>
      ))}
    </svg>
  )
}

// -----------------------------------------------------------------------------
// CategoryCard — Card with score + progress bar
// -----------------------------------------------------------------------------

function CategoryCard({ category, authorAvg, projectAvg }: {
  category: ReportCategory
  authorAvg?: number
  projectAvg?: number
}) {
  const { t } = useTranslation('agents')
  const scoreColors = scoreColor(category.score)
  const barColor = category.score >= 80 ? 'bg-green-500' : category.score >= 50 ? 'bg-amber-500' : 'bg-red-500'

  return (
    <div className="rounded-lg border border-white/[0.06] bg-white/[0.03] p-3 space-y-2.5">
      {/* Name + Score */}
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-semibold text-foreground truncate">{category.name}</span>
        <span className={cn('text-base font-bold tabular-nums', scoreColors.text)}>{category.score}</span>
      </div>

      {/* Progress bar */}
      <div className="h-1 rounded-full bg-white/[0.06] overflow-hidden">
        <div
          className={cn('h-full rounded-full transition-all duration-500', barColor)}
          style={{ width: `${category.score}%` }}
        />
      </div>

      {/* Status + findings */}
      <div className="flex items-center justify-between">
        <span className={cn('text-[10px] capitalize font-medium', scoreColors.text)}>{category.status}</span>
        <span className="text-[10px] text-foreground-muted/50">
          {t('report.findingsCount', { count: category.findings_count })}
        </span>
      </div>

      {/* Averages comparison */}
      {(authorAvg !== undefined || projectAvg !== undefined) && (
        <div className="flex items-center gap-3 text-[9px] text-foreground-muted/40">
          {authorAvg !== undefined && <span>{t('report.authorAvg', { score: Math.round(authorAvg) })}</span>}
          {projectAvg !== undefined && <span>{t('report.projectAvg', { score: Math.round(projectAvg) })}</span>}
        </div>
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// FindingRow
// -----------------------------------------------------------------------------

const SEVERITY_ICONS: Record<FindingSeverity, React.ComponentType<{ className?: string }>> = {
  critical: AlertCircle,
  warning: AlertTriangle,
  info: Info,
  good: CheckCircle2,
}

function FindingRow({ finding }: { finding: ReportFinding }) {
  const colors = FINDING_SEVERITY_COLORS[finding.severity]
  const Icon = SEVERITY_ICONS[finding.severity]

  return (
    <div className="flex items-start gap-3 py-2.5 border-b border-white/[0.04] last:border-0">
      <div className={cn('w-6 h-6 rounded-md flex items-center justify-center shrink-0 mt-0.5', colors.bg)}>
        <Icon className={cn('w-3.5 h-3.5', colors.text)} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="text-xs font-semibold text-foreground">{finding.title}</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded-md bg-white/[0.06] text-foreground-muted/60 shrink-0">
            {finding.category}
          </span>
        </div>
        <p className="text-[11px] text-foreground-muted/60 leading-relaxed">{finding.description}</p>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// SectionHeader
// -----------------------------------------------------------------------------

function SectionHeader({ icon: Icon, title }: { icon: React.ComponentType<{ className?: string }>; title: string }) {
  return (
    <div className="flex items-center gap-2 mb-3">
      <Icon className="w-3.5 h-3.5 text-foreground-muted/50" />
      <span className="text-[10px] font-semibold uppercase tracking-wider text-foreground-muted/70">{title}</span>
    </div>
  )
}

// -----------------------------------------------------------------------------
// ReportDashboard — Main dashboard layout
// -----------------------------------------------------------------------------

function ReportDashboard({ report, run, analysisContext }: {
  report: PipelineReport
  run: PipelineRun | null
  analysisContext: RunAnalysisContextDto | null
}) {
  const { t } = useTranslation('agents')
  const hasRadar = report.categories.length >= 3
  const authorStats = analysisContext?.authorStats ?? null
  const authorAvgs = analysisContext?.authorCategoryAverages ?? []
  const projectAvgs = analysisContext?.projectCategoryAverages ?? []

  // Build lookup maps for category averages
  const authorAvgMap = useMemo(() => {
    const map = new Map<string, number>()
    for (const a of authorAvgs) map.set(a.categoryName, a.avgScore)
    return map
  }, [authorAvgs])

  const projectAvgMap = useMemo(() => {
    const map = new Map<string, number>()
    for (const a of projectAvgs) map.set(a.categoryName, a.avgScore)
    return map
  }, [projectAvgs])

  return (
    <div className="p-5 space-y-5 overflow-y-auto h-full">
      {/* ── Header ── */}
      <div className="flex items-center gap-4">
        <ScoreRing score={report.overall_score} />
        <div className="flex-1 min-w-0">
          {run && (
            <div className="text-sm font-semibold text-foreground truncate mb-1">{run.title}</div>
          )}
          <p className="text-[11px] text-foreground-muted/70 leading-relaxed line-clamp-2">{report.summary}</p>
        </div>
        <div className="flex items-center gap-4 shrink-0">
          <div className="text-center">
            <div className="text-sm font-bold text-foreground">{report.categories.length}</div>
            <div className="text-[9px] text-foreground-muted/50 uppercase tracking-wider">{t('report.categories')}</div>
          </div>
          <div className="text-center">
            <div className="text-sm font-bold text-foreground">{report.findings.length}</div>
            <div className="text-[9px] text-foreground-muted/50 uppercase tracking-wider">{t('report.findings')}</div>
          </div>
        </div>
      </div>

      {/* ── Author & PR Stats ── */}
      {run?.prAuthor && (
        <div className="rounded-lg border border-white/[0.06] bg-white/[0.03] px-4 py-3 flex items-center gap-3">
          {run.prAuthorAvatar ? (
            <img
              src={run.prAuthorAvatar}
              alt={run.prAuthor}
              className="w-8 h-8 rounded-full shrink-0"
            />
          ) : (
            <div className="w-8 h-8 rounded-full bg-white/[0.08] flex items-center justify-center shrink-0">
              <GitPullRequest className="w-4 h-4 text-foreground-muted/50" />
            </div>
          )}
          <div className="flex-1 min-w-0">
            <div className="text-xs font-medium text-foreground">@{run.prAuthor}</div>
            {authorStats && (
              <div className="text-[10px] text-foreground-muted/50">
                {t('report.analysisCount', { count: authorStats.totalRuns })} · {t('report.avgScore', { score: Math.round(authorStats.avgOverallScore) })}
              </div>
            )}
          </div>
          <div className="flex items-center gap-3 text-[11px] shrink-0">
            {run.prAdditions != null && (
              <span className="flex items-center gap-0.5 text-green-400">
                <Plus className="w-3 h-3" />{run.prAdditions}
              </span>
            )}
            {run.prDeletions != null && (
              <span className="flex items-center gap-0.5 text-red-400">
                <Minus className="w-3 h-3" />{run.prDeletions}
              </span>
            )}
            {run.prChangedFiles != null && (
              <span className="flex items-center gap-0.5 text-foreground-muted/60">
                <FileCode className="w-3 h-3" />{t('report.filesCount', { count: run.prChangedFiles })}
              </span>
            )}
          </div>
        </div>
      )}

      {/* ── Radar + Categories ── */}
      <div>
        <SectionHeader icon={Layers} title={t('report.breakdown')} />
        <div className={cn('flex gap-4', hasRadar ? 'items-start' : '')}>
          {hasRadar && (
            <div className="shrink-0 rounded-xl bg-white/[0.02] border border-white/[0.06] p-3 flex items-center justify-center">
              <RadarChart categories={report.categories} />
            </div>
          )}
          <div className={cn('grid gap-2.5 flex-1 content-start', hasRadar ? 'grid-cols-2' : 'grid-cols-3')}>
            {report.categories.map((cat) => (
              <CategoryCard
                key={cat.name}
                category={cat}
                authorAvg={authorAvgMap.get(cat.name)}
                projectAvg={projectAvgMap.get(cat.name)}
              />
            ))}
          </div>
        </div>
      </div>

      {/* ── Findings ── */}
      {report.findings.length > 0 && (
        <div>
          <SectionHeader icon={Search} title={t('report.findings')} />
          <div className="rounded-xl bg-white/[0.02] border border-white/[0.06] px-4 py-1">
            {report.findings.map((f, i) => (
              <FindingRow key={i} finding={f} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// ReportPanel — Entry point with fallbacks
// -----------------------------------------------------------------------------

interface ReportPanelProps {
  steps: PipelineStepDto[] | null
  run: PipelineRun | null
  analysisContext: RunAnalysisContextDto | null
}

export function ReportPanel({ steps, run, analysisContext }: ReportPanelProps) {
  const { t } = useTranslation('agents')
  const { report, rawOutput } = useMemo(() => {
    if (!steps || steps.length === 0) return { report: null, rawOutput: null }

    const reporterStep = findReporterStep(steps)
    if (!reporterStep?.output) return { report: null, rawOutput: null }

    const parsed = parseReportFromOutput(reporterStep.output)
    return { report: parsed, rawOutput: reporterStep.output }
  }, [steps])

  if (!steps || !rawOutput) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-foreground-muted/40">
        <BarChart3 className="w-8 h-8 mb-2 opacity-30" />
        <span className="text-xs">{t('report.runPipeline')}</span>
      </div>
    )
  }

  if (!report) {
    return (
      <div className="h-full overflow-y-auto p-4">
        <MarkdownRenderer content={rawOutput} />
      </div>
    )
  }

  return <ReportDashboard report={report} run={run} analysisContext={analysisContext} />
}
