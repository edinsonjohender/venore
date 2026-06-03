// =============================================================================
// ConnectionsDialog — Manage manual connections from a single source node
// =============================================================================
// Self-sufficient: fetches its own snapshot of the layout and listens to the
// ocean-connections-changed event. Candidates are grouped (Lighthouses ·
// Standalone · Modules) so dozens of nodes stay scannable without relying on
// names alone.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Box, Check, Lightbulb, Search, X } from 'lucide-react'

import { Button } from '../ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog'
import { Input } from '../ui/input'
import {
  tauriApi,
  type OceanConnectionDto,
  type OceanNodePosition,
} from '@/lib/tauri'
import { cn } from '@/lib/utils'

export interface ConnectionsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The node we're managing connections from. Falsy = dialog hidden. */
  source: { module_id: string; module_name: string; node_variant: string } | null
  /** Project root path (needed for backend mutations). */
  projectPath: string
}

// Mirrors NodeLogbook's palette so the dot color = subtype color the user sees
// elsewhere in the app.
const SUBTYPE_DOT: Record<string, string> = {
  concept: 'bg-sky-400',
  feature: 'bg-violet-400',
  decision: 'bg-emerald-400',
  finding: 'bg-amber-400',
  question: 'bg-pink-400',
}

interface CandidateGroup {
  id: string
  title: string
  icon: React.ReactNode
  nodes: OceanNodePosition[]
}

export function ConnectionsDialog({
  open,
  onOpenChange,
  source,
  projectPath,
}: ConnectionsDialogProps) {
  const [query, setQuery] = useState('')
  const [busyId, setBusyId] = useState<string | null>(null)
  const [nodes, setNodes] = useState<OceanNodePosition[]>([])
  const [connections, setConnections] = useState<OceanConnectionDto[]>([])

  const refetch = useCallback(async () => {
    if (!projectPath) return
    try {
      const layout = await tauriApi.initializeOceanLayout({ project_path: projectPath })
      setNodes(layout.nodes)
      setConnections(layout.connections)
    } catch (err) {
      console.error('Failed to load ocean snapshot:', err)
    }
  }, [projectPath])

  useEffect(() => {
    if (!open) {
      setQuery('')
      return
    }
    refetch()
  }, [open, refetch])

  useEffect(() => {
    if (!open) return
    let cancelled = false
    const unlisten = listen<{ project_path: string }>(
      'ocean-connections-changed',
      (event) => {
        if (cancelled) return
        if (event.payload.project_path !== projectPath) return
        refetch()
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [open, projectPath, refetch])

  // Map: target_id → existing manual connection id. Used to know whether a
  // candidate is already connected and which connection to delete on toggle.
  const outgoingManual = useMemo(() => {
    const map = new Map<string, string>()
    if (!source) return map
    for (const c of connections) {
      if (c.kind !== 'manual') continue
      if (c.from_id !== source.module_id) continue
      map.set(c.to_id, c.id)
    }
    return map
  }, [connections, source])

  // Build the grouped, filtered, sorted candidate list. Each group is rendered
  // only if non-empty (so an empty filter result hides the whole section).
  const groups: CandidateGroup[] = useMemo(() => {
    if (!source) return []
    const lower = query.trim().toLowerCase()
    const matchesQuery = (text: string) =>
      lower ? text.toLowerCase().includes(lower) : true

    const candidates = nodes.filter((n) => n.module_id !== source.module_id)

    // Index lighthouses for grouping. A lighthouse appears in its own group
    // (you can connect to a lighthouse) and as the group header.
    const lighthouses = candidates.filter((n) => n.node_variant === 'lighthouse')

    // Sort connected first, then by name.
    const sortGroup = (a: OceanNodePosition, b: OceanNodePosition) => {
      const aConn = outgoingManual.has(a.module_id) ? 0 : 1
      const bConn = outgoingManual.has(b.module_id) ? 0 : 1
      if (aConn !== bConn) return aConn - bConn
      return a.module_name.localeCompare(b.module_name)
    }

    const result: CandidateGroup[] = []

    // One group per lighthouse: the lighthouse itself + its children.
    for (const lh of lighthouses) {
      const members = candidates.filter(
        (n) =>
          n.module_id === lh.module_id || n.lighthouse_id === lh.module_id,
      )
      const filtered = members
        .filter(
          (n) => matchesQuery(n.module_name) || matchesQuery(lh.module_name),
        )
        .sort(sortGroup)
      if (filtered.length === 0) continue
      result.push({
        id: `lh-${lh.module_id}`,
        title: lh.module_name,
        icon: <Lightbulb className="h-3.5 w-3.5 text-amber-400 shrink-0" />,
        nodes: filtered,
      })
    }

    // Loose knowledge nodes (no lighthouse).
    const loose = candidates.filter(
      (n) => n.node_variant === 'knowledge_node' && !n.lighthouse_id,
    )
    const looseFiltered = loose.filter((n) => matchesQuery(n.module_name)).sort(sortGroup)
    if (looseFiltered.length > 0) {
      result.push({
        id: 'loose',
        title: 'Sin isla',
        icon: <Box className="h-3.5 w-3.5 text-foreground-muted/60 shrink-0" />,
        nodes: looseFiltered,
      })
    }

    // Modules (code).
    const modules = candidates.filter((n) => n.node_variant === 'module')
    const modulesFiltered = modules.filter((n) => matchesQuery(n.module_name)).sort(sortGroup)
    if (modulesFiltered.length > 0) {
      result.push({
        id: 'modules',
        title: 'Modules',
        icon: <Box className="h-3.5 w-3.5 text-foreground-muted/60 shrink-0" />,
        nodes: modulesFiltered,
      })
    }

    return result
  }, [nodes, source, query, outgoingManual])

  const handleToggle = async (target: OceanNodePosition) => {
    if (!source || busyId) return
    setBusyId(target.module_id)
    const existing = outgoingManual.get(target.module_id)
    try {
      if (existing) {
        await tauriApi.deleteOceanConnection({
          project_path: projectPath,
          connection_id: existing,
        })
      } else {
        await tauriApi.createOceanConnection({
          project_path: projectPath,
          from_id: source.module_id,
          to_id: target.module_id,
        })
      }
    } catch (err) {
      console.error('Toggle connection failed:', err)
    } finally {
      setBusyId(null)
    }
  }

  if (!source) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <NodeIcon variant={source.node_variant} />
            Conexiones de {source.module_name}
          </DialogTitle>
          <DialogDescription>
            Click para conectar; click de nuevo para desconectar.
          </DialogDescription>
        </DialogHeader>

        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-foreground-muted/60" />
          <Input
            placeholder="Buscar nodo o faro..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
            className="pl-9"
          />
        </div>

        <div className="max-h-80 overflow-y-auto -mx-6 px-6">
          {groups.length === 0 ? (
            <div className="py-8 text-center text-sm text-foreground-muted/60">
              {query ? 'No matching node' : 'No other nodes in the project'}
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              {groups.map((group) => (
                <section key={group.id}>
                  <div className="flex items-center gap-1.5 px-1 mb-1 text-[10px] uppercase tracking-wider text-foreground-muted/60">
                    {group.icon}
                    <span className="flex-1 truncate">{group.title}</span>
                    <span className="tabular-nums">{group.nodes.length}</span>
                  </div>
                  <ul className="flex flex-col gap-0.5">
                    {group.nodes.map((n) => (
                      <CandidateRow
                        key={n.module_id}
                        node={n}
                        connected={outgoingManual.has(n.module_id)}
                        busy={busyId === n.module_id}
                        onToggle={() => handleToggle(n)}
                      />
                    ))}
                  </ul>
                </section>
              ))}
            </div>
          )}
        </div>

        <div className="flex justify-end">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            <X className="mr-1 h-4 w-4" />
            Cerrar
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

// -----------------------------------------------------------------------------
// One candidate row — colored dot, name, optional section count, connected hint
// -----------------------------------------------------------------------------

function CandidateRow({
  node,
  connected,
  busy,
  onToggle,
}: {
  node: OceanNodePosition
  connected: boolean
  busy: boolean
  onToggle: () => void
}) {
  const isModule = node.node_variant === 'module'
  const isLighthouse = node.node_variant === 'lighthouse'

  return (
    <li>
      <button
        type="button"
        onClick={onToggle}
        disabled={busy}
        className={cn(
          'flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm transition-colors',
          connected
            ? 'bg-emerald-500/10 text-emerald-200 hover:bg-emerald-500/20'
            : 'text-foreground hover:bg-foreground/5',
          busy && 'opacity-50 cursor-wait',
        )}
      >
        {isLighthouse ? (
          <Lightbulb className="w-3.5 h-3.5 text-amber-400 shrink-0" />
        ) : isModule ? (
          <Box className="w-3.5 h-3.5 text-foreground-muted/60 shrink-0" />
        ) : (
          <span
            className={cn(
              'w-[5px] h-[5px] shrink-0',
              node.subtype ? SUBTYPE_DOT[node.subtype] : 'bg-foreground-muted/50',
            )}
          />
        )}
        <span className="flex-1 truncate">{node.module_name}</span>
        {!isModule && !isLighthouse && node.section_count > 0 ? (
          <span className="text-[10px] text-foreground-muted/50 tabular-nums">
            {node.section_count}
          </span>
        ) : null}
        {connected ? (
          <span className="flex items-center gap-1 text-[10px] text-emerald-300">
            <Check className="w-3 h-3" />
          </span>
        ) : null}
      </button>
    </li>
  )
}

function NodeIcon({ variant }: { variant: string }) {
  if (variant === 'lighthouse') {
    return <Lightbulb className="h-4 w-4 text-amber-400 shrink-0" />
  }
  return <Box className="h-4 w-4 text-foreground-muted shrink-0" />
}
