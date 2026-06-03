// =============================================================================
// useAnimatedPresence - Mount/unmount lifecycle with animation support
// =============================================================================
// Keeps a component mounted during exit animations, then unmounts after duration.
//
// Phases:
//   pre-enter → entering → idle    (mount animation)
//   idle → exiting → unmounted     (unmount animation)
//
// CSS transitions handle the visual interpolation. This hook just manages
// the mount/unmount lifecycle and phase state.
//
// onAnimStart/onAnimEnd callbacks let the consumer signal external systems
// (e.g. switching R3F frameloop) without coupling this hook to them.

import { useState, useLayoutEffect, useRef, useCallback } from 'react'

export type AnimPhase = 'pre-enter' | 'entering' | 'idle' | 'exiting'

interface UseAnimatedPresenceOptions {
  /** Called when an enter/exit animation starts. */
  onAnimStart?: () => void
  /** Called when an enter/exit animation ends (or is cancelled). */
  onAnimEnd?: () => void
}

interface AnimatedPresenceResult {
  shouldRender: boolean
  phase: AnimPhase
}

export function useAnimatedPresence(
  isPresent: boolean,
  durationMs: number,
  options?: UseAnimatedPresenceOptions,
): AnimatedPresenceResult {
  const [shouldRender, setShouldRender] = useState(isPresent)
  const [phase, setPhase] = useState<AnimPhase>(isPresent ? 'idle' : 'pre-enter')

  const rafId = useRef(0)
  const timerId = useRef(0)
  const isAnimatingRef = useRef(false)

  // Keep callbacks in refs to avoid re-running the effect when they change
  const onAnimStartRef = useRef(options?.onAnimStart)
  const onAnimEndRef = useRef(options?.onAnimEnd)
  onAnimStartRef.current = options?.onAnimStart
  onAnimEndRef.current = options?.onAnimEnd

  const stopAnim = useCallback(() => {
    if (isAnimatingRef.current) {
      isAnimatingRef.current = false
      onAnimEndRef.current?.()
    }
  }, [])

  const clearTimers = useCallback(() => {
    cancelAnimationFrame(rafId.current)
    window.clearTimeout(timerId.current)
    rafId.current = 0
    timerId.current = 0
    stopAnim()
  }, [stopAnim])

  // useLayoutEffect: fires synchronously after commit, before paint.
  // State updates here are flushed immediately, guaranteeing the browser
  // computes pre-enter (width=0) before any RAF fires. useEffect's async
  // scheduling caused an intermittent race where React batched pre-enter
  // and entering into a single commit, skipping the transition.
  useLayoutEffect(() => {
    clearTimers()

    if (isPresent) {
      // --- ENTER ---
      setShouldRender(true)
      setPhase('pre-enter')

      isAnimatingRef.current = true
      onAnimStartRef.current?.()

      // Single RAF is sufficient with useLayoutEffect — pre-enter is
      // already committed to the DOM synchronously before paint.
      rafId.current = requestAnimationFrame(() => {
        setPhase('entering')

        timerId.current = window.setTimeout(() => {
          stopAnim()
          setPhase('idle')
        }, durationMs)
      })
    } else if (shouldRender) {
      // --- EXIT ---
      setPhase('exiting')

      isAnimatingRef.current = true
      onAnimStartRef.current?.()

      timerId.current = window.setTimeout(() => {
        setShouldRender(false)
        // Delay onAnimEnd by one RAF so consumers (e.g. frameloop="always")
        // remain active while the canvas resizes after panel unmount
        rafId.current = requestAnimationFrame(() => {
          stopAnim()
        })
      }, durationMs)
    }

    return clearTimers
  }, [isPresent]) // eslint-disable-line react-hooks/exhaustive-deps -- intentional: clearTimers/durationMs are stable

  return { shouldRender, phase }
}
