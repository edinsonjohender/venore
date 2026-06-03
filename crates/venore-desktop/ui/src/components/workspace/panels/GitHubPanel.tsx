// =============================================================================
// GitHubPanel - PRs/Issues panel with tabs, fetch-on-mount, state filters
// =============================================================================
// Follows ProjectPanel pattern: stats bar + tabs + tab content.
// Handles 3 states: no auth (inline PAT input), no repo detected, ready.

import { useState, useEffect, useCallback } from 'react'
import { useTranslation, Trans } from 'react-i18next'
import { Github, AlertCircle, Loader2, Eye, EyeOff, User } from 'lucide-react'
import type { PanelContentProps } from './registry'
import { GitHubTabs, type GitHubTabId } from './github/GitHubTabs'
import { PullsTab } from './github/PullsTab'
import { IssuesTab } from './github/IssuesTab'
import { setGitHubRefreshHandler } from './github/GitHubHeaderActions'
import { tauriApi } from '@/lib/tauri'
import { useGithubAuthStore } from '@/stores/githubAuthStore'
import type {
  GitHubAuthStatusResponse,
  GitHubDetectRepoResponse,
  GitHubPullRequestDto,
  GitHubIssueDto,
} from '@/lib/tauri'

export function GitHubPanel({ projectPath }: PanelContentProps) {
  const { t } = useTranslation('github')
  const [activeTab, setActiveTab] = useState<GitHubTabId>('pulls')

  // Auth + repo
  const [auth, setAuth] = useState<GitHubAuthStatusResponse | null>(null)
  const [repo, setRepo] = useState<GitHubDetectRepoResponse | null>(null)
  const [setupLoading, setSetupLoading] = useState(true)

  // GCM choice
  const [showPatFallback, setShowPatFallback] = useState(false)
  const [gcmLoading, setGcmLoading] = useState(false)

  // Inline PAT input
  const [patInput, setPatInput] = useState('')
  const [patVisible, setPatVisible] = useState(false)
  const [patError, setPatError] = useState<string | null>(null)
  const [patLoading, setPatLoading] = useState(false)

  // Data
  const [pulls, setPulls] = useState<GitHubPullRequestDto[]>([])
  const [issues, setIssues] = useState<GitHubIssueDto[]>([])
  const [pullsLoading, setPullsLoading] = useState(false)
  const [issuesLoading, setIssuesLoading] = useState(false)
  const [pullsError, setPullsError] = useState<string | null>(null)
  const [issuesError, setIssuesError] = useState<string | null>(null)

  // Connection state cached at boot (keyring token, no prompts).
  const ghLoaded = useGithubAuthStore((s) => s.loaded)
  const ghAuthenticated = useGithubAuthStore((s) => s.authenticated)
  const ghLogin = useGithubAuthStore((s) => s.login)
  const ghName = useGithubAuthStore((s) => s.name)
  const ghAvatar = useGithubAuthStore((s) => s.avatarUrl)
  const applyGhStatus = useGithubAuthStore((s) => s.applyStatus)

  const isReady = auth?.authenticated && repo?.detected

  // --- Detect the repo once per project (independent of auth) ---
  // Kept separate from the auth effect so a settling auth cache doesn't
  // re-trigger the `git remote get-url` detection.
  useEffect(() => {
    if (!projectPath) return
    let cancelled = false
    tauriApi
      .githubDetectRepo({ project_path: projectPath })
      .then((repoRes) => { if (!cancelled) setRepo(repoRes) })
      .catch(() => {
        if (!cancelled) setRepo({ detected: false, owner: null, repo: null } as GitHubDetectRepoResponse)
      })
    return () => { cancelled = true }
  }, [projectPath])

  // --- Resolve auth: cached fast-path, else full check ---
  useEffect(() => {
    if (!projectPath) return
    setSetupLoading(true)
    setShowPatFallback(false)

    // Fast path: boot already confirmed a valid keyring session — use the
    // cache, skip the auth round-trip (and any GCM prompt) entirely.
    if (ghLoaded && ghAuthenticated) {
      setAuth({
        authenticated: true,
        login: ghLogin,
        name: ghName,
        avatar_url: ghAvatar,
        gcm_detected: false, gcm_login: null, gcm_name: null, gcm_avatar_url: null,
      })
      setSetupLoading(false)
      return
    }

    // Not (known) connected → full status check, which still surfaces GCM.
    let cancelled = false
    tauriApi
      .githubAuthStatus()
      .then((authRes) => { if (!cancelled) { setAuth(authRes); setSetupLoading(false) } })
      .catch(() => {
        if (!cancelled) {
          setAuth({
            authenticated: false, login: null, name: null, avatar_url: null,
            gcm_detected: false, gcm_login: null, gcm_name: null, gcm_avatar_url: null,
          })
          setSetupLoading(false)
        }
      })
    return () => { cancelled = true }
  }, [projectPath, ghLoaded, ghAuthenticated, ghLogin, ghName, ghAvatar])

  // --- Inline PAT connect ---
  const handleStorePat = async () => {
    if (!patInput.trim()) return
    setPatError(null)
    setPatLoading(true)
    try {
      const status = await tauriApi.githubStorePat({ token: patInput.trim() })
      setAuth(status)
      applyGhStatus(status)
      setPatInput('')
      setPatVisible(false)
    } catch (err: unknown) {
      setPatError(err instanceof Error ? err.message : 'Invalid token')
    } finally {
      setPatLoading(false)
    }
  }

  // --- Fetch data when ready ---
  const fetchPulls = useCallback(async () => {
    if (!projectPath) return
    setPullsLoading(true)
    setPullsError(null)
    try {
      const res = await tauriApi.githubListPulls({ project_path: projectPath, per_page: 30 })
      setPulls(res.pulls)
    } catch (err: unknown) {
      setPullsError(err instanceof Error ? err.message : 'Failed to load')
    } finally {
      setPullsLoading(false)
    }
  }, [projectPath])

  const fetchIssues = useCallback(async () => {
    if (!projectPath) return
    setIssuesLoading(true)
    setIssuesError(null)
    try {
      const res = await tauriApi.githubListIssues({ project_path: projectPath, per_page: 30 })
      setIssues(res.issues)
    } catch (err: unknown) {
      setIssuesError(err instanceof Error ? err.message : 'Failed to load')
    } finally {
      setIssuesLoading(false)
    }
  }, [projectPath])

  // Auto-fetch when auth+repo are ready
  useEffect(() => {
    if (!isReady) return
    fetchPulls()
    fetchIssues()
  }, [isReady, fetchPulls, fetchIssues])

  // --- Wire refresh button in header ---
  const refreshing = pullsLoading || issuesLoading
  const handleRefresh = useCallback(() => {
    fetchPulls()
    fetchIssues()
  }, [fetchPulls, fetchIssues])

  useEffect(() => {
    setGitHubRefreshHandler(isReady ? handleRefresh : null, refreshing)
    return () => setGitHubRefreshHandler(null, false)
  }, [isReady, handleRefresh, refreshing])

  // --- Not ready states ---
  if (setupLoading) {
    return (
      <div className="flex-1 flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  if (!auth?.authenticated) {
    // GCM detected — show account card + tab toggle
    if (auth?.gcm_detected) {
      const handleAcceptGcm = async () => {
        setGcmLoading(true)
        try {
          const status = await tauriApi.githubAcceptGcm()
          setAuth(status)
        } catch {
          setShowPatFallback(true)
        } finally {
          setGcmLoading(false)
        }
      }

      return (
        <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 h-full">
          {/* Account card */}
          <div className="flex items-center gap-3">
            {auth.gcm_avatar_url ? (
              <img src={auth.gcm_avatar_url} alt={auth.gcm_login ?? ''} className="w-10 h-10 rounded-full" />
            ) : (
              <User className="w-10 h-10 text-foreground-muted/20" />
            )}
            <div>
              <span className="text-xs font-medium text-foreground">@{auth.gcm_login}</span>
              {auth.gcm_name && <p className="text-[10px] text-foreground-muted">{auth.gcm_name}</p>}
              <p className="text-[10px] text-foreground-muted/60">{t('panel.gcmDetected')}</p>
            </div>
          </div>

          <div className="w-full max-w-[280px] space-y-2 mt-1">
            {/* Tab toggle — same pattern as TabsTrigger */}
            <div className="inline-flex items-center gap-1 border-b border-border px-2 w-full">
              <button
                onClick={() => setShowPatFallback(false)}
                className={`inline-flex items-center justify-center px-3 py-1.5 text-[10px] font-medium transition-colors border-b-2 -mb-px flex-1 ${
                  !showPatFallback
                    ? 'text-foreground border-brand'
                    : 'text-foreground-muted border-transparent hover:text-foreground'
                }`}
              >
                {t('panel.gcmUseAccount')}
              </button>
              <button
                onClick={() => setShowPatFallback(true)}
                className={`inline-flex items-center justify-center px-3 py-1.5 text-[10px] font-medium transition-colors border-b-2 -mb-px flex-1 ${
                  showPatFallback
                    ? 'text-foreground border-brand'
                    : 'text-foreground-muted border-transparent hover:text-foreground'
                }`}
              >
                {t('panel.gcmUseDifferent')}
              </button>
            </div>

            {/* Tab content */}
            {!showPatFallback ? (
              <div className="space-y-2">
                <p className="text-[10px] text-foreground-muted/40 text-center leading-relaxed">
                  {t('panel.gcmDetectedHint')}
                </p>
                <button
                  onClick={handleAcceptGcm}
                  disabled={gcmLoading}
                  className="w-full h-8 text-xs font-medium rounded-md bg-brand text-white hover:bg-brand/90 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {gcmLoading ? <Loader2 className="w-3 h-3 animate-spin mx-auto" /> : t('panel.connect')}
                </button>
              </div>
            ) : (
              <div className="space-y-2">
                <p className="text-[10px] text-foreground-muted/60 text-center">
                  <Trans
                    i18nKey="modal.patDescription"
                    ns="github"
                    components={{
                      repo: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[10px]" />,
                      readorg: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[10px]" />,
                    }}
                  />
                </p>
                <div className="flex gap-1.5">
                  <div className="relative flex-1">
                    <input
                      type={patVisible ? 'text' : 'password'}
                      placeholder={t('panel.patPlaceholder')}
                      value={patInput}
                      onChange={(e) => { setPatInput(e.target.value); setPatError(null) }}
                      onKeyDown={(e) => e.key === 'Enter' && handleStorePat()}
                      className="w-full h-8 text-xs px-2 pr-7 rounded-md border border-border bg-background-secondary text-foreground placeholder:text-foreground-subtle outline-none focus:border-brand"
                    />
                    <button
                      type="button"
                      onClick={() => setPatVisible(!patVisible)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-foreground-subtle hover:text-foreground-muted"
                    >
                      {patVisible ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                    </button>
                  </div>
                  <button
                    onClick={handleStorePat}
                    disabled={!patInput.trim() || patLoading}
                    className="h-8 px-3 text-xs font-medium rounded-md bg-brand text-white hover:bg-brand/90 disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
                  >
                    {patLoading ? (
                      <Loader2 className="w-3 h-3 animate-spin" />
                    ) : (
                      t('panel.connect')
                    )}
                  </button>
                </div>
                {patError && (
                  <div className="flex items-start gap-1.5 text-[10px] text-semantic-error">
                    <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
                    {patError}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )
    }

    // No GCM — plain PAT input
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 h-full">
        <Github className="w-10 h-10 text-foreground-muted/20" />
        <span className="text-xs font-medium text-foreground-muted">{t('panel.notConnected')}</span>
        <span className="text-[10px] text-foreground-muted/60 text-center leading-relaxed">
          {t('panel.notConnectedHint')}
        </span>
        <div className="w-full max-w-[280px] space-y-2 mt-1">
          <p className="text-[10px] text-foreground-muted/60 text-center">
            <Trans
              i18nKey="modal.patDescription"
              ns="github"
              components={{
                repo: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[10px]" />,
                readorg: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[10px]" />,
              }}
            />
          </p>
          <div className="flex gap-1.5">
            <div className="relative flex-1">
              <input
                type={patVisible ? 'text' : 'password'}
                placeholder={t('panel.patPlaceholder')}
                value={patInput}
                onChange={(e) => { setPatInput(e.target.value); setPatError(null) }}
                onKeyDown={(e) => e.key === 'Enter' && handleStorePat()}
                className="w-full h-8 text-xs px-2 pr-7 rounded-md border border-border bg-background-secondary text-foreground placeholder:text-foreground-subtle outline-none focus:border-brand"
              />
              <button
                type="button"
                onClick={() => setPatVisible(!patVisible)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-foreground-subtle hover:text-foreground-muted"
              >
                {patVisible ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
              </button>
            </div>
            <button
              onClick={handleStorePat}
              disabled={!patInput.trim() || patLoading}
              className="h-8 px-3 text-xs font-medium rounded-md bg-brand text-white hover:bg-brand/90 disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
            >
              {patLoading ? (
                <Loader2 className="w-3 h-3 animate-spin" />
              ) : (
                t('panel.connect')
              )}
            </button>
          </div>
          {patError && (
            <div className="flex items-start gap-1.5 text-[10px] text-semantic-error">
              <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
              {patError}
            </div>
          )}
        </div>
      </div>
    )
  }

  if (!repo?.detected) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 h-full">
        <Github className="w-10 h-10 text-foreground-muted/20" />
        <span className="text-xs font-medium text-foreground-muted">{t('panel.noRepoLinked')}</span>
        <span className="text-[10px] text-foreground-muted/60 text-center leading-relaxed">
          {t('panel.noRepoLinkedHint')}
        </span>
      </div>
    )
  }

  // --- Ready ---
  return (
    <div className="flex flex-col h-full">
      {/* Repo info bar */}
      <div className="flex items-center gap-2 px-3 h-7 border-b border-border shrink-0">
        <Github className="w-3 h-3 text-foreground-muted/50" />
        <span className="text-[10px] text-foreground-muted truncate">
          {repo.owner}/{repo.repo}
        </span>
      </div>

      {/* Tabs */}
      <GitHubTabs
        active={activeTab}
        onChange={setActiveTab}
        pullCount={pulls.length || null}
        issueCount={issues.length || null}
      />

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'pulls' ? (
          <PullsTab pulls={pulls} loading={pullsLoading} error={pullsError} projectPath={projectPath} />
        ) : (
          <IssuesTab issues={issues} loading={issuesLoading} error={issuesError} />
        )}
      </div>
    </div>
  )
}
