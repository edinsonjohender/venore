// =============================================================================
// useTerminal - xterm.js <-> Tauri PTY bridge
// =============================================================================
// Creates an xterm.js instance, connects onData → write_terminal,
// listens terminal:output → xterm.write, exposes fit() for resize.

import { useEffect, useRef, useCallback } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { tauriApi, type TerminalOutputPayload } from '@/lib/tauri'
import '@xterm/xterm/css/xterm.css'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface UseTerminalOptions {
  terminalId: string
  containerRef: React.RefObject<HTMLDivElement | null>
}

interface UseTerminalReturn {
  fit: () => void
  clear: () => void
  copySelection: () => Promise<void>
  paste: () => Promise<void>
  focus: () => void
}

// -----------------------------------------------------------------------------
// Theme (matches CSS variables for zinc-950 background)
// -----------------------------------------------------------------------------

const TERMINAL_THEME = {
  background: '#09090b',
  foreground: '#fafafa',
  cursor: '#fafafa',
  cursorAccent: '#09090b',
  selectionBackground: '#27272a',
  selectionForeground: '#fafafa',
  black: '#09090b',
  red: '#ef4444',
  green: '#22c55e',
  yellow: '#eab308',
  blue: '#3b82f6',
  magenta: '#a855f7',
  cyan: '#06b6d4',
  white: '#fafafa',
  brightBlack: '#52525b',
  brightRed: '#f87171',
  brightGreen: '#4ade80',
  brightYellow: '#facc15',
  brightBlue: '#60a5fa',
  brightMagenta: '#c084fc',
  brightCyan: '#22d3ee',
  brightWhite: '#ffffff',
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useTerminal({ terminalId, containerRef }: UseTerminalOptions): UseTerminalReturn {
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)

  // Stable fit function
  const fit = useCallback(() => {
    const fitAddon = fitAddonRef.current
    const term = termRef.current
    if (!fitAddon || !term) return

    try {
      fitAddon.fit()
      // Notify backend of new size
      tauriApi.resizeTerminal({
        terminal_id: terminalId,
        cols: term.cols,
        rows: term.rows,
      }).catch(() => {})
    } catch {
      // fitAddon.fit() can throw if container has zero size
    }
  }, [terminalId])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    // Create terminal instance
    const term = new Terminal({
      theme: TERMINAL_THEME,
      fontFamily: "'Geist Mono', 'Cascadia Code', 'Fira Code', Consolas, monospace",
      fontSize: 13,
      lineHeight: 1.2,
      cursorBlink: true,
      cursorStyle: 'bar',
      scrollback: 5000,
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    const webLinksAddon = new WebLinksAddon()

    term.loadAddon(fitAddon)
    term.loadAddon(webLinksAddon)
    term.open(container)

    termRef.current = term
    fitAddonRef.current = fitAddon

    // Initial fit (after open)
    requestAnimationFrame(() => {
      try { fitAddon.fit() } catch {}
    })

    // Custom key handler — intercept copy/paste/clear before PTY
    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      if (e.type !== 'keydown') return true

      // Ctrl+C: copy if selection exists, otherwise let SIGINT through
      if (e.ctrlKey && !e.shiftKey && e.key === 'c') {
        if (term.hasSelection()) {
          navigator.clipboard.writeText(term.getSelection()).catch(() => {})
          term.clearSelection()
          return false
        }
        return true // SIGINT
      }

      // Ctrl+Shift+C: always copy selection
      if (e.ctrlKey && e.shiftKey && e.key === 'C') {
        if (term.hasSelection()) {
          navigator.clipboard.writeText(term.getSelection()).catch(() => {})
          term.clearSelection()
        }
        return false
      }

      // Ctrl+V / Ctrl+Shift+V: paste from clipboard
      if (e.ctrlKey && (e.key === 'v' || e.key === 'V')) {
        navigator.clipboard.readText().then((text) => {
          if (text) {
            tauriApi.writeTerminal({ terminal_id: terminalId, data: text }).catch(() => {})
          }
        }).catch(() => {})
        return false
      }

      // Ctrl+L: clear terminal display
      if (e.ctrlKey && !e.shiftKey && e.key === 'l') {
        term.clear()
        return false
      }

      return true
    })

    // User input → write to PTY
    const onDataDispose = term.onData((data) => {
      tauriApi.writeTerminal({ terminal_id: terminalId, data }).catch(() => {})
    })

    // PTY output → write to xterm
    let unlisten: UnlistenFn | null = null
    const listenPromise = listen<TerminalOutputPayload>('terminal:output', (event) => {
      if (event.payload.terminal_id === terminalId) {
        term.write(event.payload.data)
      }
    })
    listenPromise.then((fn) => { unlisten = fn })

    // Cleanup
    return () => {
      onDataDispose.dispose()
      unlisten?.()
      term.dispose()
      termRef.current = null
      fitAddonRef.current = null
    }
  }, [terminalId, containerRef]) // eslint-disable-line react-hooks/exhaustive-deps

  const clear = useCallback(() => {
    termRef.current?.clear()
  }, [])

  const copySelection = useCallback(async () => {
    const term = termRef.current
    if (term?.hasSelection()) {
      await navigator.clipboard.writeText(term.getSelection())
      term.clearSelection()
    }
  }, [])

  const paste = useCallback(async () => {
    const text = await navigator.clipboard.readText()
    if (text) {
      tauriApi.writeTerminal({ terminal_id: terminalId, data: text }).catch(() => {})
    }
  }, [terminalId])

  const focus = useCallback(() => {
    termRef.current?.focus()
  }, [])

  return { fit, clear, copySelection, paste, focus }
}
