// =============================================================================
// CloneFromGitHubModal - Multi-step modal for cloning a GitHub repository
// =============================================================================
// Steps:
// 1. Auth — Verify GitHub auth, offer PAT / Device Flow if not connected
// 2. Select — Browse user repos or enter manual URL
// 3. Confirm — Review selection, choose destination
// 4. Cloning — Progress bar with live percent + phase
// 5. Done — Auto-invokes onComplete

import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation, Trans } from 'react-i18next'
import { listen } from '@tauri-apps/api/event'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { documentDir, downloadDir, homeDir, join } from '@tauri-apps/api/path'
import {
  Github, Loader2, Search, Lock, Globe, Star,
  ChevronRight, FolderOpen, AlertCircle, Check, Eye, EyeOff, LogOut, User,
} from 'lucide-react'
import { Modal } from '../ui/modal'
import { Button } from '../ui/button'
import { Input } from '../ui/input'
import { Progress } from '../ui/progress'
import { ScrollArea } from '../ui/scroll-area'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '../ui/tabs'
import {
  tauriApi,
  GitHubAuthStatusResponse,
  GitHubUserRepoDto,
  GitHubCloneProgressPayload,
  GitHubCloneDonePayload,
  GitHubCloneErrorPayload,
} from '../../lib/tauri'
import { useGithubAuthStore } from '@/stores/githubAuthStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface CloneFromGitHubModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Called after the clone finishes. `hasVenore` is true when the cloned
   *  tree already has `.venore/project.json` and the workspace can open
   *  directly; false routes the launcher into the onboarding wizard. */
  onComplete: (path: string, name: string, owner: string, repo: string, hasVenore: boolean) => void
}

type Step = 'auth' | 'select' | 'confirm' | 'cloning'

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function CloneFromGitHubModal({ open, onOpenChange, onComplete }: CloneFromGitHubModalProps) {
  const { t } = useTranslation('github')

  // Step state
  const [step, setStep] = useState<Step>('auth')

  // Auth state
  const [authStatus, setAuthStatus] = useState<GitHubAuthStatusResponse | null>(null)
  const [authLoading, setAuthLoading] = useState(false)
  const [patInput, setPatInput] = useState('')
  const [patVisible, setPatVisible] = useState(false)
  const [patError, setPatError] = useState<string | null>(null)
  const [showPatInModal, setShowPatInModal] = useState(false)

  // Repo list state
  const [repos, setRepos] = useState<GitHubUserRepoDto[]>([])
  const [reposLoading, setReposLoading] = useState(false)
  const [reposPage, setReposPage] = useState(1)
  const [hasMore, setHasMore] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [manualInput, setManualInput] = useState('')

  // Selection state
  const [selectedRepo, setSelectedRepo] = useState<GitHubUserRepoDto | null>(null)
  const [manualOwner, setManualOwner] = useState('')
  const [manualRepoName, setManualRepoName] = useState('')
  const [manualCloneUrl, setManualCloneUrl] = useState('')

  // Confirm state
  const [destDir, setDestDir] = useState('')

  // Clone state
  const [cloneId, setCloneId] = useState<string | null>(null)
  const [clonePercent, setClonePercent] = useState<number | null>(null)
  const [clonePhase, setClonePhase] = useState('')
  const [cloneError, setCloneError] = useState<string | null>(null)

  // Set when the chosen destination folder already exists, so the confirm
  // step offers "open existing" vs "clone a fresh copy" instead of failing.
  const [existingDest, setExistingDest] = useState<{
    path: string
    isVenore: boolean
    suggestedName: string
  } | null>(null)

  const cloneIdRef = useRef<string | null>(null)

  // Reset on open
  useEffect(() => {
    if (open) {
      setStep('auth')
      setAuthStatus(null)
      setPatInput('')
      setPatVisible(false)
      setPatError(null)
      setShowPatInModal(false)
      setRepos([])
      setReposPage(1)
      setHasMore(false)
      setSearchQuery('')
      setManualInput('')
      setSelectedRepo(null)
      setManualOwner('')
      setManualRepoName('')
      setManualCloneUrl('')
      setDestDir('')
      setCloneId(null)
      setClonePercent(null)
      setClonePhase('')
      setCloneError(null)
      setExistingDest(null)
      cloneIdRef.current = null
      checkAuth()
    }
  }, [open])

  // Set default destination dir on open. Try Documents → Downloads → home
  // before giving up. The previous fallback silently set destDir to '' which
  // let the user click Clone with an unresolved destination, producing an
  // `os error 123` deep in git instead of a clear validation error.
  useEffect(() => {
    if (!open || destDir) return
    let cancelled = false
    ;(async () => {
      const candidates: Array<() => Promise<string>> = [documentDir, downloadDir, homeDir]
      for (const getDir of candidates) {
        try {
          const base = await getDir()
          if (cancelled) return
          const venoreDir = await join(base, 'Venore')
          setDestDir(venoreDir)
          return
        } catch {
          // try next candidate
        }
      }
      // All candidates failed — leave empty so the Clone button stays
      // disabled and the user has to pick manually. Better than crashing.
    })()
    return () => {
      cancelled = true
    }
  }, [open, destDir])

  // Listen for clone events
  useEffect(() => {
    if (!open) return

    const unlisteners: Promise<() => void>[] = []

    unlisteners.push(
      listen<GitHubCloneProgressPayload>('github:clone:progress', (event) => {
        if (event.payload.clone_id === cloneIdRef.current) {
          setClonePercent(event.payload.percent)
          setClonePhase(event.payload.phase)
        }
      })
    )

    unlisteners.push(
      listen<GitHubCloneDonePayload>('github:clone:done', (event) => {
        if (event.payload.clone_id === cloneIdRef.current) {
          onComplete(
            event.payload.path,
            event.payload.repo,
            event.payload.owner,
            event.payload.repo,
            event.payload.has_venore,
          )
          onOpenChange(false)
        }
      })
    )

    unlisteners.push(
      listen<GitHubCloneErrorPayload>('github:clone:error', (event) => {
        if (event.payload.clone_id === cloneIdRef.current) {
          setCloneError(event.payload.message)
        }
      })
    )

    return () => {
      unlisteners.forEach(p => p.then(fn => fn()))
    }
  }, [open, onComplete, onOpenChange])

  // -------------------------------------------------------------------------
  // Auth
  // -------------------------------------------------------------------------

  const checkAuth = async () => {
    // Fast path: boot already confirmed a valid keyring session — skip the
    // auth round-trip (and any GCM prompt) and go straight to repo selection.
    const gh = useGithubAuthStore.getState()
    if (gh.loaded && gh.authenticated) {
      setAuthStatus({
        authenticated: true,
        login: gh.login,
        name: gh.name,
        avatar_url: gh.avatarUrl,
        gcm_detected: false, gcm_login: null, gcm_name: null, gcm_avatar_url: null,
      })
      setStep('select')
      loadRepos(1)
      return
    }

    setAuthLoading(true)
    try {
      const status = await tauriApi.githubAuthStatus()
      setAuthStatus(status)
      if (status.authenticated) {
        setStep('select')
        loadRepos(1)
      }
    } catch (err) {
      console.error('Auth check failed:', err)
    } finally {
      setAuthLoading(false)
    }
  }

  const handleStorePat = async () => {
    if (!patInput.trim()) return
    setPatError(null)
    setAuthLoading(true)
    try {
      const status = await tauriApi.githubStorePat({ token: patInput.trim() })
      setAuthStatus(status)
      useGithubAuthStore.getState().applyStatus(status)
      setPatInput('')
      setPatVisible(false)
      if (status.authenticated) {
        setStep('select')
        loadRepos(1)
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Invalid token'
      setPatError(message)
    } finally {
      setAuthLoading(false)
    }
  }

  const handleDisconnect = async () => {
    setAuthLoading(true)
    try {
      await tauriApi.githubDisconnect()
      useGithubAuthStore.getState().reset()
      const status = await tauriApi.githubAuthStatus()
      setAuthStatus(status)
      setShowPatInModal(false)
      setRepos([])
      setStep('auth')
    } catch (err) {
      console.error('Failed to disconnect:', err)
    } finally {
      setAuthLoading(false)
    }
  }

  // -------------------------------------------------------------------------
  // Repo loading
  // -------------------------------------------------------------------------

  const loadRepos = async (page: number) => {
    setReposLoading(true)
    try {
      const result = await tauriApi.githubListUserRepos({ page, per_page: 30 })
      if (page === 1) {
        setRepos(result.repos)
      } else {
        setRepos(prev => [...prev, ...result.repos])
      }
      setHasMore(result.has_more)
      setReposPage(page)
    } catch (err) {
      console.error('Failed to load repos:', err)
    } finally {
      setReposLoading(false)
    }
  }

  const handleLoadMore = () => {
    loadRepos(reposPage + 1)
  }

  // -------------------------------------------------------------------------
  // Selection
  // -------------------------------------------------------------------------

  const handleSelectRepo = (repo: GitHubUserRepoDto) => {
    setSelectedRepo(repo)
    setManualOwner('')
    setManualRepoName('')
    setManualCloneUrl('')
    setStep('confirm')
  }

  const handleManualSelect = () => {
    const input = manualInput.trim()
    if (!input) return

    // Parse "owner/repo" or "https://github.com/owner/repo"
    let owner = ''
    let repo = ''
    let cloneUrl = ''

    const urlMatch = input.match(/github\.com\/([^/]+)\/([^/.\s]+)/)
    const slashMatch = input.match(/^([^/\s]+)\/([^/\s]+)$/)

    if (urlMatch) {
      owner = urlMatch[1]
      repo = urlMatch[2]
    } else if (slashMatch) {
      owner = slashMatch[1]
      repo = slashMatch[2]
    }

    if (!owner || !repo) return

    cloneUrl = `https://github.com/${owner}/${repo}.git`
    setSelectedRepo(null)
    setManualOwner(owner)
    setManualRepoName(repo)
    setManualCloneUrl(cloneUrl)
    setStep('confirm')
  }

  // -------------------------------------------------------------------------
  // Confirm & Clone
  // -------------------------------------------------------------------------

  const effectiveOwner = selectedRepo ? selectedRepo.owner : manualOwner
  const effectiveRepo = selectedRepo ? selectedRepo.name : manualRepoName
  const effectiveCloneUrl = selectedRepo ? selectedRepo.clone_url : manualCloneUrl

  const handleChangeDestination = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('clone.selectDestination'),
      defaultPath: destDir || undefined,
    })
    if (selected && typeof selected === 'string') {
      setDestDir(selected)
    }
  }

  // Run the actual clone into `folderName` under `destDir`.
  const startClone = async (folderName: string) => {
    setCloneError(null)
    setClonePercent(null)
    setClonePhase('')

    // Generate the clone id up front and record it BEFORE invoking, so an
    // error event the backend emits immediately is matched by the listener
    // instead of dropped — which used to leave the modal stuck on "Cloning…".
    const id = crypto.randomUUID()
    cloneIdRef.current = id
    setCloneId(id)
    setStep('cloning')

    try {
      await tauriApi.githubCloneRepo({
        clone_id: id,
        clone_url: effectiveCloneUrl,
        owner: effectiveOwner,
        repo: folderName,
        dest_dir: destDir,
      })
    } catch (err: any) {
      setCloneError(err.message || 'Clone failed')
    }
  }

  // Entry point from the Confirm button: check whether the destination already
  // exists. If it does, surface the open-vs-fresh choice instead of failing;
  // otherwise clone straight away.
  const handleClone = async () => {
    try {
      const info = await tauriApi.githubInspectCloneDestination({
        dest_dir: destDir,
        repo: effectiveRepo,
      })
      if (info.exists) {
        setExistingDest({
          path: info.path,
          isVenore: info.is_venore,
          suggestedName: info.suggested_name,
        })
        return
      }
      startClone(effectiveRepo)
    } catch {
      // Inspection failed for some reason — fall back to attempting the clone;
      // the backend still guards against an existing destination.
      startClone(effectiveRepo)
    }
  }

  // "Open existing" — hand the already-present folder to the launcher.
  const handleOpenExisting = () => {
    if (!existingDest) return
    onComplete(
      existingDest.path,
      effectiveRepo,
      effectiveOwner,
      effectiveRepo,
      existingDest.isVenore,
    )
    onOpenChange(false)
  }

  // "Clone a fresh copy" — clone into the first free numbered folder.
  const handleCloneFresh = () => {
    if (!existingDest) return
    const name = existingDest.suggestedName
    setExistingDest(null)
    startClone(name)
  }

  // -------------------------------------------------------------------------
  // Filtered repos
  // -------------------------------------------------------------------------

  const filteredRepos = searchQuery
    ? repos.filter(r =>
        r.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        r.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (r.description && r.description.toLowerCase().includes(searchQuery.toLowerCase()))
      )
    : repos

  // -------------------------------------------------------------------------
  // Helpers
  // -------------------------------------------------------------------------

  const canClose = step !== 'cloning' || !!cloneError

  const handleOpenChange = useCallback((open: boolean) => {
    if (!open && !canClose) return
    onOpenChange(open)
  }, [canClose, onOpenChange])

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <Modal
      open={open}
      onOpenChange={handleOpenChange}
      icon={<Github className="w-5 h-5 text-foreground" />}
      title={t('clone.title')}
      maxWidth="max-w-md"
      blockClose={!canClose}
    >
      {/* Step 1: Auth */}
      {step === 'auth' && (
        <div className="space-y-3">
          {authLoading && !authStatus ? (
            <div className="flex items-center gap-2 text-sm text-foreground-muted">
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              {t('modal.checking')}
            </div>
          ) : authStatus?.gcm_detected ? (
            <div className="space-y-3">
              {/* Account card — always visible */}
              <div className="flex items-center gap-3 p-3 rounded-lg bg-background-tertiary">
                {authStatus.gcm_avatar_url ? (
                  <img src={authStatus.gcm_avatar_url} alt={authStatus.gcm_login ?? ''} className="w-8 h-8 rounded-full" />
                ) : (
                  <User className="w-8 h-8 text-foreground-muted/40" />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-foreground">@{authStatus.gcm_login}</span>
                  {authStatus.gcm_name && (
                    <p className="text-xs text-foreground-muted truncate">{authStatus.gcm_name}</p>
                  )}
                  <p className="text-[10px] text-foreground-muted/60">{t('panel.gcmDetected')}</p>
                </div>
              </div>

              {/* Tab-style toggle — same pattern as TabsTrigger */}
              <div className="inline-flex items-center gap-1 border-b border-border px-2 w-full">
                <button
                  onClick={() => setShowPatInModal(false)}
                  className={`inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium transition-colors border-b-2 -mb-px flex-1 ${
                    !showPatInModal
                      ? 'text-foreground border-brand'
                      : 'text-foreground-muted border-transparent hover:text-foreground'
                  }`}
                >
                  {t('panel.gcmUseAccount')}
                </button>
                <button
                  onClick={() => setShowPatInModal(true)}
                  className={`inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium transition-colors border-b-2 -mb-px flex-1 ${
                    showPatInModal
                      ? 'text-foreground border-brand'
                      : 'text-foreground-muted border-transparent hover:text-foreground'
                  }`}
                >
                  {t('panel.gcmUseDifferent')}
                </button>
              </div>

              {/* Content based on selected tab */}
              {!showPatInModal ? (
                <div className="space-y-2">
                  <p className="text-xs text-foreground-muted/60">{t('panel.gcmDetectedHint')}</p>
                  <Button
                    size="sm"
                    onClick={async () => {
                      setAuthLoading(true)
                      try {
                        const status = await tauriApi.githubAcceptGcm()
                        setAuthStatus(status)
                        useGithubAuthStore.getState().applyStatus(status)
                        if (status.authenticated) {
                          setStep('select')
                          loadRepos(1)
                        }
                      } catch {
                        setShowPatInModal(true)
                      } finally {
                        setAuthLoading(false)
                      }
                    }}
                    disabled={authLoading}
                    className="w-full h-8"
                  >
                    {authLoading ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : t('modal.connectButton')}
                  </Button>
                </div>
              ) : (
                <div className="space-y-2">
                  <p className="text-xs text-foreground-muted">
                    <Trans
                      i18nKey="modal.patDescription"
                      ns="github"
                      components={{
                        repo: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[11px]" />,
                        readorg: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[11px]" />,
                      }}
                    />
                  </p>
                  <div className="flex gap-2">
                    <div className="relative flex-1">
                      <Input
                        type={patVisible ? 'text' : 'password'}
                        placeholder={t('modal.patPlaceholder')}
                        value={patInput}
                        onChange={(e) => { setPatInput(e.target.value); setPatError(null) }}
                        onKeyDown={(e) => e.key === 'Enter' && handleStorePat()}
                        className="h-8 text-xs pr-8"
                      />
                      <button
                        type="button"
                        onClick={() => setPatVisible(!patVisible)}
                        className="absolute right-2 top-1/2 -translate-y-1/2 text-foreground-subtle hover:text-foreground-muted"
                      >
                        {patVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                      </button>
                    </div>
                    <Button
                      size="sm"
                      onClick={handleStorePat}
                      disabled={!patInput.trim() || authLoading}
                      className="h-8"
                    >
                      {authLoading ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        t('modal.connectButton')
                      )}
                    </Button>
                  </div>
                  {patError && (
                    <div className="flex items-start gap-1.5 text-xs text-semantic-error">
                      <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                      {patError}
                    </div>
                  )}
                </div>
              )}
            </div>
          ) : (
            <div className="space-y-3">
              <p className="text-xs text-foreground-muted">
                <Trans
                  i18nKey="modal.patDescription"
                  ns="github"
                  components={{
                    repo: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[11px]" />,
                    readorg: <code className="text-foreground bg-background-tertiary px-1 py-0.5 rounded text-[11px]" />,
                  }}
                />
              </p>
              <div className="flex gap-2">
                <div className="relative flex-1">
                  <Input
                    type={patVisible ? 'text' : 'password'}
                    placeholder={t('modal.patPlaceholder')}
                    value={patInput}
                    onChange={(e) => { setPatInput(e.target.value); setPatError(null) }}
                    onKeyDown={(e) => e.key === 'Enter' && handleStorePat()}
                    className="h-8 text-xs pr-8"
                  />
                  <button
                    type="button"
                    onClick={() => setPatVisible(!patVisible)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-foreground-subtle hover:text-foreground-muted"
                  >
                    {patVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                  </button>
                </div>
                <Button
                  size="sm"
                  onClick={handleStorePat}
                  disabled={!patInput.trim() || authLoading}
                  className="h-8"
                >
                  {authLoading ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    t('modal.connectButton')
                  )}
                </Button>
              </div>
              {patError && (
                <div className="flex items-start gap-1.5 text-xs text-semantic-error">
                  <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                  {patError}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Step 2: Select Repo */}
      {step === 'select' && (
        <div className="space-y-3">
          {/* Auth card — same design as GitHubModal */}
          {authStatus?.authenticated && (
            <div className="flex items-center gap-3 p-3 rounded-lg bg-background-tertiary">
              {authStatus.avatar_url && (
                <img
                  src={authStatus.avatar_url}
                  alt={authStatus.login ?? ''}
                  className="w-8 h-8 rounded-full"
                />
              )}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <Check className="w-3.5 h-3.5 text-brand" />
                  <span className="text-sm font-medium text-foreground">
                    {authStatus.login}
                  </span>
                </div>
                {authStatus.name && (
                  <p className="text-xs text-foreground-muted truncate">{authStatus.name}</p>
                )}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleDisconnect}
                disabled={authLoading}
              >
                <LogOut className="w-3.5 h-3.5 mr-1.5" />
                {t('modal.disconnect')}
              </Button>
            </div>
          )}

          {/* Tabs: My Repos | URL */}
          <Tabs defaultValue="repos">
            <TabsList className="w-full">
              <TabsTrigger value="repos">{t('clone.tabMyRepos')}</TabsTrigger>
              <TabsTrigger value="url">{t('clone.tabUrl')}</TabsTrigger>
            </TabsList>

            {/* Tab: My Repos */}
            <TabsContent value="repos" className="pt-3 space-y-3">
              {/* Search */}
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-foreground-muted" />
                <Input
                  placeholder={t('clone.searchPlaceholder')}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pl-9"
                />
              </div>

              {/* Repo List */}
              <ScrollArea className="h-[240px]">
                <div className="space-y-1">
                  {reposLoading && repos.length === 0 ? (
                    <div className="flex items-center justify-center py-8">
                      <Loader2 className="w-5 h-5 animate-spin text-foreground-muted" />
                    </div>
                  ) : filteredRepos.length === 0 ? (
                    <p className="text-sm text-foreground-muted text-center py-4">
                      {t('clone.noReposFound')}
                    </p>
                  ) : (
                    filteredRepos.map((repo) => (
                      <button
                        key={repo.id}
                        onClick={() => handleSelectRepo(repo)}
                        className="w-full flex items-center gap-3 px-3 py-2 rounded-md hover:bg-background-tertiary transition-colors text-left"
                      >
                        {repo.is_private ? (
                          <Lock className="w-4 h-4 text-foreground-muted shrink-0" />
                        ) : (
                          <Globe className="w-4 h-4 text-foreground-muted shrink-0" />
                        )}
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium truncate">{repo.name}</div>
                          {repo.description && (
                            <div className="text-xs text-foreground-muted truncate">{repo.description}</div>
                          )}
                        </div>
                        <div className="flex items-center gap-2 shrink-0">
                          {repo.language && (
                            <span className="text-xs text-foreground-muted">{repo.language}</span>
                          )}
                          {repo.stargazers_count > 0 && (
                            <span className="flex items-center gap-0.5 text-xs text-foreground-muted">
                              <Star className="w-3 h-3" />
                              {repo.stargazers_count}
                            </span>
                          )}
                          <ChevronRight className="w-4 h-4 text-foreground-muted" />
                        </div>
                      </button>
                    ))
                  )}

                  {/* Load more */}
                  {hasMore && !searchQuery && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={handleLoadMore}
                      disabled={reposLoading}
                      className="w-full"
                    >
                      {reposLoading ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        t('clone.loadMore')
                      )}
                    </Button>
                  )}
                </div>
              </ScrollArea>
            </TabsContent>

            {/* Tab: URL */}
            <TabsContent value="url" className="pt-3 space-y-3">
              <p className="text-xs text-foreground-muted">{t('clone.manualEntry')}</p>
              <div className="flex gap-2">
                <Input
                  placeholder={t('clone.manualPlaceholder')}
                  value={manualInput}
                  onChange={(e) => setManualInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleManualSelect()}
                  className="flex-1"
                />
                <Button onClick={handleManualSelect} disabled={!manualInput.trim()} size="sm">
                  {t('clone.next')}
                </Button>
              </div>
            </TabsContent>
          </Tabs>
        </div>
      )}

      {/* Step 3: Confirm */}
      {step === 'confirm' && (
        <div className="space-y-4">
          <h3 className="text-sm font-medium">{t('clone.confirmTitle')}</h3>

          {/* Repo info */}
          <div className="flex items-center gap-3 p-3 rounded-md bg-background-tertiary">
            <Github className="w-5 h-5 text-foreground-muted shrink-0" />
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium">{effectiveOwner}/{effectiveRepo}</div>
              {selectedRepo?.description && (
                <div className="text-xs text-foreground-muted truncate">{selectedRepo.description}</div>
              )}
            </div>
            {selectedRepo && (
              <span className="text-xs text-foreground-muted">
                {selectedRepo.is_private ? t('clone.private') : t('clone.public')}
              </span>
            )}
          </div>

          {/* Destination */}
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('clone.destination')}</label>
            <div className="flex items-center gap-2">
              <div className="flex-1 text-sm text-foreground-muted truncate bg-background-tertiary px-3 py-2 rounded-md">
                {destDir ? `${destDir}/${effectiveRepo}` : t('clone.destinationMissing', { defaultValue: 'No destination selected' })}
              </div>
              <Button variant="outline" size="sm" onClick={handleChangeDestination}>
                <FolderOpen className="w-4 h-4 mr-1" />
                {t('clone.changeDestination')}
              </Button>
            </div>
            {!destDir && (
              <p className="text-xs text-destructive">
                {t('clone.destinationRequired', { defaultValue: 'Pick a folder to clone into.' })}
              </p>
            )}
          </div>

          {/* Destination already exists → offer open vs fresh copy */}
          {existingDest ? (
            <div className="space-y-3">
              <div className="flex items-start gap-2 text-xs text-amber-400">
                <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                <span>
                  {t('clone.destExists', {
                    defaultValue: 'This folder already exists. Open it, or clone a fresh copy.',
                  })}
                </span>
              </div>
              <div className="flex flex-col gap-2">
                <Button variant="outline" onClick={handleOpenExisting} className="w-full justify-start">
                  <FolderOpen className="w-4 h-4 mr-2" />
                  {t('clone.openExisting', { defaultValue: 'Open the existing folder' })}
                </Button>
                <Button onClick={handleCloneFresh} className="w-full justify-start">
                  <Github className="w-4 h-4 mr-2" />
                  {t('clone.cloneFresh', {
                    defaultValue: 'Clone a fresh copy ({{name}})',
                    name: existingDest.suggestedName,
                  })}
                </Button>
              </div>
              <Button variant="ghost" size="sm" onClick={() => setExistingDest(null)} className="w-full">
                {t('clone.back')}
              </Button>
            </div>
          ) : (
            <div className="flex justify-between">
              <Button variant="ghost" onClick={() => setStep('select')}>
                {t('clone.back')}
              </Button>
              <Button onClick={handleClone} disabled={!destDir || !effectiveRepo}>
                {t('clone.clone')}
              </Button>
            </div>
          )}
        </div>
      )}

      {/* Step 4: Cloning */}
      {step === 'cloning' && (
        <div className="space-y-4 py-4">
          {cloneError ? (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-sm text-destructive">
                <AlertCircle className="w-4 h-4" />
                {t('clone.cloneFailed', { message: cloneError })}
              </div>
              <div className="flex justify-between">
                <Button variant="ghost" onClick={() => { setStep('confirm'); setCloneError(null) }}>
                  {t('clone.back')}
                </Button>
                <Button variant="outline" onClick={() => onOpenChange(false)}>
                  {t('clone.cancel')}
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <Loader2 className="w-4 h-4 animate-spin text-brand" />
                <span className="text-sm font-medium">{t('clone.cloning')}</span>
              </div>
              <Progress value={clonePercent ?? 0} className="h-2" />
              <p className="text-xs text-foreground-muted">
                {clonePercent != null
                  ? t('clone.cloningPhase', { phase: clonePhase, percent: clonePercent })
                  : t('clone.cloningIndeterminate', { phase: clonePhase || 'Initializing' })
                }
              </p>
            </div>
          )}
        </div>
      )}
    </Modal>
  )
}
