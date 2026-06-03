// =============================================================================
// useWizardNavigation - Navigation and validation for wizard steps (5-step flow)
// =============================================================================

import { useWizardStore } from '../stores/wizardStore'
import { useWizardDataStore } from '../stores/wizardDataStore'

// -----------------------------------------------------------------------------
// Validation Functions
// -----------------------------------------------------------------------------

function validateStep0(): boolean {
  // Step 0: Context - Description validation
  const description = useWizardDataStore.getState().step1.description
  return description.trim().length >= 20
}

function validateStep1(): boolean {
  // Step 1: Rules - Depth level selected (always true since there's a default)
  return true
}

function validateStep2(): boolean {
  // Step 2: Analysis & Index - Must have completed indexing
  const indexResult = useWizardDataStore.getState().indexResult
  return indexResult !== null
}

function validateStep3(): boolean {
  // Step 3: Index Results - Review only, always valid
  return true
}

function validateStep4(): boolean {
  // Step 4: Complete - Final step, always valid
  return true
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useWizardNavigation() {
  const currentStep = useWizardStore((state) => state.currentStep)
  const setStep = useWizardStore((state) => state.setStep)
  const nextStep = useWizardStore((state) => state.nextStep)
  const prevStep = useWizardStore((state) => state.prevStep)

  // Subscribe to stores for reactive validation
  const description = useWizardDataStore((state) => state.step1.description)
  const indexResult = useWizardDataStore((state) => state.indexResult)

  // Validation map
  const validators: Record<number, () => boolean> = {
    0: validateStep0,
    1: validateStep1,
    2: validateStep2,
    3: validateStep3,
    4: validateStep4,
  }

  // Check if current step is valid (this will re-run when subscribed stores change)
  const isCurrentStepValid = validators[currentStep]?.() ?? true

  // Check if we can go to next step
  const canGoNext = currentStep < 4 && isCurrentStepValid

  // Check if we can go back
  const canGoBack = currentStep > 0

  // Navigate to specific step (with validation)
  const goToStep = (step: number) => {
    if (step >= 0 && step <= 4) {
      // Validate all steps between current and target
      let valid = true
      const start = Math.min(currentStep, step)
      const end = Math.max(currentStep, step)

      for (let i = start; i < end; i++) {
        if (!validators[i]?.()) {
          valid = false
          break
        }
      }

      if (valid) {
        setStep(step)
      }
    }
  }

  // Navigate next with validation
  const goNext = () => {
    if (canGoNext) {
      nextStep()
    }
  }

  // Navigate back (no validation needed)
  const goBack = () => {
    if (canGoBack) {
      prevStep()
    }
  }

  // Get validation errors for current step
  const getValidationErrors = (): string[] => {
    const errors: string[] = []

    switch (currentStep) {
      case 0:
        if (!validateStep0()) {
          errors.push('Description must be at least 20 characters')
        }
        break
      case 2:
        if (!validateStep2()) {
          errors.push('Analysis and indexing must complete first')
        }
        break
    }

    return errors
  }

  return {
    currentStep,
    canGoNext,
    canGoBack,
    isCurrentStepValid,
    goNext,
    goBack,
    goToStep,
    getValidationErrors,
  }
}
