// =============================================================================
// OceanConnections — Orchestrator for connection lines between ocean nodes
// =============================================================================
// Splits connections by kind:
//   - dependency  → existing bidir dedup + faint white/gray render
//   - manual      → no dedup (each direction is its own arrow), emerald render

import { useMemo } from 'react'
import { ConnectionLine } from './ConnectionLine'
import { GRID_CONFIG, cellToWorld } from './ocean-config'
import { calculateBuoyHeight } from './nodes/BuoyNode'
import { calculateCylinderHeight } from './nodes/CylinderNode'
import type { OceanNodePosition, OceanConnectionDto } from '@/lib/tauri'

const { layerHeight, layerGap } = GRID_CONFIG

/** Small lift above the body's content top so curves don't graze its surface. */
const ANCHOR_LIFT = 0.19

interface OceanConnectionsProps {
  nodes: OceanNodePosition[]
  connections: OceanConnectionDto[]
}

interface NodeWorldPos {
  x: number
  z: number
  height: number
}

/** Y where a connection should anchor on top of `node`. Per-variant so the
 *  bezier sits just above each body's silhouette regardless of geometry. */
function anchorHeight(node: OceanNodePosition): number {
  switch (node.node_variant) {
    case 'buoy':
      return calculateBuoyHeight() + ANCHOR_LIFT
    case 'cylinder':
      return calculateCylinderHeight() + ANCHOR_LIFT
    // module / knowledge_node / lighthouse keep deriving from the layer count
    // (lighthouse has no DTO layers but its current behaviour is preserved on
    // purpose — anchor logic for lighthouse/knowledge is a separate concern).
    case 'module':
    case 'knowledge_node':
    case 'lighthouse':
    default: {
      const topLayerIndex = (node.layers?.length || 1) - 1
      return topLayerIndex * (layerHeight + layerGap) + layerHeight / 2 + ANCHOR_LIFT
    }
  }
}

export function OceanConnections({ nodes, connections }: OceanConnectionsProps) {
  // Map module_id → world position with connection height
  const nodePositions = useMemo(() => {
    const map = new Map<string, NodeWorldPos>()
    for (const node of nodes) {
      const [x, , z] = cellToWorld(node.col, node.row)
      map.set(node.module_id, { x, z, height: anchorHeight(node) })
    }
    return map
  }, [nodes])

  // Dependency connections: count pairs to detect bidirectional, then dedup.
  const dependencyLines = useMemo(() => {
    const deps = connections.filter((c) => c.kind === 'dependency')
    const pairCount = new Map<string, number>()
    for (const conn of deps) {
      const sorted = [conn.from_id, conn.to_id].sort()
      const key = `${sorted[0]}::${sorted[1]}`
      pairCount.set(key, (pairCount.get(key) || 0) + 1)
    }

    const seen = new Set<string>()
    const result: Array<{
      id: string
      fromId: string
      toId: string
      isBidirectional: boolean
    }> = []
    for (const conn of deps) {
      const sorted = [conn.from_id, conn.to_id].sort()
      const key = `${sorted[0]}::${sorted[1]}`
      if (seen.has(key)) continue
      seen.add(key)
      if (!nodePositions.has(conn.from_id) || !nodePositions.has(conn.to_id)) continue
      result.push({
        id: conn.id,
        fromId: conn.from_id,
        toId: conn.to_id,
        isBidirectional: (pairCount.get(key) || 0) >= 2,
      })
    }
    return result
  }, [connections, nodePositions])

  // Manual connections: each renders as its own directed arrow (no dedup).
  const manualLines = useMemo(() => {
    return connections
      .filter((c) => c.kind === 'manual')
      .filter((c) => nodePositions.has(c.from_id) && nodePositions.has(c.to_id))
      .map((c) => ({ id: c.id, fromId: c.from_id, toId: c.to_id }))
  }, [connections, nodePositions])

  if (dependencyLines.length === 0 && manualLines.length === 0) return null

  return (
    <group>
      {dependencyLines.map((conn) => (
        <ConnectionLine
          key={conn.id}
          from={nodePositions.get(conn.fromId)!}
          to={nodePositions.get(conn.toId)!}
          kind="dependency"
          isBidirectional={conn.isBidirectional}
        />
      ))}
      {manualLines.map((conn) => (
        <ConnectionLine
          key={conn.id}
          from={nodePositions.get(conn.fromId)!}
          to={nodePositions.get(conn.toId)!}
          kind="manual"
        />
      ))}
    </group>
  )
}
