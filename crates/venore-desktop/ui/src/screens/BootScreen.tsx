// =============================================================================
// BootScreen - Initial loading screen during app initialization
// =============================================================================
// Shows:
// - Animated spinner
// - Current phase (booting, checking services, ready)
// - List of checks with status icons
// - Total duration on completion

import { useEffect, useState } from 'react'
import { CheckCircle, XCircle, Loader2, Circle, RotateCcw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { tauriApi } from '../lib/tauri'
import { useAIConfigStore } from '../stores/aiConfigStore'
import { useGithubAuthStore } from '../stores/githubAuthStore'
import { isMacOS } from '../lib/platform'
import { WindowControls } from '../components/WindowControls'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

type CheckStatus = 'pending' | 'running' | 'success' | 'failed'

interface Check {
  id: string
  name: string
  status: CheckStatus
  duration?: number
  error?: string
}

type BootPhase = 'booting' | 'checking' | 'ready' | 'error'

interface BootScreenProps {
  /** Called when boot completes successfully */
  onReady?: () => void
  /** Called if boot fails */
  onError?: (error: string) => void
}

// -----------------------------------------------------------------------------
// Phase Info
// -----------------------------------------------------------------------------

const PHASE_KEYS: Record<BootPhase, { title: string; description: string }> = {
  booting: {
    title: 'boot.phases.booting.title',
    description: 'boot.phases.booting.description',
  },
  checking: {
    title: 'boot.phases.checking.title',
    description: 'boot.phases.checking.description',
  },
  ready: {
    title: 'boot.phases.ready.title',
    description: 'boot.phases.ready.description',
  },
  error: {
    title: 'boot.phases.error.title',
    description: 'boot.phases.error.description',
  },
}

// -----------------------------------------------------------------------------
// Check Status Icon
// -----------------------------------------------------------------------------

function CheckStatusIcon({ status }: { status: CheckStatus }) {
  switch (status) {
    case 'pending':
      return <Circle className="w-4 h-4 text-foreground-muted/50" />
    case 'running':
      return <Loader2 className="w-4 h-4 text-brand animate-spin" />
    case 'success':
      return <CheckCircle className="w-4 h-4 text-brand" />
    case 'failed':
      return <XCircle className="w-4 h-4 text-semantic-error" />
  }
}

// -----------------------------------------------------------------------------
// Check List Item
// -----------------------------------------------------------------------------

function CheckItem({ check }: { check: Check }) {
  return (
    <div className="flex items-center gap-3 py-1.5">
      <CheckStatusIcon status={check.status} />
      <span
        className={`text-sm flex-1 ${
          check.status === 'running'
            ? 'text-foreground'
            : check.status === 'success'
            ? 'text-foreground-muted'
            : check.status === 'failed'
            ? 'text-semantic-error'
            : 'text-foreground-muted/50'
        }`}
      >
        {check.name}
      </span>
      {check.duration !== undefined && check.status === 'success' && (
        <span className="text-xs text-foreground-muted/50">
          {check.duration}ms
        </span>
      )}
      {check.error && (
        <span className="text-xs text-semantic-error truncate max-w-[200px]">
          {check.error}
        </span>
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Global flag to prevent duplicate boot sequence
// -----------------------------------------------------------------------------

let isBootSequenceRunning = false

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function BootScreen({ onReady, onError }: BootScreenProps) {
  const { t } = useTranslation('screens')
  const [phase, setPhase] = useState<BootPhase>('booting')
  const [checks, setChecks] = useState<Check[]>([])
  const [totalDuration, setTotalDuration] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Run boot sequence on mount (only once, even in React Strict Mode)
  useEffect(() => {
    if (!isBootSequenceRunning) {
      console.log('🎬 [BootScreen] Starting boot sequence')
      runBootSequence()
    } else {
      console.log('⚠️ [BootScreen] Boot sequence already running, skipping')
    }
  }, [])

  // Call onReady when phase changes to ready
  useEffect(() => {
    if (phase === 'ready' && onReady) {
      // Small delay to show the success state
      const timer = setTimeout(() => {
        onReady()
      }, 500)
      return () => clearTimeout(timer)
    }
  }, [phase, onReady])

  // Call onError when phase changes to error
  useEffect(() => {
    if (phase === 'error' && error && onError) {
      onError(error)
    }
  }, [phase, error, onError])

  const runBootSequence = async () => {
    // Atomic test-and-set: Check flag and set it in ONE operation
    // This prevents race conditions from React Strict Mode double mounting
    const wasAlreadyRunning = isBootSequenceRunning
    isBootSequenceRunning = true  // Set IMMEDIATELY, before any checks

    // Guard against double execution (check AFTER setting flag)
    if (wasAlreadyRunning) {
      console.warn('⚠️ [BootScreen] Boot sequence already running, ignoring duplicate call')
      return
    }

    console.log('✅ [BootScreen] Starting boot sequence (flag now set to prevent duplicates)')

    const startTime = Date.now()

    try {
      // Initialize checks
      const initialChecks: Check[] = [
        { id: 'backend', name: t('boot.checks.backend'), status: 'pending' },
        { id: 'database', name: t('boot.checks.database'), status: 'pending' },
        { id: 'llm_gateway', name: t('boot.checks.llmGateway'), status: 'pending' },
        { id: 'ai_config', name: t('boot.checks.aiConfiguration'), status: 'pending' },
      ]
      setChecks(initialChecks)

      // Phase 1: Booting
      setPhase('booting')
      await sleep(300)

      // Phase 2: Initialize backend (this is where AppState::new() runs)
      setPhase('checking')

      // Step 1: Initialize backend (creates AppState, connects DB, creates LLM gateway)
      setChecks(prev => updateCheck(prev, 'backend', { status: 'running' }))
      const initStart = Date.now()
      try {
        const initResult = await tauriApi.initializeBackend()
        const initDuration = Date.now() - initStart
        if (initResult.success) {
          setChecks(prev => updateCheck(prev, 'backend', {
            status: 'success',
            duration: initDuration
          }))
        } else {
          throw new Error(initResult.message)
        }
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : t('boot.errors.backendInitFailed')
        setChecks(prev => updateCheck(prev, 'backend', {
          status: 'failed',
          error: errorMessage
        }))
        throw err
      }

      // Step 2: Verify database
      setChecks(prev => updateCheck(prev, 'database', { status: 'running' }))
      const dbStart = Date.now()
      try {
        const dbResult = await tauriApi.checkDatabase()
        const dbDuration = Date.now() - dbStart
        if (dbResult.success) {
          setChecks(prev => updateCheck(prev, 'database', {
            status: 'success',
            duration: dbDuration
          }))
        } else {
          throw new Error(dbResult.message)
        }
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : t('boot.errors.databaseCheckFailed')
        setChecks(prev => updateCheck(prev, 'database', {
          status: 'failed',
          error: errorMessage
        }))
        throw err
      }

      // Check 3: LLM Gateway
      setChecks(prev => updateCheck(prev, 'llm_gateway', { status: 'running' }))
      const llmStart = Date.now()
      try {
        const llmResult = await tauriApi.checkLlmGateway()
        const llmDuration = Date.now() - llmStart
        if (llmResult.success) {
          setChecks(prev => updateCheck(prev, 'llm_gateway', {
            status: 'success',
            duration: llmDuration
          }))
        } else {
          throw new Error(llmResult.message)
        }
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : t('boot.errors.llmGatewayCheckFailed')
        setChecks(prev => updateCheck(prev, 'llm_gateway', {
          status: 'failed',
          error: errorMessage
        }))
        throw err
      }

      // Step 4: Load AI Configuration into store
      setChecks(prev => updateCheck(prev, 'ai_config', { status: 'running' }))
      const aiStart = Date.now()
      try {
        await useAIConfigStore.getState().load()
        const aiDuration = Date.now() - aiStart
        setChecks(prev => updateCheck(prev, 'ai_config', {
          status: 'success',
          duration: aiDuration
        }))
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : t('boot.errors.aiConfigLoadFailed')
        setChecks(prev => updateCheck(prev, 'ai_config', {
          status: 'failed',
          error: errorMessage
        }))
        throw err
      }

      // Validate the GitHub session in the background — keyring token only, no
      // prompts. Fire-and-forget on purpose: it must NOT add latency to the
      // boot or gate the launcher. The cache fills in shortly after; the
      // GitHub panel reads it instead of re-validating on every open.
      void useGithubAuthStore.getState().validate()

      // Phase 3: Ready
      const duration = Date.now() - startTime
      setTotalDuration(duration)
      setPhase('ready')
      console.log('✅ [BootScreen] Boot sequence completed successfully')

    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error'
      setError(errorMessage)
      setPhase('error')
      // Reset flag on error to allow retry (if UI implements retry button)
      isBootSequenceRunning = false
      console.error('❌ [BootScreen] Boot sequence failed:', errorMessage)
    }
  }

  const phaseKeys = PHASE_KEYS[phase]

  return (
    <div className="h-full w-full flex flex-col bg-background select-none">
      {/* Window Controls Bar */}
      <div className={`h-8 border-b border-border flex items-center ${isMacOS ? 'justify-start' : 'justify-end'}`} data-tauri-drag-region>
        <WindowControls />
      </div>

      {/* Boot Content */}
      <div className="flex-1 flex items-center justify-center">
        <div className="flex flex-col items-center gap-6 max-w-md px-4">
        {/* Logo/Icon Area */}
        <div className="relative">
          {phase === 'error' ? (
            <div className="w-16 h-16 rounded-full bg-semantic-error/10 flex items-center justify-center">
              <XCircle className="w-8 h-8 text-semantic-error" />
            </div>
          ) : phase === 'ready' ? (
            <div className="w-16 h-16 rounded-full bg-brand/10 flex items-center justify-center">
              <CheckCircle className="w-8 h-8 text-brand" />
            </div>
          ) : (
            <div className="w-16 h-16 border-4 border-background-tertiary border-t-brand rounded-full animate-spin" />
          )}
        </div>

        {/* Title & Description */}
        <div className="text-center">
          <h1 className="text-xl font-semibold text-foreground mb-1">
            {t(phaseKeys.title)}
          </h1>
          <p className="text-foreground-muted text-sm">
            {t(phaseKeys.description)}
          </p>
        </div>

        {/* Check List */}
        {checks.length > 0 && (
          <div className="w-full space-y-1 p-4 bg-background-secondary/50 rounded-lg border border-border">
            {checks.map((check) => (
              <CheckItem key={check.id} check={check} />
            ))}
          </div>
        )}

        {/* Error Message + Retry */}
        {error && (
          <div className="w-full p-4 bg-semantic-error/10 border border-semantic-error/30 rounded-lg">
            <p className="text-sm text-semantic-error">{error}</p>
          </div>
        )}

        {phase === 'error' && (
          <button
            onClick={() => {
              setError(null)
              setChecks([])
              setTotalDuration(null)
              runBootSequence()
            }}
            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-foreground bg-background-secondary hover:bg-background-tertiary border border-border rounded-lg transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            {t('boot.retry')}
          </button>
        )}

        {/* Duration (on completion) */}
        {totalDuration !== null && phase === 'ready' && (
          <p className="text-xs text-foreground-muted/50">
            {t('boot.loadedIn', { duration: totalDuration })}
          </p>
        )}
        </div>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Utilities
// -----------------------------------------------------------------------------

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

function updateCheck(
  checks: Check[],
  id: string,
  updates: Partial<Check>
): Check[] {
  return checks.map(check =>
    check.id === id ? { ...check, ...updates } : check
  )
}
