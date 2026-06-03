// =============================================================================
// FileTree — PR file tree grouped by directory (left panel)
// =============================================================================

import { useState, useMemo } from 'react'
import {
  ChevronRight, ChevronDown, FileCode, FolderOpen, Folder,
  Plus, Minus,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { GitHubPrFileDto } from '@/lib/tauri'

interface FileTreeProps {
  files: GitHubPrFileDto[]
  selectedFile: string | null
  onSelectFile: (filename: string) => void
}

interface DirNode {
  name: string
  path: string
  files: GitHubPrFileDto[]
  children: DirNode[]
}

function buildTree(files: GitHubPrFileDto[]): DirNode {
  const root: DirNode = { name: '', path: '', files: [], children: [] }

  for (const file of files) {
    const parts = file.filename.split('/')
    let current = root

    // Navigate/create directory nodes
    for (let i = 0; i < parts.length - 1; i++) {
      const dirName = parts[i]
      const dirPath = parts.slice(0, i + 1).join('/')
      let child = current.children.find((c) => c.name === dirName)
      if (!child) {
        child = { name: dirName, path: dirPath, files: [], children: [] }
        current.children.push(child)
      }
      current = child
    }

    current.files.push(file)
  }

  return root
}

// Flatten single-child directory chains: "src" > "components" → "src/components"
function collapseTree(node: DirNode): DirNode {
  // Recursively collapse children first
  node.children = node.children.map(collapseTree)

  // If a dir has exactly 1 child dir and no files, merge them
  if (node.children.length === 1 && node.files.length === 0 && node.name !== '') {
    const child = node.children[0]
    return {
      name: `${node.name}/${child.name}`,
      path: child.path,
      files: child.files,
      children: child.children,
    }
  }

  return node
}

function statusColor(status: string) {
  switch (status) {
    case 'added': return 'bg-green-500/15 text-green-400'
    case 'removed': return 'bg-red-500/15 text-red-400'
    case 'modified': return 'bg-blue-500/15 text-blue-400'
    case 'renamed': return 'bg-amber-500/15 text-amber-400'
    default: return 'bg-foreground-muted/10 text-foreground-muted'
  }
}

function DirEntry({
  node,
  selectedFile,
  onSelectFile,
  depth,
}: {
  node: DirNode
  selectedFile: string | null
  onSelectFile: (f: string) => void
  depth: number
}) {
  const [expanded, setExpanded] = useState(true)

  return (
    <div>
      {node.name && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center gap-1 px-2 py-1 text-[11px] text-foreground-muted hover:bg-background-tertiary/50 transition-colors"
          style={{ paddingLeft: `${depth * 12 + 8}px` }}
        >
          {expanded ? (
            <ChevronDown className="w-3 h-3 shrink-0" />
          ) : (
            <ChevronRight className="w-3 h-3 shrink-0" />
          )}
          {expanded ? (
            <FolderOpen className="w-3.5 h-3.5 shrink-0 text-brand/70" />
          ) : (
            <Folder className="w-3.5 h-3.5 shrink-0 text-brand/70" />
          )}
          <span className="truncate font-medium">{node.name}</span>
        </button>
      )}
      {expanded && (
        <>
          {node.children.map((child) => (
            <DirEntry
              key={child.path}
              node={child}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              depth={node.name ? depth + 1 : depth}
            />
          ))}
          {node.files.map((file) => (
            <FileEntry
              key={file.filename}
              file={file}
              selected={selectedFile === file.filename}
              onSelect={() => onSelectFile(file.filename)}
              depth={node.name ? depth + 1 : depth}
            />
          ))}
        </>
      )}
    </div>
  )
}

function FileEntry({
  file,
  selected,
  onSelect,
  depth,
}: {
  file: GitHubPrFileDto
  selected: boolean
  onSelect: () => void
  depth: number
}) {
  const name = file.filename.split('/').pop() ?? file.filename

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full flex items-center gap-1.5 px-2 py-1 text-[11px] transition-colors',
        selected
          ? 'bg-background-tertiary text-foreground'
          : 'text-foreground-muted hover:bg-background-tertiary/50',
      )}
      style={{ paddingLeft: `${depth * 12 + 8}px` }}
    >
      <FileCode className="w-3.5 h-3.5 shrink-0" />
      <span className="flex-1 truncate text-left">{name}</span>
      <span className={cn('shrink-0 text-[9px] px-1 py-0.5 rounded font-medium leading-none', statusColor(file.status))}>
        {file.status[0].toUpperCase()}
      </span>
      <span className="shrink-0 flex items-center gap-1 text-[9px]">
        {file.additions > 0 && (
          <span className="text-green-400 flex items-center"><Plus className="w-2.5 h-2.5" />{file.additions}</span>
        )}
        {file.deletions > 0 && (
          <span className="text-red-400 flex items-center"><Minus className="w-2.5 h-2.5" />{file.deletions}</span>
        )}
      </span>
    </button>
  )
}

export function FileTree({ files, selectedFile, onSelectFile }: FileTreeProps) {
  const tree = useMemo(() => {
    const raw = buildTree(files)
    // Collapse root's children (not root itself)
    raw.children = raw.children.map(collapseTree)
    return raw
  }, [files])

  return (
    <div className="h-full overflow-y-auto py-1">
      <DirEntry
        node={tree}
        selectedFile={selectedFile}
        onSelectFile={onSelectFile}
        depth={0}
      />
    </div>
  )
}
