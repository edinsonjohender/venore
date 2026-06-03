// =============================================================================
// ResearchFooter — Status controls: start/pause/stop, elapsed time, progress
// =============================================================================
// Connected to the Research Engine backend (multi-agent orchestrator).
// No longer uses the chat for research execution.

import { useState, useEffect, useRef, useCallback } from 'react'
import { Play, Pause, Square, Clock, Hexagon, Loader2 } from 'lucide-react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import { tauriApi } from '@/lib/tauri'
import type { Feature } from './mock-data'

type ResearchStatus = 'idle' | 'running' | 'paused' | 'completed'

interface ResearchFooterProps {
  feature: Feature
  projectPath?: string
  projectId?: string
}

function formatElapsed(seconds: number): string {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}h ${m}m ${s}s`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

export function ResearchFooter({ feature }: ResearchFooterProps) {
  const [status, setStatus] = useState<ResearchStatus>('idle')
  const [starting, setStarting] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [phase, setPhase] = useState('')
  const [runId, setRunId] = useState<string | null>(null)
  const [workersActive, setWorkersActive] = useState(0)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const { hexagons } = feature
  const total = hexagons.length
  const avgPct = total > 0 ? Math.round(hexagons.reduce((s, h) => s + h.percentage, 0) / total) : 0
  const completed = hexagons.filter((h) => h.percentage >= 85).length
  const deadEnds = hexagons.filter((h) => h.isDeadEnd).length

  // Timer
  const startTimer = useCallback(() => {
    if (intervalRef.current) return
    intervalRef.current = setInterval(() => setElapsed((e) => e + 1), 1000)
  }, [])

  const stopTimer = useCallback(() => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current)
      intervalRef.current = null
    }
  }, [])

  useEffect(() => () => stopTimer(), [stopTimer])

  // Recover state on mount
  useEffect(() => {
    tauriApi.getResearchStatus(feature.id).then((result) => {
      if (!result) return
      setRunId(result.runId)
      if (result.status === 'running') {
        setStatus('running')
        setPhase(result.phase)
        setElapsed(Math.floor(result.durationMs / 1000))
        startTimer()
      } else if (result.status === 'paused') {
        setStatus('paused')
        setPhase(result.phase)
        setElapsed(Math.floor(result.durationMs / 1000))
      }
    }).catch(() => {/* no active run */})
  }, [feature.id, startTimer])

  // Listen for research events
  useEffect(() => {
    const unlisteners: UnlistenFn[] = []

    listen<{ run_id: string }>('research:run-completed', () => {
      setStatus('completed')
      stopTimer()
      setWorkersActive(0)
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string; error: string }>('research:run-failed', (ev) => {
      setStatus('idle')
      stopTimer()
      setWorkersActive(0)
      toast.error(`Research failed: ${ev.payload.error}`)
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string }>('research:run-paused', () => {
      setStatus('paused')
      stopTimer()
      setWorkersActive(0)
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string; from: string; to: string }>('research:phase-transition', (ev) => {
      setPhase(ev.payload.to)
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string; worker_id: string }>('research:worker-started', () => {
      setWorkersActive((n) => n + 1)
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string; worker_id: string }>('research:worker-completed', () => {
      setWorkersActive((n) => Math.max(0, n - 1))
    }).then((fn) => unlisteners.push(fn))

    listen<{ run_id: string; worker_id: string }>('research:worker-failed', () => {
      setWorkersActive((n) => Math.max(0, n - 1))
    }).then((fn) => unlisteners.push(fn))

    return () => { unlisteners.forEach((fn) => fn()) }
  }, [stopTimer])

  const handleStart = async () => {
    if (starting) return
    setStarting(true)
    setStatus('running')
    setElapsed(0)
    setPhase('decomposing')
    startTimer()

    try {
      const result = await tauriApi.startResearch({ featureId: feature.id })
      if (result) setRunId(result.runId)
    } catch (err) {
      toast.error('Failed to start research')
      setStatus('idle')
      stopTimer()
    } finally {
      setStarting(false)
    }
  }

  const handlePause = async () => {
    if (!runId) return
    setStatus('paused')
    stopTimer()
    try {
      await tauriApi.pauseResearch(runId)
    } catch (err) {
      toast.error('Failed to pause research')
    }
  }

  const handleResume = async () => {
    if (!runId) return
    // Resume is not yet implemented in backend — would need resume_research command
    // For now, start a new run
    toast.error('Resume not yet implemented — start a new research run')
  }

  const handleStop = async () => {
    if (!runId) return
    setStatus('idle')
    stopTimer()
    setElapsed(0)
    setWorkersActive(0)
    try {
      await tauriApi.stopResearch(runId)
      setRunId(null)
    } catch (err) {
      toast.error('Failed to stop research')
    }
  }

  return (
    <div className="shrink-0 border-t border-border/50 bg-background-secondary">
      {/* Progress bar */}
      <div className="h-0.5 bg-white/5">
        <div
          className={cn(
            'h-full transition-all duration-500',
            status === 'running' ? 'bg-brand' : status === 'paused' ? 'bg-amber-500' : 'bg-brand/50',
          )}
          style={{ width: `${avgPct}%` }}
        />
      </div>

      <div className="flex items-center gap-4 px-4 py-1.5">
        {/* Controls */}
        <div className="flex items-center gap-1">
          {status === 'idle' && (
            <button
              onClick={handleStart}
              disabled={starting}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-brand/15 text-brand hover:bg-brand/25 disabled:opacity-40 transition-colors"
            >
              <Play className="w-3 h-3" />
              {starting ? 'Starting…' : 'Start'}
            </button>
          )}
          {status === 'running' && (
            <>
              <button
                onClick={handlePause}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-amber-500/15 text-amber-400 hover:bg-amber-500/25 transition-colors"
              >
                <Pause className="w-3 h-3" />
                Pause
              </button>
              <button
                onClick={handleStop}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors"
              >
                <Square className="w-3 h-3" />
                Stop
              </button>
            </>
          )}
          {status === 'paused' && (
            <>
              <button
                onClick={handleResume}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
              >
                <Play className="w-3 h-3" />
                Resume
              </button>
              <button
                onClick={handleStop}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors"
              >
                <Square className="w-3 h-3" />
                Stop
              </button>
            </>
          )}
          {status === 'completed' && (
            <span className="flex items-center gap-1.5 text-xs font-medium text-emerald-400">
              Completed
            </span>
          )}
        </div>

        {/* Status indicator */}
        {status === 'running' && (
          <Loader2 className="w-3 h-3 text-brand animate-spin" />
        )}

        {/* Phase badge */}
        {phase && status !== 'idle' && (
          <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-brand/10 text-brand capitalize">
            {phase}
          </span>
        )}

        {/* Workers count */}
        {workersActive > 0 && (
          <span className="text-[10px] text-foreground-muted">
            {workersActive} worker{workersActive > 1 ? 's' : ''}
          </span>
        )}

        {/* Separator */}
        <div className="w-px h-3.5 bg-border/50" />

        {/* Stats */}
        <div className="flex items-center gap-4 text-[11px] text-foreground-muted">
          <span className="flex items-center gap-1.5">
            <Hexagon className="w-3 h-3" />
            {completed}/{total} resolved
          </span>
          {deadEnds > 0 && (
            <span className="text-red-400">{deadEnds} dead ends</span>
          )}
          <span>{avgPct}% avg</span>
        </div>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Elapsed time */}
        {(status === 'running' || status === 'paused' || elapsed > 0) && (
          <span className="flex items-center gap-1.5 text-[11px] text-foreground-muted">
            <Clock className="w-3 h-3" />
            {formatElapsed(elapsed)}
          </span>
        )}
      </div>
    </div>
  )
}
