// =============================================================================
// Wizard Store - Main navigation and state management
// =============================================================================

import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type {
  WizardState,
  WizardCheckpoint,
  CheckpointInfo,
} from '../lib/wizard/types'

interface WizardStoreState extends WizardState {
  // Additional internal state
  _hasUnsavedChanges: boolean

  // Execution flags (prevent duplicate operations from React Strict Mode)
  hasResetStores: boolean
  isRootContextGenerating: boolean
  isGenerationStarted: boolean
  isIslandDetectionRunning: boolean

  // Flag actions
  setHasResetStores: (value: boolean) => void
  setIsRootContextGenerating: (value: boolean) => void
  setIsGenerationStarted: (value: boolean) => void
  setIsIslandDetectionRunning: (value: boolean) => void
  resetExecutionFlags: () => void
}

export const useWizardStore = create<WizardStoreState>()(
  persist(
    (set, get) => ({
      // Initial state
      currentStep: 0,
      isOpen: false,
      hasCheckpoint: false,
      checkpointInfo: null,
      _hasUnsavedChanges: false,

      // Execution flags
      hasResetStores: false,
      isRootContextGenerating: false,
      isGenerationStarted: false,
      isIslandDetectionRunning: false,

      // Navigation actions
      setStep: (step: number) => {
        set({ currentStep: step, _hasUnsavedChanges: true })
      },

      nextStep: () => {
        const { currentStep } = get()
        if (currentStep < 5) {
          set({ currentStep: currentStep + 1, _hasUnsavedChanges: true })
        }
      },

      prevStep: () => {
        const { currentStep } = get()
        if (currentStep > 0) {
          set({ currentStep: currentStep - 1 })
        }
      },

      // Modal control
      openWizard: () => {
        set({ isOpen: true })
      },

      closeWizard: () => {
        set({ isOpen: false })
      },

      // Reset
      resetWizard: () => {
        set({
          currentStep: 0,
          isOpen: false,
          hasCheckpoint: false,
          checkpointInfo: null,
          _hasUnsavedChanges: false,
          // Reset execution flags
          hasResetStores: false,
          isRootContextGenerating: false,
          isGenerationStarted: false,
          isIslandDetectionRunning: false,
        })
      },

      // Checkpoint
      loadCheckpoint: (checkpoint: WizardCheckpoint) => {
        set({
          currentStep: checkpoint.currentStep,
          hasCheckpoint: false,
          checkpointInfo: null,
          _hasUnsavedChanges: false,
        })
      },

      // Execution flag actions
      setHasResetStores: (value: boolean) => {
        set({ hasResetStores: value })
      },

      setIsRootContextGenerating: (value: boolean) => {
        set({ isRootContextGenerating: value })
      },

      setIsGenerationStarted: (value: boolean) => {
        set({ isGenerationStarted: value })
      },

      setIsIslandDetectionRunning: (value: boolean) => {
        set({ isIslandDetectionRunning: value })
      },

      resetExecutionFlags: () => {
        set({
          hasResetStores: false,
          isRootContextGenerating: false,
          isGenerationStarted: false,
          isIslandDetectionRunning: false,
        })
      },
    }),
    {
      name: 'venore-wizard-state',
      partialize: (state) => ({
        currentStep: state.currentStep,
        hasCheckpoint: state.hasCheckpoint,
        checkpointInfo: state.checkpointInfo,
      }),
    }
  )
)
