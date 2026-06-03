// =============================================================================
// highlighter — Shiki singleton with lazy init and language detection
// =============================================================================
// Uses shiki for VS Code-quality syntax highlighting. Loads the highlighter
// lazily and detects languages from file extensions.

import { createHighlighter, type Highlighter } from 'shiki'

let highlighterInstance: Highlighter | null = null
let initPromise: Promise<Highlighter> | null = null

const LOADED_LANGS = new Set<string>()

// Bundled languages to preload (common in codebases)
const PRELOAD_LANGS = [
  'typescript', 'javascript', 'tsx', 'jsx', 'json', 'css', 'html',
  'rust', 'python', 'markdown', 'yaml', 'toml', 'bash', 'sql',
]

async function getHighlighter(): Promise<Highlighter> {
  if (highlighterInstance) return highlighterInstance
  if (initPromise) return initPromise

  initPromise = createHighlighter({
    themes: ['github-dark'],
    langs: PRELOAD_LANGS,
  })

  highlighterInstance = await initPromise
  for (const lang of PRELOAD_LANGS) LOADED_LANGS.add(lang)
  return highlighterInstance
}

/**
 * Highlight code with shiki. Returns HTML string.
 * Falls back gracefully if language isn't supported.
 */
export async function highlightCode(code: string, lang: string): Promise<string> {
  try {
    const hl = await getHighlighter()

    // Load language on demand if not preloaded
    if (!LOADED_LANGS.has(lang)) {
      try {
        await hl.loadLanguage(lang as Parameters<typeof hl.loadLanguage>[0])
        LOADED_LANGS.add(lang)
      } catch {
        // Language not supported — fall back to plaintext
        lang = 'text'
      }
    }

    return hl.codeToHtml(code, {
      lang,
      theme: 'github-dark',
    })
  } catch {
    // Highlighter failed — return escaped plain text
    return `<pre><code>${escapeHtml(code)}</code></pre>`
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

// Extension → language mapping
const EXT_MAP: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  mjs: 'javascript',
  cjs: 'javascript',
  json: 'json',
  css: 'css',
  scss: 'scss',
  less: 'less',
  html: 'html',
  htm: 'html',
  xml: 'xml',
  svg: 'xml',
  md: 'markdown',
  mdx: 'mdx',
  rs: 'rust',
  py: 'python',
  rb: 'ruby',
  go: 'go',
  java: 'java',
  kt: 'kotlin',
  swift: 'swift',
  c: 'c',
  cpp: 'cpp',
  h: 'c',
  hpp: 'cpp',
  cs: 'csharp',
  php: 'php',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  fish: 'fish',
  ps1: 'powershell',
  sql: 'sql',
  graphql: 'graphql',
  gql: 'graphql',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
  ini: 'ini',
  dockerfile: 'dockerfile',
  makefile: 'makefile',
  cmake: 'cmake',
  lua: 'lua',
  r: 'r',
  dart: 'dart',
  vue: 'vue',
  svelte: 'svelte',
  astro: 'astro',
  lock: 'text',
  txt: 'text',
  log: 'text',
  env: 'text',
}

/**
 * Detect shiki language from a filename.
 */
export function getLanguageFromFilename(filename: string): string {
  // Handle dotfiles like Dockerfile, Makefile
  const basename = filename.split('/').pop() ?? filename
  const lower = basename.toLowerCase()

  if (lower === 'dockerfile') return 'dockerfile'
  if (lower === 'makefile') return 'makefile'
  if (lower.startsWith('.env')) return 'text'

  const ext = basename.split('.').pop()?.toLowerCase() ?? ''
  return EXT_MAP[ext] ?? 'text'
}
