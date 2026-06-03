/**
 * Reset all wizard stores to initial state
 *
 * Call this when opening the wizard for a NEW project to clear previous data.
 * For checkpoint resume, don't call this - let stores load from localStorage.
 */

import { useWizardStore } from '@/stores/wizardStore'
import { useWizardDataStore } from '@/stores/wizardDataStore'
import { useWizardCacheStore } from '@/stores/wizardCacheStore'

export function resetAllWizardStores() {
  console.log('🔄 [resetAllStores] Resetting all wizard stores for new project')

  useWizardStore.getState().resetWizard() // Navigation + execution flags
  useWizardDataStore.getState().reset()    // All step data
  useWizardCacheStore.getState().reset()   // Transient cache

  console.log('✅ [resetAllStores] All stores reset to initial state')
}
