// =============================================================================
// ReportTab — Research report structured by objective type
// =============================================================================
// Each objective (validate/understand/compare/decide/explore) produces a
// different report template with appropriate visuals and text structure.

import { cn } from '@/lib/utils'
import { PHASE_COLORS, PHASE_LABELS } from './hex-colors'
import type { Feature, Hexagon, HexPhase, ResearchObjective } from './mock-data'

interface ReportTabProps {
  feature: Feature
}

// -----------------------------------------------------------------------------
// Shared helpers
// -----------------------------------------------------------------------------

function getPhaseHexagons(hexagons: Hexagon[], phase: HexPhase) {
  return hexagons.filter((h) => h.phase === phase)
}

function getDeadEnds(hexagons: Hexagon[]) {
  return hexagons.filter((h) => h.isDeadEnd)
}

function avgPercentage(hexagons: Hexagon[]) {
  if (hexagons.length === 0) return 0
  return Math.round(hexagons.reduce((s, h) => s + h.percentage, 0) / hexagons.length)
}

const OBJECTIVE_LABELS: Record<ResearchObjective, string> = {
  validate: 'Feasibility Validation',
  understand: 'Problem Space Analysis',
  compare: 'Alternatives Comparison',
  decide: 'GO / NO-GO Decision',
  explore: 'Open Exploration',
}

// -----------------------------------------------------------------------------
// Executive Summary (shared across all)
// -----------------------------------------------------------------------------

function ExecutiveSummary({ feature }: { feature: Feature }) {
  const { hexagons } = feature
  const avg = avgPercentage(hexagons)
  const deadEnds = getDeadEnds(hexagons)
  const highRisk = hexagons.filter((h) => h.risk === 'high')

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Executive Summary</h2>
      <div className="rounded-lg border border-border/50 bg-background-secondary p-4">
        <p className="text-sm text-foreground leading-relaxed">
          Research on <strong>{feature.name}</strong> has reached <strong>{avg}%</strong> average progress
          across {hexagons.length} investigation points.
          {deadEnds.length > 0 && ` ${deadEnds.length} path${deadEnds.length > 1 ? 's' : ''} were identified as dead ends and discarded.`}
          {highRisk.length > 0 && ` ${highRisk.length} high-risk area${highRisk.length > 1 ? 's' : ''} require attention.`}
          {avg >= 80 && highRisk.length === 0 && ' Evidence suggests this is ready to move forward.'}
          {avg >= 50 && avg < 80 && ' More investigation is needed before a confident decision can be made.'}
          {avg < 50 && ' Research is still in early stages.'}
        </p>
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Phase Findings (shared)
// -----------------------------------------------------------------------------

function PhaseFindings({ feature }: { feature: Feature }) {
  const phases: HexPhase[] = ['discover', 'define', 'validate', 'conclude']

  return (
    <section className="space-y-3">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Findings by Phase</h2>
      {phases.map((phase) => {
        const phaseHex = getPhaseHexagons(feature.hexagons, phase)
        if (phaseHex.length === 0) return null
        const color = PHASE_COLORS[phase]

        return (
          <div key={phase} className="rounded-lg border border-border/50 bg-background-secondary p-4 space-y-2">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full" style={{ backgroundColor: color }} />
              <span className="text-xs font-medium text-foreground capitalize">{PHASE_LABELS[phase]}</span>
              <span className="text-[10px] text-foreground-subtle">{phaseHex.length} points — {avgPercentage(phaseHex)}% avg</span>
            </div>
            <div className="space-y-1.5">
              {phaseHex.map((hex) => (
                <div key={hex.id} className="flex items-start gap-2">
                  <span className="text-[10px] text-foreground-subtle w-8 shrink-0 text-right mt-0.5">{hex.percentage}%</span>
                  <div className="min-w-0">
                    <span className="text-[11px] text-foreground">{hex.title}</span>
                    {hex.notes && (
                      <p className="text-[10px] text-foreground-muted mt-0.5">{hex.notes}</p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )
      })}
    </section>
  )
}

// -----------------------------------------------------------------------------
// Dead Ends Section (shared)
// -----------------------------------------------------------------------------

function DeadEndsSection({ hexagons }: { hexagons: Hexagon[] }) {
  const deadEnds = getDeadEnds(hexagons)
  if (deadEnds.length === 0) return null

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Dead Ends</h2>
      <div className="rounded-lg border border-red-500/20 bg-red-500/5 p-4 space-y-2">
        {deadEnds.map((hex) => (
          <div key={hex.id} className="space-y-0.5">
            <span className="text-[11px] text-foreground font-medium">{hex.title}</span>
            <p className="text-[10px] text-foreground-muted">{hex.notes || hex.description}</p>
          </div>
        ))}
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Evidence Section (shared)
// -----------------------------------------------------------------------------

function EvidenceSection({ feature }: { feature: Feature }) {
  if (feature.evidence.length === 0) return null

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Sources & Evidence</h2>
      <div className="rounded-lg border border-border/50 bg-background-secondary p-4 space-y-1.5">
        {feature.evidence.map((ev) => (
          <div key={ev.id} className="flex items-start gap-2">
            <span className="text-[10px] text-foreground-subtle shrink-0 uppercase w-8">{ev.type}</span>
            <div className="min-w-0">
              <span className="text-[11px] text-foreground">{ev.title}</span>
              {ev.url && <p className="text-[10px] text-brand truncate">{ev.url}</p>}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Visual: Comparison Table (for "compare" objective)
// -----------------------------------------------------------------------------

function ComparisonTable({ feature }: { feature: Feature }) {
  // Group hexagons that represent alternatives (top-level branches)
  const roots = feature.hexagons.filter((h) => h.parentId === feature.hexagons.find((r) => r.parentId === null)?.id)
  if (roots.length < 2) return null

  const criteria = ['Progress', 'Confidence', 'Risk', 'Phase', 'Status']

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Comparison Matrix</h2>
      <div className="rounded-lg border border-border/50 overflow-hidden">
        <table className="w-full text-[11px]">
          <thead>
            <tr className="bg-background-secondary">
              <th className="text-left px-3 py-2 text-foreground-muted font-medium">Criteria</th>
              {roots.map((r) => (
                <th key={r.id} className="text-left px-3 py-2 text-foreground font-medium">{r.title}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {criteria.map((c, i) => (
              <tr key={c} className={i % 2 === 0 ? 'bg-background-secondary/50' : ''}>
                <td className="px-3 py-1.5 text-foreground-muted">{c}</td>
                {roots.map((r) => (
                  <td key={r.id} className="px-3 py-1.5 text-foreground">
                    {c === 'Progress' && `${r.percentage}%`}
                    {c === 'Confidence' && (
                      <span className={cn(
                        'px-1.5 py-0.5 rounded text-[9px] font-medium capitalize',
                        r.confidence === 'high' ? 'bg-emerald-500/15 text-emerald-400'
                          : r.confidence === 'medium' ? 'bg-amber-500/15 text-amber-400'
                            : 'bg-red-500/15 text-red-400',
                      )}>{r.confidence}</span>
                    )}
                    {c === 'Risk' && (
                      <span className={cn(
                        'px-1.5 py-0.5 rounded text-[9px] font-medium capitalize',
                        r.risk === 'low' ? 'bg-emerald-500/15 text-emerald-400'
                          : r.risk === 'medium' ? 'bg-amber-500/15 text-amber-400'
                            : 'bg-red-500/15 text-red-400',
                      )}>{r.risk}</span>
                    )}
                    {c === 'Phase' && (
                      <span className="flex items-center gap-1.5">
                        <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: PHASE_COLORS[r.phase] }} />
                        <span className="capitalize">{r.phase}</span>
                      </span>
                    )}
                    {c === 'Status' && (r.isDeadEnd ? '✕ Dead End' : '● Active')}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Visual: Risk Matrix (for "validate" objective)
// -----------------------------------------------------------------------------

function RiskMatrix({ hexagons }: { hexagons: Hexagon[] }) {
  const activeHex = hexagons.filter((h) => !h.isDeadEnd)

  const riskLevels = ['low', 'medium', 'high'] as const
  const confidenceLevels = ['high', 'medium', 'low'] as const

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Risk vs Confidence Matrix</h2>
      <div className="rounded-lg border border-border/50 overflow-hidden">
        <table className="w-full text-[10px]">
          <thead>
            <tr className="bg-background-secondary">
              <th className="px-3 py-2 text-foreground-muted font-medium text-left">Risk ↓ / Confidence →</th>
              {confidenceLevels.map((c) => (
                <th key={c} className="px-3 py-2 text-foreground-muted font-medium capitalize text-center">{c}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {riskLevels.map((risk) => (
              <tr key={risk}>
                <td className="px-3 py-2 text-foreground-muted capitalize font-medium">{risk}</td>
                {confidenceLevels.map((conf) => {
                  const cellHex = activeHex.filter((h) => h.risk === risk && h.confidence === conf)
                  const bgColor = risk === 'high' && conf === 'low' ? 'bg-red-500/10'
                    : risk === 'low' && conf === 'high' ? 'bg-emerald-500/10'
                      : risk === 'high' || conf === 'low' ? 'bg-amber-500/5'
                        : ''
                  return (
                    <td key={conf} className={cn('px-2 py-2 text-center', bgColor)}>
                      {cellHex.length > 0 ? (
                        <div className="flex flex-col gap-0.5">
                          {cellHex.map((h) => (
                            <span key={h.id} className="text-[9px] text-foreground truncate block">{h.title}</span>
                          ))}
                        </div>
                      ) : (
                        <span className="text-foreground-subtle">—</span>
                      )}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Visual: Progress Overview (for "decide" objective)
// -----------------------------------------------------------------------------

function DecisionScorecard({ feature }: { feature: Feature }) {
  const { hexagons } = feature
  const avg = avgPercentage(hexagons)
  const highConf = hexagons.filter((h) => h.confidence === 'high').length
  const highRisk = hexagons.filter((h) => h.risk === 'high').length
  const deadEnds = getDeadEnds(hexagons).length

  const decision = avg >= 80 && highRisk === 0
    ? 'go'
    : avg >= 50 || highRisk > 0
      ? 'go-risk'
      : 'no-go'

  const decisionConfig = {
    go: { label: 'GO', color: 'text-emerald-400', bg: 'bg-emerald-500/10', border: 'border-emerald-500/30' },
    'go-risk': { label: 'GO WITH RISKS', color: 'text-amber-400', bg: 'bg-amber-500/10', border: 'border-amber-500/30' },
    'no-go': { label: 'NO-GO', color: 'text-red-400', bg: 'bg-red-500/10', border: 'border-red-500/30' },
  }
  const d = decisionConfig[decision]

  const factors = [
    { label: 'Average progress', value: `${avg}%`, good: avg >= 70 },
    { label: 'High confidence points', value: `${highConf}/${hexagons.length}`, good: highConf > hexagons.length / 2 },
    { label: 'High risk areas', value: `${highRisk}`, good: highRisk === 0 },
    { label: 'Dead ends', value: `${deadEnds}`, good: deadEnds <= 1 },
    { label: 'Evidence collected', value: `${feature.evidence.length}`, good: feature.evidence.length >= 3 },
  ]

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Decision</h2>
      <div className={cn('rounded-lg border p-5 text-center', d.bg, d.border)}>
        <div className={cn('text-2xl font-bold', d.color)}>{d.label}</div>
      </div>
      <div className="rounded-lg border border-border/50 overflow-hidden">
        <table className="w-full text-[11px]">
          <tbody>
            {factors.map((f, i) => (
              <tr key={f.label} className={i % 2 === 0 ? 'bg-background-secondary/50' : ''}>
                <td className="px-3 py-1.5 text-foreground-muted">{f.label}</td>
                <td className="px-3 py-1.5 text-right font-medium">
                  <span className={f.good ? 'text-emerald-400' : 'text-amber-400'}>{f.value}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Visual: Phase Distribution Bar (for "understand" / "explore")
// -----------------------------------------------------------------------------

function PhaseDistribution({ hexagons }: { hexagons: Hexagon[] }) {
  const phases: HexPhase[] = ['discover', 'define', 'validate', 'conclude']
  const total = hexagons.length

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Research Coverage</h2>
      <div className="rounded-lg border border-border/50 bg-background-secondary p-4">
        {/* Stacked bar */}
        <div className="h-6 rounded-full overflow-hidden flex bg-white/5 mb-3">
          {phases.map((phase) => {
            const count = getPhaseHexagons(hexagons, phase).length
            const pct = total > 0 ? (count / total) * 100 : 0
            if (pct === 0) return null
            return (
              <div
                key={phase}
                className="h-full flex items-center justify-center"
                style={{
                  width: `${pct}%`,
                  backgroundColor: PHASE_COLORS[phase],
                  opacity: 0.6,
                }}
              >
                {pct >= 10 && <span className="text-[9px] text-white font-medium">{Math.round(pct)}%</span>}
              </div>
            )
          })}
        </div>
        {/* Legend */}
        <div className="flex flex-wrap gap-3">
          {phases.map((phase) => {
            const count = getPhaseHexagons(hexagons, phase).length
            return (
              <span key={phase} className="flex items-center gap-1.5 text-[10px] text-foreground-muted">
                <span className="w-2 h-2 rounded-full" style={{ backgroundColor: PHASE_COLORS[phase] }} />
                {PHASE_LABELS[phase]} ({count})
              </span>
            )
          })}
        </div>
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Visual: Discovery Map (for "explore" objective)
// -----------------------------------------------------------------------------

function DiscoveryMap({ feature }: { feature: Feature }) {
  // Group hexagons by their root branch
  const root = feature.hexagons.find((h) => h.parentId === null)
  if (!root) return null
  const branches = feature.hexagons.filter((h) => h.parentId === root.id)

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Discovery Map</h2>
      <div className="rounded-lg border border-border/50 bg-background-secondary p-4 space-y-3">
        {branches.map((branch) => {
          const children = feature.hexagons.filter((h) => h.parentId === branch.id)
          const color = PHASE_COLORS[branch.phase]
          return (
            <div key={branch.id}>
              <div className="flex items-center gap-2 mb-1">
                <span className="w-2 h-2 rounded-full" style={{ backgroundColor: color }} />
                <span className="text-[11px] font-medium text-foreground">{branch.title}</span>
                <span className="text-[10px] text-foreground-subtle">{branch.percentage}%</span>
              </div>
              {children.length > 0 && (
                <div className="ml-4 border-l border-border/30 pl-3 space-y-1">
                  {children.map((child) => (
                    <div key={child.id} className="flex items-center gap-2">
                      <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: PHASE_COLORS[child.phase] }} />
                      <span className="text-[10px] text-foreground-muted">{child.title}</span>
                      <span className="text-[9px] text-foreground-subtle">{child.percentage}%</span>
                      {child.isDeadEnd && <span className="text-[9px] text-red-400">✕</span>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Open Questions (shared)
// -----------------------------------------------------------------------------

function OpenQuestions({ hexagons }: { hexagons: Hexagon[] }) {
  const lowProgress = hexagons.filter((h) => h.percentage < 30 && !h.isDeadEnd)
  if (lowProgress.length === 0) return null

  return (
    <section className="space-y-2">
      <h2 className="text-xs font-medium text-foreground-muted uppercase tracking-wider">Open Questions</h2>
      <div className="rounded-lg border border-border/50 bg-background-secondary p-4 space-y-1.5">
        {lowProgress.map((hex) => (
          <div key={hex.id} className="flex items-start gap-2">
            <span className="text-foreground-subtle mt-0.5">?</span>
            <div>
              <span className="text-[11px] text-foreground">{hex.title}</span>
              <span className="text-[10px] text-foreground-subtle ml-1.5">({hex.percentage}%)</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Main ReportTab — selects template by objective
// -----------------------------------------------------------------------------

export function ReportTab({ feature }: ReportTabProps) {
  const { objective } = feature

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto p-6 space-y-6">
        {/* Report header */}
        <div className="space-y-1">
          <span className="text-[10px] text-foreground-subtle uppercase tracking-wider">
            {OBJECTIVE_LABELS[objective]} Report
          </span>
          <h1 className="text-sm font-medium text-foreground">{feature.name}</h1>
          <p className="text-[11px] text-foreground-muted">{feature.description}</p>
        </div>

        {/* Executive Summary — always */}
        <ExecutiveSummary feature={feature} />

        {/* Visual section — depends on objective */}
        {objective === 'compare' && <ComparisonTable feature={feature} />}
        {objective === 'validate' && <RiskMatrix hexagons={feature.hexagons} />}
        {objective === 'decide' && <DecisionScorecard feature={feature} />}
        {(objective === 'understand' || objective === 'explore') && <PhaseDistribution hexagons={feature.hexagons} />}
        {objective === 'explore' && <DiscoveryMap feature={feature} />}

        {/* Text sections — shared */}
        <PhaseFindings feature={feature} />
        <DeadEndsSection hexagons={feature.hexagons} />
        <OpenQuestions hexagons={feature.hexagons} />
        <EvidenceSection feature={feature} />
      </div>
    </div>
  )
}
