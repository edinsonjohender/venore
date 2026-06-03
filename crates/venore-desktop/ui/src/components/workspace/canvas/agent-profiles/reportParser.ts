// =============================================================================
// reportParser — Extract structured JSON report from reporter step output
// =============================================================================

import type { PipelineStepDto } from '@/lib/tauri'
import type { PipelineReport, CategoryStatus, FindingSeverity } from './types'

/** Find the last completed reporter step */
export function findReporterStep(steps: PipelineStepDto[]): PipelineStepDto | null {
  for (let i = steps.length - 1; i >= 0; i--) {
    if (steps[i].stage === 'reporter' && steps[i].status === 'completed') {
      return steps[i]
    }
  }
  return null
}

/** Extract content from ```json ... ``` fence blocks */
function extractJsonBlock(text: string): string | null {
  const match = text.match(/```json\s*\n([\s\S]*?)\n\s*```/)
  if (match?.[1]) return match[1].trim()
  return null
}

const VALID_STATUSES: CategoryStatus[] = ['good', 'warning', 'critical']
const VALID_SEVERITIES: FindingSeverity[] = ['critical', 'warning', 'info', 'good']

/** Basic shape validation for the parsed report */
function isValidReport(obj: unknown): obj is PipelineReport {
  if (typeof obj !== 'object' || obj === null) return false
  const o = obj as Record<string, unknown>

  if (typeof o.overall_score !== 'number' || o.overall_score < 0 || o.overall_score > 100) return false
  if (typeof o.summary !== 'string') return false
  if (!Array.isArray(o.categories) || o.categories.length === 0) return false
  if (!Array.isArray(o.findings)) return false

  for (const cat of o.categories) {
    if (typeof cat !== 'object' || cat === null) return false
    const c = cat as Record<string, unknown>
    if (typeof c.name !== 'string') return false
    if (typeof c.score !== 'number' || c.score < 0 || c.score > 100) return false
    if (!VALID_STATUSES.includes(c.status as CategoryStatus)) return false
    if (typeof c.findings_count !== 'number') return false
  }

  for (const f of o.findings) {
    if (typeof f !== 'object' || f === null) return false
    const fi = f as Record<string, unknown>
    if (typeof fi.title !== 'string') return false
    if (typeof fi.category !== 'string') return false
    if (!VALID_SEVERITIES.includes(fi.severity as FindingSeverity)) return false
    if (typeof fi.description !== 'string') return false
  }

  return true
}

/** Parse a structured report from the reporter step output. Returns null if parsing fails. */
export function parseReportFromOutput(output: string): PipelineReport | null {
  const jsonStr = extractJsonBlock(output)
  if (!jsonStr) return null

  try {
    const parsed = JSON.parse(jsonStr)
    if (isValidReport(parsed)) return parsed
    return null
  } catch {
    return null
  }
}
