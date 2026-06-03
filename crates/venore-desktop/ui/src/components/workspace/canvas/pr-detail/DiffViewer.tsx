// =============================================================================
// DiffViewer — Syntax-highlighted diff with line numbers
// =============================================================================
// Parses the patch, renders with line numbers and colored backgrounds.
// Progressive: shows plain diff immediately, then re-renders with shiki.

import { useEffect, useState, useMemo } from 'react'
import { cn } from '@/lib/utils'
import { parsePatch, type DiffHunk, type DiffLine } from '@/lib/diff-parser'
import { highlightCode, getLanguageFromFilename } from '@/lib/highlighter'
import type { GitHubPrFileDto } from '@/lib/tauri'

interface DiffViewerProps {
  file: GitHubPrFileDto
}

// Limit for auto-expanding large diffs
const LARGE_DIFF_THRESHOLD = 500

export function DiffViewer({ file }: DiffViewerProps) {
  const [expanded, setExpanded] = useState(false)

  const hunks = useMemo(() => {
    if (!file.patch) return []
    return parsePatch(file.patch)
  }, [file.patch])

  const totalLines = useMemo(
    () => hunks.reduce((acc, h) => acc + h.lines.length, 0),
    [hunks],
  )

  const isLarge = totalLines > LARGE_DIFF_THRESHOLD

  if (!file.patch) {
    return (
      <div className="flex-1 flex items-center justify-center text-xs text-foreground-muted/50">
        Binary file or no diff available
      </div>
    )
  }

  if (isLarge && !expanded) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-xs text-foreground-muted/50">
        <span>Large diff ({totalLines} lines)</span>
        <button
          onClick={() => setExpanded(true)}
          className="px-3 py-1 rounded bg-background-tertiary text-foreground-muted hover:text-foreground transition-colors"
        >
          Expand
        </button>
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-auto">
      {/* File header */}
      <div className="sticky top-0 z-10 flex items-center gap-2 px-3 py-1.5 bg-background-secondary border-b border-border text-[11px]">
        <span className="font-mono text-foreground truncate">{file.filename}</span>
        <span className="shrink-0 text-green-400">+{file.additions}</span>
        <span className="shrink-0 text-red-400">-{file.deletions}</span>
      </div>

      {/* Hunks */}
      <div className="font-mono text-[11px] leading-[18px]">
        {hunks.map((hunk, i) => (
          <HunkView key={i} hunk={hunk} filename={file.filename} />
        ))}
      </div>
    </div>
  )
}

function HunkView({ hunk, filename }: { hunk: DiffHunk; filename: string }) {
  const lang = getLanguageFromFilename(filename)
  const [highlightedLines, setHighlightedLines] = useState<Map<number, string> | null>(null)

  // Progressive highlighting: highlight all non-header lines in the hunk
  useEffect(() => {
    let cancelled = false

    const codeLines = hunk.lines
      .map((line, idx) => ({ line, idx }))
      .filter(({ line }) => line.type !== 'hunk-header')

    if (codeLines.length === 0) return

    // Highlight each line individually for proper coloring
    const code = codeLines.map(({ line }) => line.content).join('\n')

    highlightCode(code, lang).then((html) => {
      if (cancelled) return

      // Extract individual lines from the highlighted HTML
      // shiki wraps each line in a span inside <code>
      const lineMap = new Map<number, string>()
      const parser = new DOMParser()
      const doc = parser.parseFromString(html, 'text/html')
      const codeEl = doc.querySelector('code')
      if (codeEl) {
        const spans = codeEl.querySelectorAll('.line')
        spans.forEach((span, i) => {
          if (i < codeLines.length) {
            lineMap.set(codeLines[i].idx, span.innerHTML)
          }
        })
      }
      setHighlightedLines(lineMap)
    })

    return () => { cancelled = true }
  }, [hunk, lang])

  return (
    <table className="w-full border-collapse">
      <tbody>
        {hunk.lines.map((line, idx) => (
          <DiffLineRow
            key={idx}
            line={line}
            highlightedHtml={highlightedLines?.get(idx) ?? null}
          />
        ))}
      </tbody>
    </table>
  )
}

function DiffLineRow({
  line,
  highlightedHtml,
}: {
  line: DiffLine
  highlightedHtml: string | null
}) {
  if (line.type === 'hunk-header') {
    return (
      <tr className="bg-blue-500/8">
        <td colSpan={3} className="px-3 py-0.5 text-blue-400 select-none">
          {line.content}
        </td>
      </tr>
    )
  }

  const bgClass =
    line.type === 'add'
      ? 'bg-green-500/10'
      : line.type === 'remove'
        ? 'bg-red-500/10'
        : ''

  const gutterClass =
    line.type === 'add'
      ? 'bg-green-500/15 text-green-400/60'
      : line.type === 'remove'
        ? 'bg-red-500/15 text-red-400/60'
        : 'text-foreground-muted/30'

  const prefixChar =
    line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' '

  return (
    <tr className={cn(bgClass, 'hover:brightness-110')}>
      {/* Old line number */}
      <td className={cn('w-[50px] px-2 text-right select-none text-[10px] align-top', gutterClass)}>
        {line.oldLineNo ?? ''}
      </td>
      {/* New line number */}
      <td className={cn('w-[50px] px-2 text-right select-none text-[10px] align-top border-r border-border/20', gutterClass)}>
        {line.newLineNo ?? ''}
      </td>
      {/* Code content */}
      <td className="px-3 whitespace-pre-wrap break-all">
        <span className={cn(
          'select-none mr-1',
          line.type === 'add' && 'text-green-400',
          line.type === 'remove' && 'text-red-400',
          line.type === 'context' && 'text-foreground-muted/30',
        )}>
          {prefixChar}
        </span>
        {highlightedHtml ? (
          <span dangerouslySetInnerHTML={{ __html: highlightedHtml }} />
        ) : (
          <span className="text-foreground">{line.content}</span>
        )}
      </td>
    </tr>
  )
}
