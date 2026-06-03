// =============================================================================
// Auth Store - Optional Venore Cloud authentication state
// =============================================================================
// Manages sign-in/sign-out state for the optional collaboration SaaS.
// All features work without authentication — this is purely for cloud features.

import { create } from 'zustand'
import { tauriApi } from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface CloudUser {
  userId: string
  email: string
  displayName: string
  avatarUrl: string | null
}

interface AuthState {
  authenticated: boolean
  user: CloudUser | null
  loading: boolean
  error: string | null

  /** Read token + cached profile from backend (fast, offline) */
  checkStatus: () => Promise<void>
  /** Start browser-based OAuth flow (venore.app) */
  signIn: () => Promise<void>
  /** Start OAuth PKCE flow with a provider (github, google) */
  signInWithOAuth: (provider: string) => Promise<void>
  /** Sign in with email and password via Supabase */
  signInWithEmail: (email: string, password: string) => Promise<boolean>
  /** Sign up with email, password, and display name. Returns 'needs_confirmation' | 'signed_in' | null on error */
  signUpWithEmail: (email: string, password: string, displayName: string) => Promise<'needs_confirmation' | 'signed_in' | null>
  /** Clear tokens and cached profile */
  signOut: () => Promise<void>
  /** Clear local state (called when signed-out event received) */
  clearState: () => void
  /** Clear error */
  clearError: () => void
}

// -----------------------------------------------------------------------------
// Store
// -----------------------------------------------------------------------------

export const useAuthStore = create<AuthState>((set, get) => ({
  authenticated: false,
  user: null,
  loading: false,
  error: null,

  checkStatus: async () => {
    try {
      const status = await tauriApi.cloudAuthStatus()
      if (status.authenticated && status.user_id) {
        set({
          authenticated: true,
          user: {
            userId: status.user_id,
            email: status.email ?? '',
            displayName: status.display_name ?? '',
            avatarUrl: status.avatar_url,
          },
        })
      } else {
        set({ authenticated: false, user: null })
      }
    } catch (err) {
      console.error('Failed to check cloud auth status:', err)
      set({ authenticated: false, user: null })
    }
  },

  signIn: async () => {
    if (get().loading) return
    set({ loading: true, error: null })
    try {
      await tauriApi.cloudStartSignIn()
      // The actual auth completion is handled by the cloud:auth:success event
      // which calls checkStatus() to refresh state
    } catch (err) {
      console.error('Failed to start cloud sign-in:', err)
    } finally {
      set({ loading: false })
    }
  },

  signInWithOAuth: async (provider: string) => {
    if (get().loading) return
    set({ loading: true, error: null })
    try {
      await tauriApi.cloudStartOAuth(provider)
      // Completion handled by cloud:auth:success event → checkStatus()
    } catch (err) {
      console.error('Failed to start OAuth flow:', err)
    } finally {
      set({ loading: false })
    }
  },

  signInWithEmail: async (email: string, password: string) => {
    if (get().loading) return false
    set({ loading: true, error: null })
    try {
      const status = await tauriApi.cloudSignInWithEmail({ email, password })
      if (status.authenticated && status.user_id) {
        set({
          authenticated: true,
          user: {
            userId: status.user_id,
            email: status.email ?? '',
            displayName: status.display_name ?? '',
            avatarUrl: status.avatar_url,
          },
        })
        return true
      }
      set({ error: 'Authentication failed' })
      return false
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Authentication failed'
      set({ error: message })
      return false
    } finally {
      set({ loading: false })
    }
  },

  signUpWithEmail: async (email: string, password: string, displayName: string) => {
    if (get().loading) return null
    set({ loading: true, error: null })
    try {
      const res = await tauriApi.cloudSignUpWithEmail({
        email,
        password,
        display_name: displayName,
      })
      if (res.needs_confirmation) {
        return 'needs_confirmation'
      }
      // Auto-confirmed → refresh status to get user data
      await get().checkStatus()
      return 'signed_in'
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Sign up failed'
      set({ error: message })
      return null
    } finally {
      set({ loading: false })
    }
  },

  signOut: async () => {
    if (get().loading) return
    set({ loading: true })
    try {
      await tauriApi.cloudSignOut()
      set({ authenticated: false, user: null })
    } catch (err) {
      console.error('Failed to sign out from cloud:', err)
    } finally {
      set({ loading: false })
    }
  },

  clearState: () => {
    set({ authenticated: false, user: null, loading: false, error: null })
  },

  clearError: () => {
    set({ error: null })
  },
}))
