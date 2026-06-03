// =============================================================================
// island-utils — Geometric helpers for rendering the territory of an island
// =============================================================================
// Pure functions:
//  - computeIslandTiles(lighthouseId, nodes): MST rooted at the lighthouse,
//    expanded to every cell along each MST edge using Bresenham. Each cell
//    is tagged as 'node' (lighthouse + children) or 'path' (intermediate
//    cells of the MST edges).
//  - islandColor(lighthouseId): deterministic color from a small palette
//    derived from a string hash. Will be replaced by user-chosen colors later.

export interface NodePosition {
  id: string
  col: number
  row: number
  lighthouseId: string | null
}

export interface IslandTile {
  col: number
  row: number
  kind: 'node' | 'path'
}

// -----------------------------------------------------------------------------
// MST (Prim's algorithm) on Manhattan distance, rooted at the lighthouse.
// Each unvisited node attaches to the closest member already in the tree —
// this matches the user's intent: nodes hop through nearer siblings instead
// of always running a straight line back to the lighthouse.
// -----------------------------------------------------------------------------

interface IslandEdge {
  from: { col: number; row: number }
  to: { col: number; row: number }
}

function manhattan(a: { col: number; row: number }, b: { col: number; row: number }): number {
  return Math.abs(a.col - b.col) + Math.abs(a.row - b.row)
}

function buildMST(
  lighthouse: NodePosition,
  children: NodePosition[],
): IslandEdge[] {
  if (children.length === 0) return []

  const inTree: NodePosition[] = [lighthouse]
  const remaining = [...children]
  const edges: IslandEdge[] = []

  while (remaining.length > 0) {
    let bestI = -1
    let bestParent: NodePosition | null = null
    let bestDist = Infinity

    for (let i = 0; i < remaining.length; i++) {
      const candidate = remaining[i]
      for (const treeNode of inTree) {
        const d = manhattan(candidate, treeNode)
        if (d < bestDist) {
          bestDist = d
          bestI = i
          bestParent = treeNode
        }
      }
    }

    if (bestI === -1 || !bestParent) break
    const attached = remaining.splice(bestI, 1)[0]
    inTree.push(attached)
    edges.push({
      from: { col: bestParent.col, row: bestParent.row },
      to: { col: attached.col, row: attached.row },
    })
  }

  return edges
}

// -----------------------------------------------------------------------------
// Manhattan L-path between two cells — moves along one axis first, then the
// other. No diagonal jumps: each consecutive pair of cells shares an edge.
// We pick the order (horizontal-first vs vertical-first) so the longer leg
// runs first, which gives a more natural "trunk + tail" shape when an island
// has many children fanning out from the lighthouse.
// -----------------------------------------------------------------------------

function cellsAlongLine(
  from: { col: number; row: number },
  to: { col: number; row: number },
): { col: number; row: number }[] {
  const cells: { col: number; row: number }[] = []
  let x = from.col
  let y = from.row
  const dx = Math.abs(to.col - x)
  const dy = Math.abs(to.row - y)
  const sx = to.col > x ? 1 : -1
  const sy = to.row > y ? 1 : -1

  cells.push({ col: x, row: y })

  // Walk the dominant axis first
  if (dx >= dy) {
    while (x !== to.col) {
      x += sx
      cells.push({ col: x, row: y })
    }
    while (y !== to.row) {
      y += sy
      cells.push({ col: x, row: y })
    }
  } else {
    while (y !== to.row) {
      y += sy
      cells.push({ col: x, row: y })
    }
    while (x !== to.col) {
      x += sx
      cells.push({ col: x, row: y })
    }
  }

  return cells
}

// -----------------------------------------------------------------------------
// Public API
// -----------------------------------------------------------------------------

/**
 * Returns the deduplicated list of cells the island covers, each tagged as
 * 'node' (a real lighthouse/child node sits there) or 'path' (a Bresenham
 * intermediate cell). Node cells take priority over path cells when both
 * apply to the same coordinate.
 */
export function computeIslandTiles(
  lighthouseId: string,
  nodes: NodePosition[],
): IslandTile[] {
  const lighthouse = nodes.find((n) => n.id === lighthouseId)
  if (!lighthouse) return []
  const children = nodes.filter((n) => n.lighthouseId === lighthouseId && n.id !== lighthouseId)

  const nodeCells = new Set<string>()
  nodeCells.add(`${lighthouse.col},${lighthouse.row}`)
  for (const c of children) nodeCells.add(`${c.col},${c.row}`)

  const edges = buildMST(lighthouse, children)
  const pathCells = new Set<string>()
  for (const e of edges) {
    for (const cell of cellsAlongLine(e.from, e.to)) {
      const key = `${cell.col},${cell.row}`
      if (!nodeCells.has(key)) pathCells.add(key)
    }
  }

  const tiles: IslandTile[] = []
  for (const key of nodeCells) {
    const [c, r] = key.split(',').map(Number)
    tiles.push({ col: c, row: r, kind: 'node' })
  }
  for (const key of pathCells) {
    const [c, r] = key.split(',').map(Number)
    tiles.push({ col: c, row: r, kind: 'path' })
  }
  return tiles
}

// -----------------------------------------------------------------------------
// Color palette — cartoon-flat, vibrant. Default fallback when there's no
// per-lighthouse override; the user-facing palette dialog uses the same set.
// -----------------------------------------------------------------------------

export const ISLAND_PALETTE = [
  '#ef4444', // red
  '#f59e0b', // amber
  '#10b981', // emerald
  '#06b6d4', // cyan
  '#8b5cf6', // violet
  '#ec4899', // pink
  '#84cc16', // lime
  '#f97316', // orange
  '#3b82f6', // blue
  '#a855f7', // purple
  '#14b8a6', // teal
  '#f43f5e', // rose
] as const

/** Deterministic color derived from a lighthouse id — used as the fallback
 *  when no user override exists. */
export function derivedIslandColor(lighthouseId: string): string {
  let hash = 0
  for (let i = 0; i < lighthouseId.length; i++) {
    hash = ((hash << 5) - hash + lighthouseId.charCodeAt(i)) | 0
  }
  return ISLAND_PALETTE[Math.abs(hash) % ISLAND_PALETTE.length]
}

/** Effective color for a lighthouse: explicit override if set, else derived. */
export function islandColor(
  lighthouseId: string,
  overrides?: Record<string, string>,
): string {
  if (overrides && overrides[lighthouseId]) return overrides[lighthouseId]
  return derivedIslandColor(lighthouseId)
}
