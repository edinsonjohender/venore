// =============================================================================
// Monaco bootstrap — bundle the editor locally instead of loading from a CDN
// =============================================================================
// By default `@monaco-editor/react` downloads Monaco from jsdelivr at runtime.
// That (a) requires an internet connection and (b) is blocked by the app's CSP
// in release builds — `script-src` does not allow the CDN, so the editor stays
// stuck on "Loading…" while Preview (plain React) keeps working.
//
// This module points the loader at the locally bundled copy of `monaco-editor`
// and wires its web workers through Vite (`?worker`), so every editor works
// offline and within the CSP (`worker-src 'self' blob:` already permits them).
//
// Import once, for side effects, before React renders (see `main.tsx`).

import { loader } from '@monaco-editor/react'
import * as monaco from 'monaco-editor'

import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'

// Resolve the Vite-bundled worker for each Monaco language label. Unmapped
// languages (markdown, rust, python, …) tokenize on the main thread and only
// need the generic editor worker.
self.MonacoEnvironment = {
  getWorker(_workerId, label) {
    switch (label) {
      case 'json':
        return new jsonWorker()
      case 'css':
      case 'scss':
      case 'less':
        return new cssWorker()
      case 'html':
      case 'handlebars':
      case 'razor':
        return new htmlWorker()
      case 'typescript':
      case 'javascript':
        return new tsWorker()
      default:
        return new editorWorker()
    }
  },
}

// Use the locally bundled Monaco instead of the default CDN download.
loader.config({ monaco })
