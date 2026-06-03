// =============================================================================
// FileEditorView — Monaco Editor for editing project files
// =============================================================================
// Loads file content from backend, provides syntax-highlighted editing,
// tracks dirty state, and saves with Ctrl+S.

import { useState, useEffect, useRef, useCallback } from 'react'
import Editor, { type BeforeMount, type OnMount } from '@monaco-editor/react'
import { Loader2 } from 'lucide-react'
import { tauriApi } from '@/lib/tauri'
import { useCanvasTabStore } from '@/stores/canvasTabStore'

// -----------------------------------------------------------------------------
// Monaco configuration — run once
// -----------------------------------------------------------------------------

let monacoConfigured = false

function configureMonaco(monaco: Parameters<BeforeMount>[0]) {
  if (monacoConfigured) return
  monacoConfigured = true

  // TypeScript: support JSX/TSX, modern syntax, no type-checking errors
  const tsDefaults = monaco.languages.typescript.typescriptDefaults
  tsDefaults.setCompilerOptions({
    target: monaco.languages.typescript.ScriptTarget.ESNext,
    module: monaco.languages.typescript.ModuleKind.ESNext,
    moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
    jsx: monaco.languages.typescript.JsxEmit.ReactJSX,
    allowJs: true,
    esModuleInterop: true,
    allowNonTsExtensions: true,
  })
  // Disable semantic diagnostics (type errors) — we don't have node_modules types
  tsDefaults.setDiagnosticsOptions({
    noSemanticValidation: true,
    noSuggestionDiagnostics: true,
  })

  // Same for JavaScript files
  const jsDefaults = monaco.languages.typescript.javascriptDefaults
  jsDefaults.setCompilerOptions({
    target: monaco.languages.typescript.ScriptTarget.ESNext,
    module: monaco.languages.typescript.ModuleKind.ESNext,
    jsx: monaco.languages.typescript.JsxEmit.ReactJSX,
    allowJs: true,
    allowNonTsExtensions: true,
  })
  jsDefaults.setDiagnosticsOptions({
    noSemanticValidation: true,
    noSuggestionDiagnostics: true,
  })
}

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface FileEditorViewProps {
  relativePath: string
  projectPath: string
  tabId: string
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function FileEditorView({ relativePath, projectPath, tabId }: FileEditorViewProps) {
  const [content, setContent] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  const originalContentRef = useRef<string>('')
  const currentContentRef = useRef<string>('')
  const setTabDirty = useCanvasTabStore((s) => s.setTabDirty)

  // Load file content on mount
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)

    tauriApi.readFile({ project_path: projectPath, relative_path: relativePath })
      .then((res) => {
        if (cancelled) return
        originalContentRef.current = res.content
        currentContentRef.current = res.content
        setContent(res.content)
        setLoading(false)
      })
      .catch((err) => {
        if (cancelled) return
        setError(err.message || 'Failed to read file')
        setLoading(false)
      })

    return () => { cancelled = true }
  }, [projectPath, relativePath])

  // Save handler
  const handleSave = useCallback(async () => {
    if (saving) return
    setSaving(true)
    try {
      await tauriApi.writeFile({
        project_path: projectPath,
        relative_path: relativePath,
        content: currentContentRef.current,
      })
      originalContentRef.current = currentContentRef.current
      setTabDirty(tabId, false)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Save failed'
      console.error('File save error:', msg)
    } finally {
      setSaving(false)
    }
  }, [projectPath, relativePath, tabId, setTabDirty, saving])

  // Editor mount — register Ctrl+S + fix font metrics
  const handleEditorMount: OnMount = useCallback((editor, monaco) => {
    // Monaco KeyMod/KeyCode: CtrlCmd = 2048, KeyS = 49
    editor.addCommand(2048 | 49, () => {
      handleSave()
    })

    // Geist Mono may not be loaded yet — remeasure once fonts are ready
    document.fonts.ready.then(() => {
      monaco.editor.remeasureFonts()
    })
  }, [handleSave])

  // Content change handler
  const handleChange = useCallback((value: string | undefined) => {
    const newContent = value ?? ''
    currentContentRef.current = newContent
    const isDirty = newContent !== originalContentRef.current
    setTabDirty(tabId, isDirty)
  }, [tabId, setTabDirty])

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center bg-background-tertiary">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 bg-background-tertiary">
        <span className="text-xs text-foreground-muted">Failed to open file</span>
        <span className="text-[10px] text-foreground-muted/60">{error}</span>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-background-tertiary">
      {/* Status bar */}
      <div className="flex items-center h-7 px-3 border-b border-border shrink-0">
        <span className="text-[11px] text-foreground-muted/60 truncate flex-1">
          {relativePath.replace(/\\/g, '/')}
        </span>
        {saving && (
          <span className="text-[10px] text-foreground-muted/50 ml-2 shrink-0">
            Saving...
          </span>
        )}
      </div>

      {/* Monaco Editor — absolute wrapper so Monaco gets real pixel dimensions */}
      <div className="flex-1 relative overflow-hidden">
        <div className="absolute inset-0">
        <Editor
          defaultValue={content ?? ''}
          path={`file:///${relativePath.replace(/\\/g, '/')}`}
          theme="vs-dark"
          beforeMount={configureMonaco}
          onMount={handleEditorMount}
          onChange={handleChange}
          options={{
            minimap: { enabled: false },
            fontSize: 13,
            fontFamily: "'Geist Mono', monospace",
            lineNumbers: 'on',
            scrollBeyondLastLine: false,
            wordWrap: 'on',
            automaticLayout: true,
            padding: { top: 8 },
          }}
        />
        </div>
      </div>
    </div>
  )
}
