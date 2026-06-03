// =============================================================================
// GitHub Auth Store - Cached GitHub connection state
// =============================================================================
// Validated once at boot via githubValidateSession() (keyring token only, no
// prompts). GitHubPanel and the clone modal read the connected state from here
// instead of re-validating (a network round-trip, and previously a GCM picker)
// on every mount. Auth actions (PAT connect, disconnect) push fresh state in
// via applyStatus / reset.

import { create } from 'zustand'
import { tauriApi, type GitHubAuthStatusResponse } from '@/lib/tauri'

interface GithubAuthState {
  /// True once a validation attempt has finished (success or failure), so
  /// consumers know the cached value is meaningful rather than the initial blank.
  loaded: boolean
  authenticated: boolean
  login: string | null
  name: string | null
  avatarUrl: string | null

  /// Validate the stored keyring token once and cache the result. Safe to call
  /// repeatedly — only the first run hits the backend.
  validate: () => Promise<void>
  /// Overwrite the cache from a fresh auth response (after PAT connect / accept).
  applyStatus: (status: GitHubAuthStatusResponse) => void
  /// Clear to a disconnected state (after disconnect).
  reset: () => void
}

export const useGithubAuthStore = create<GithubAuthState>((set, get) => ({
  loaded: false,
  authenticated: false,
  login: null,
  name: null,
  avatarUrl: null,

  validate: async () => {
    if (get().loaded) return
    try {
      const status = await tauriApi.githubValidateSession()
      set({
        loaded: true,
        authenticated: status.authenticated,
        login: status.login ?? null,
        name: status.name ?? null,
        avatarUrl: status.avatar_url ?? null,
      })
    } catch (err) {
      console.error('Failed to validate GitHub session:', err)
      // Mark loaded anyway so consumers fall back to their own checks instead
      // of waiting forever.
      set({ loaded: true })
    }
  },

  applyStatus: (status) =>
    set({
      loaded: true,
      authenticated: status.authenticated,
      login: status.login ?? null,
      name: status.name ?? null,
      avatarUrl: status.avatar_url ?? null,
    }),

  reset: () =>
    set({
      loaded: true,
      authenticated: false,
      login: null,
      name: null,
      avatarUrl: null,
    }),
}))
