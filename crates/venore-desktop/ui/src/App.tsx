// =============================================================================
// App - Main application component
// =============================================================================
// Manages the main app flow:
// 1. BootScreen - Initial loading
// 2. LauncherScreen - Project selection
// 3. WorkspaceScreen - Active project workspace

import { useState, useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Toaster } from 'sonner'
import { BootScreen, LauncherScreen, OceanCatalogScreen, WorkspaceScreen } from './screens'
import { DevChatPreview } from './screens/DevChatPreview'
import { useAuthStore } from './stores/authStore'
import { useAppNotifications } from './hooks/useAppNotifications'
import { useAiConnectionsBootstrap } from './hooks/useAiConnectionsBootstrap'
import { useAppPhaseStore } from './stores/appPhaseStore'

function App() {
  useAppNotifications()
  useAiConnectionsBootstrap()

  // Phase + active project live in the store so deep components (title-bar
  // menu, command palette, etc.) can navigate without prop drilling.
  const phase = useAppPhaseStore((s) => s.phase)
  const currentProjectPath = useAppPhaseStore((s) => s.currentProjectPath)
  const currentProjectId = useAppPhaseStore((s) => s.currentProjectId)
  const currentProjectType = useAppPhaseStore((s) => s.currentProjectType)
  const setPhase = useAppPhaseStore((s) => s.setPhase)
  const openProject = useAppPhaseStore((s) => s.openProject)

  const [showDevPreview, setShowDevPreview] = useState(() => window.location.hash === '#dev-chat')

  // Ctrl+Shift+D toggles dev chat preview. Ctrl+Shift+O toggles the Ocean catalog.
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'D') {
        e.preventDefault()
        setShowDevPreview((v) => !v)
      }
      if (e.ctrlKey && e.shiftKey && (e.key === 'O' || e.key === 'o')) {
        e.preventDefault()
        const current = useAppPhaseStore.getState().phase
        setPhase(current === 'ocean-catalog' ? 'launcher' : 'ocean-catalog')
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [setPhase])

  // Hide splash screen once React is ready
  useEffect(() => {
    const splash = document.getElementById('splash-screen')
    if (splash) {
      splash.style.display = 'none'
    }
  }, [])

  // Listen for cloud auth events
  useEffect(() => {
    const unlisteners: (() => void)[] = []

    listen('cloud:auth:success', () => {
      console.log('[App] Cloud auth success — refreshing status')
      useAuthStore.getState().checkStatus()
    }).then((fn) => unlisteners.push(fn))

    listen('cloud:auth:signed-out', () => {
      console.log('[App] Cloud auth signed out')
      useAuthStore.getState().clearState()
    }).then((fn) => unlisteners.push(fn))

    listen('cloud:auth:error', (event) => {
      console.warn('[App] Cloud auth error:', event.payload)
    }).then((fn) => unlisteners.push(fn))

    return () => {
      unlisteners.forEach((fn) => fn())
    }
  }, [])

  const handleBootReady = () => {
    console.log('[App] Boot complete, showing launcher')
    // Non-blocking: check cloud auth status from keyring (fast, offline)
    useAuthStore.getState().checkStatus()
    setPhase('launcher')
  }

  const handleBootError = (error: string) => {
    console.error('[App] Boot failed:', error)
    // BootScreen stays visible with error state + retry button
  }

  const handleProjectOpen = (projectPath: string, projectType?: string, projectId?: string) => {
    void openProject(projectPath, projectType, projectId)
  }

  if (showDevPreview) {
    return <DevChatPreview />
  }

  return (
    <>
    <Toaster
      position="bottom-right"
      richColors
      closeButton
      toastOptions={{ className: 'font-sans', duration: 8000 }}
    />
    <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
      <div className="flex-1 overflow-hidden">
        {phase === 'boot' && (
          <BootScreen
            onReady={handleBootReady}
            onError={handleBootError}
          />
        )}

        {phase === 'launcher' && (
          <LauncherScreen
            onProjectOpen={handleProjectOpen}
            onOpenOceanCatalog={() => setPhase('ocean-catalog')}
          />
        )}

        {phase === 'ocean-catalog' && (
          <OceanCatalogScreen onBack={() => setPhase('launcher')} />
        )}

        {phase === 'workspace' && currentProjectPath && (
          <WorkspaceScreen
            projectPath={currentProjectPath}
            projectId={currentProjectId ?? undefined}
            projectType={currentProjectType as 'code' | 'knowledge'}
          />
        )}
      </div>
    </div>
    </>
  )
}

export default App
