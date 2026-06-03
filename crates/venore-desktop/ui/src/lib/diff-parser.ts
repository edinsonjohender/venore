// =============================================================================
// diff-parser — Parse unified diff patches into structured data
// =============================================================================
// Pure utility (no I/O). Parses the patch string from GitHub's PR files API
// into hunks and lines with type annotations and line numbers.

export interface DiffHunk {
  oldStart: number
  oldCount: number
  newStart: number
  newCount: number
  header: string
  lines: DiffLine[]
}

export interface DiffLine {
  type: 'context' | 'add' | 'remove' | 'hunk-header'
  content: string
  oldLineNo?: number
  newLineNo?: number
}

const HUNK_HEADER_RE = /^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@(.*)$/

/**
 * Parse a unified diff patch string into an array of hunks.
 */
export function parsePatch(patch: string): DiffHunk[] {
  const lines = patch.split('\n')
  const hunks: DiffHunk[] = []
  let currentHunk: DiffHunk | null = null
  let oldLine = 0
  let newLine = 0

  for (const line of lines) {
    const hunkMatch = line.match(HUNK_HEADER_RE)

    if (hunkMatch) {
      const oldStart = parseInt(hunkMatch[1], 10)
      const oldCount = hunkMatch[2] !== undefined ? parseInt(hunkMatch[2], 10) : 1
      const newStart = parseInt(hunkMatch[3], 10)
      const newCount = hunkMatch[4] !== undefined ? parseInt(hunkMatch[4], 10) : 1

      currentHunk = {
        oldStart,
        oldCount,
        newStart,
        newCount,
        header: line,
        lines: [{
          type: 'hunk-header',
          content: line,
        }],
      }
      hunks.push(currentHunk)
      oldLine = oldStart
      newLine = newStart
      continue
    }

    if (!currentHunk) continue

    if (line.startsWith('+')) {
      currentHunk.lines.push({
        type: 'add',
        content: line.slice(1),
        newLineNo: newLine,
      })
      newLine++
    } else if (line.startsWith('-')) {
      currentHunk.lines.push({
        type: 'remove',
        content: line.slice(1),
        oldLineNo: oldLine,
      })
      oldLine++
    } else if (line.startsWith('\\')) {
      // "\ No newline at end of file" — skip
    } else {
      // Context line (starts with space or is empty)
      currentHunk.lines.push({
        type: 'context',
        content: line.startsWith(' ') ? line.slice(1) : line,
        oldLineNo: oldLine,
        newLineNo: newLine,
      })
      oldLine++
      newLine++
    }
  }

  return hunks
}
