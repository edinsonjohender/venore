// =============================================================================
// CodeBlock - Syntax-highlighted fenced code with language label + copy button
// =============================================================================

import { useState, useEffect, useCallback } from 'react'
import { Copy, Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { highlightCode } from '@/lib/highlighter'
import { cn } from '@/lib/utils'

// -----------------------------------------------------------------------------
// Language display names
// -----------------------------------------------------------------------------

const LANG_DISPLAY: Record<string, string> = {
  ts: 'TypeScript',
  tsx: 'TSX',
  typescript: 'TypeScript',
  js: 'JavaScript',
  jsx: 'JSX',
  javascript: 'JavaScript',
  py: 'Python',
  python: 'Python',
  rs: 'Rust',
  rust: 'Rust',
  go: 'Go',
  java: 'Java',
  rb: 'Ruby',
  ruby: 'Ruby',
  cpp: 'C++',
  c: 'C',
  cs: 'C#',
  csharp: 'C#',
  php: 'PHP',
  swift: 'Swift',
  kotlin: 'Kotlin',
  dart: 'Dart',
  sql: 'SQL',
  html: 'HTML',
  css: 'CSS',
  scss: 'SCSS',
  json: 'JSON',
  yaml: 'YAML',
  toml: 'TOML',
  bash: 'Bash',
  sh: 'Shell',
  shell: 'Shell',
  zsh: 'Zsh',
  powershell: 'PowerShell',
  markdown: 'Markdown',
  md: 'Markdown',
  xml: 'XML',
  graphql: 'GraphQL',
  dockerfile: 'Dockerfile',
  makefile: 'Makefile',
  lua: 'Lua',
  r: 'R',
  vue: 'Vue',
  svelte: 'Svelte',
  text: 'Text',
}

function displayLang(lang: string): string {
  return LANG_DISPLAY[lang.toLowerCase()] ?? lang.toUpperCase()
}

// -----------------------------------------------------------------------------
// Props
// -----------------------------------------------------------------------------

interface CodeBlockProps {
  code: string
  language?: string
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function CodeBlock({ code, language }: CodeBlockProps) {
  const { t } = useTranslation('common')
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  const lang = language || 'text'

  useEffect(() => {
    let cancelled = false
    highlightCode(code, lang).then((html) => {
      if (!cancelled) setHighlightedHtml(html)
    })
    return () => { cancelled = true }
  }, [code, lang])

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code)
    setCopied(true)
    const timer = setTimeout(() => setCopied(false), 2000)
    return () => clearTimeout(timer)
  }, [code])

  return (
    <div className="rounded-lg border border-border overflow-hidden mb-3">
      {/* Header bar */}
      <div className="flex items-center justify-between bg-background-secondary/80 border-b border-border px-3 py-1.5">
        <span className="text-[10px] font-mono text-foreground-muted tracking-wide">
          {displayLang(lang)}
        </span>
        <button
          type="button"
          onClick={handleCopy}
          className={cn(
            'inline-flex items-center gap-1 text-[10px] font-mono transition-colors',
            copied
              ? 'text-brand'
              : 'text-foreground-subtle hover:text-foreground-muted',
          )}
        >
          {copied ? (
            <>
              <Check className="w-3 h-3" />
              {t('copied')}
            </>
          ) : (
            <>
              <Copy className="w-3 h-3" />
              {t('copy')}
            </>
          )}
        </button>
      </div>

      {/* Code area */}
      <div className="bg-background p-3 overflow-x-auto">
        {highlightedHtml ? (
          <div
            className="code-block-shiki"
            dangerouslySetInnerHTML={{ __html: highlightedHtml }}
          />
        ) : (
          <pre className="m-0 p-0">
            <code className="font-mono text-xs leading-[1.6] text-foreground/80">
              {code}
            </code>
          </pre>
        )}
      </div>
    </div>
  )
}
