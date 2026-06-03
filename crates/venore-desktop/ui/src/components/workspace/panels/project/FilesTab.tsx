// =============================================================================
// FilesTab - VS Code-style file explorer tree from analysis data
// =============================================================================

import { useState, useMemo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Search, ChevronRight, ChevronDown, FileText, FolderClosed, FolderOpen, Inbox, Loader2, AlertTriangle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ModuleSummaryDto } from '@/lib/tauri'
import { useCanvasTabStore } from '@/stores/canvasTabStore'

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface FilesTabProps {
  modules: ModuleSummaryDto[]
  orphanFiles: string[]
  loading: boolean
  error: string | null
}

interface FileTreeNode {
  /** Segment name (folder or filename) */
  name: string
  /** Full relative path from project root */
  fullPath: string
  isDir: boolean
  children: FileTreeNode[]
  /** Whether this folder is a module root (has .context.md) */
  hasContext?: boolean
}

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

export function FilesTab({ modules, orphanFiles, loading, error }: FilesTabProps) {
  const { t } = useTranslation('project')
  const [search, setSearch] = useState('')
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const openFile = useCanvasTabStore((s) => s.openFile)

  // Build full file paths: "{module.path}/{filename}" for each module file
  const allPaths = useMemo(() => {
    const paths: string[] = []
    const contextDirs = new Set<string>()

    for (const m of modules) {
      const base = m.path.replace(/\\/g, '/')
      // Add .context.md if module has one
      if (m.context_path != null) {
        paths.push(`${base}/.context.md`)
        contextDirs.add(base)
      }
      for (const f of m.files) {
        paths.push(`${base}/${f}`)
      }
    }
    for (const f of orphanFiles) {
      paths.push(f)
    }
    return { paths, contextDirs }
  }, [modules, orphanFiles])

  // Filter by search
  const filteredPaths = useMemo(() => {
    if (!search.trim()) return allPaths.paths
    const q = search.toLowerCase()
    return allPaths.paths.filter((p) => p.toLowerCase().includes(q))
  }, [allPaths.paths, search])

  // Build tree
  const tree = useMemo(
    () => buildFileTree(filteredPaths, allPaths.contextDirs),
    [filteredPaths, allPaths.contextDirs],
  )

  const toggle = useCallback((path: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }, [])

  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4 h-full">
        <AlertTriangle className="w-8 h-8 text-foreground-muted/30" />
        <span className="text-xs font-medium text-foreground-muted">{t('files.noAnalysisAvailable')}</span>
        <span className="text-[10px] text-foreground-muted/60 text-center leading-relaxed">
          {t('files.runWizardHint')}
        </span>
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 text-foreground-muted/40 animate-spin" />
      </div>
    )
  }

  const totalFiles = allPaths.paths.length

  return (
    <div className="flex flex-col h-full">
      {/* Search */}
      <div className="flex items-center gap-1.5 px-2 py-1 border-b border-border shrink-0">
        <Search className="w-3 h-3 text-foreground-muted shrink-0" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t('files.searchPlaceholder')}
          className="flex-1 h-6 bg-transparent text-[11px] text-foreground placeholder:text-foreground-muted/50 outline-none"
        />
      </div>

      {/* Tree */}
      {tree.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
          <Inbox className="w-8 h-8 text-foreground-muted/30" />
          <span className="text-[11px] text-foreground-muted">
            {totalFiles === 0 ? t('files.noFiles') : t('files.noResults')}
          </span>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          {tree.map((node) => (
            <TreeRow key={node.fullPath} node={node} depth={0} collapsed={collapsed} onToggle={toggle} onFileClick={openFile} />
          ))}
        </div>
      )}

      {/* Summary */}
      <div className="flex items-center px-2 h-5 border-t border-border shrink-0">
        <span className="text-[10px] text-foreground-muted/50">
          {t('files.totalFiles', { count: filteredPaths.length })}
        </span>
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Tree row (recursive)
// -----------------------------------------------------------------------------

function TreeRow({
  node,
  depth,
  collapsed,
  onToggle,
  onFileClick,
}: {
  node: FileTreeNode
  depth: number
  collapsed: Set<string>
  onToggle: (path: string) => void
  onFileClick: (path: string) => void
}) {
  const indent = depth * 12
  const isCollapsed = collapsed.has(node.fullPath)

  if (node.isDir) {
    const Icon = isCollapsed ? FolderClosed : FolderOpen
    const Chevron = isCollapsed ? ChevronRight : ChevronDown

    return (
      <>
        <button
          onClick={() => onToggle(node.fullPath)}
          className="w-full flex items-center h-[22px] hover:bg-background-tertiary transition-colors text-left"
          style={{ paddingLeft: indent + 4 }}
        >
          <Chevron className="w-3 h-3 text-foreground-muted/40 shrink-0" />
          <Icon
            className={cn(
              'w-3.5 h-3.5 shrink-0 mx-1',
              node.hasContext ? 'text-brand/60' : 'text-foreground-muted/50',
            )}
          />
          <span className="text-[11px] text-foreground truncate">{node.name}</span>
          {node.hasContext && (
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400/60 shrink-0 ml-1.5" />
          )}
        </button>
        {!isCollapsed &&
          node.children.map((child) => (
            <TreeRow
              key={child.fullPath}
              node={child}
              depth={depth + 1}
              collapsed={collapsed}
              onToggle={onToggle}
              onFileClick={onFileClick}
            />
          ))}
      </>
    )
  }

  // File row
  const isContext = node.name === '.context.md'

  return (
    <button
      onClick={() => onFileClick(node.fullPath)}
      className="w-full flex items-center h-[22px] hover:bg-background-tertiary transition-colors text-left"
      style={{ paddingLeft: indent + 4 + 14 }}
      title={node.fullPath}
    >
      <FileText
        className={cn(
          'w-3.5 h-3.5 shrink-0 mr-1.5',
          isContext ? 'text-brand/50' : 'text-foreground-muted/35',
        )}
      />
      <span
        className={cn(
          'text-[11px] truncate',
          isContext ? 'text-brand/70' : 'text-foreground/70',
        )}
      >
        {node.name}
      </span>
    </button>
  )
}

// -----------------------------------------------------------------------------
// Build tree from flat paths
// -----------------------------------------------------------------------------

function buildFileTree(paths: string[], contextDirs: Set<string>): FileTreeNode[] {
  const root: Map<string, FileTreeNode> = new Map()

  for (const rawPath of paths) {
    const segments = rawPath.replace(/\\/g, '/').split('/').filter(Boolean)
    let currentMap = root
    let currentPath = ''

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i]
      currentPath = currentPath ? `${currentPath}/${seg}` : seg
      const isLast = i === segments.length - 1

      if (!isLast) {
        // Directory
        if (!currentMap.has(seg)) {
          const dirNode: FileTreeNode = {
            name: seg,
            fullPath: currentPath,
            isDir: true,
            children: [],
            hasContext: contextDirs.has(currentPath),
          }
          currentMap.set(seg, dirNode)
        }
        const dirNode = currentMap.get(seg)!
        // Use children map for the next level — convert array to map for lookup
        const childMap = new Map(dirNode.children.map((c) => [c.name, c]))
        // Process next segment
        const nextSeg = segments[i + 1]
        const nextIsLast = i + 1 === segments.length - 1

        if (nextIsLast) {
          // Next segment is a file
          const filePath = `${currentPath}/${nextSeg}`
          if (!childMap.has(nextSeg)) {
            const fileNode: FileTreeNode = {
              name: nextSeg,
              fullPath: filePath,
              isDir: false,
              children: [],
            }
            dirNode.children.push(fileNode)
          }
          break
        } else {
          // Next segment is another dir
          if (!childMap.has(nextSeg)) {
            const nextPath = `${currentPath}/${nextSeg}`
            const nextDirNode: FileTreeNode = {
              name: nextSeg,
              fullPath: nextPath,
              isDir: true,
              children: [],
              hasContext: contextDirs.has(nextPath),
            }
            dirNode.children.push(nextDirNode)
            childMap.set(nextSeg, nextDirNode)
          }
          // Skip to the sub-dir level — continue building inside that node
          // Recursive insert for remaining segments
          insertRemaining(childMap.get(nextSeg)!, segments.slice(i + 2), `${currentPath}/${nextSeg}`, contextDirs)
          break
        }
      } else {
        // This is a file at root level
        if (!currentMap.has(seg)) {
          const fileNode: FileTreeNode = {
            name: seg,
            fullPath: currentPath,
            isDir: false,
            children: [],
          }
          currentMap.set(seg, fileNode)
        }
      }
    }
  }

  // Sort: dirs first, then files, alphabetical
  return sortTree(Array.from(root.values()))
}

function insertRemaining(
  parent: FileTreeNode,
  segments: string[],
  basePath: string,
  contextDirs: Set<string>,
) {
  if (segments.length === 0) return

  const childMap = new Map(parent.children.map((c) => [c.name, c]))

  if (segments.length === 1) {
    // File
    const fileName = segments[0]
    if (!childMap.has(fileName)) {
      parent.children.push({
        name: fileName,
        fullPath: `${basePath}/${fileName}`,
        isDir: false,
        children: [],
      })
    }
    return
  }

  // Directory + more
  const dirName = segments[0]
  const dirPath = `${basePath}/${dirName}`
  if (!childMap.has(dirName)) {
    const dirNode: FileTreeNode = {
      name: dirName,
      fullPath: dirPath,
      isDir: true,
      children: [],
      hasContext: contextDirs.has(dirPath),
    }
    parent.children.push(dirNode)
    childMap.set(dirName, dirNode)
  }

  insertRemaining(childMap.get(dirName)!, segments.slice(1), dirPath, contextDirs)
}

function sortTree(nodes: FileTreeNode[]): FileTreeNode[] {
  nodes.sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
    return a.name.localeCompare(b.name)
  })
  for (const node of nodes) {
    if (node.isDir && node.children.length > 0) {
      node.children = sortTree(node.children)
    }
  }
  return nodes
}
