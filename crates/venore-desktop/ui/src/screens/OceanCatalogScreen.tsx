// =============================================================================
// OceanCatalogScreen — fixed showcase of every Ocean element type
// =============================================================================
// Read-only canvas with hardcoded mocks of every node variant, subtype,
// connection kind, and island. No project, no wizard, no backend calls — just
// render the existing 3D components against a static dataset so we can see
// what the Ocean *can* contain at a glance.

import { useEffect } from 'react'
import { ArrowLeft } from 'lucide-react'
import { Canvas } from '@react-three/fiber'
import { MapControls, Text } from '@react-three/drei'
import { Button } from '@/components/ui/button'
import { OceanGrid } from '@/components/ocean/OceanGrid'
import { OceanLighting } from '@/components/ocean/OceanLighting'
import { OceanNode } from '@/components/ocean/OceanNode'
import { OceanConnections } from '@/components/ocean/OceanConnections'
import { IslandTiles } from '@/components/ocean/IslandTiles'
import {
  SecurityPerimeter,
  OverflowHalo,
  ScannerRover,
  PresencePin,
} from '@/components/ocean/decorators'
import { CAMERA_CONFIG, GRID_CONFIG, cellToWorld } from '@/components/ocean/ocean-config'
import { useLighthouseColorsStore } from '@/stores/lighthouseColorsStore'
import type {
  OceanNodePosition,
  OceanConnectionDto,
  NodeLayerDto,
  KnowledgeNodeSubtype,
} from '@/lib/tauri'

// -----------------------------------------------------------------------------
// Helpers — build mock OceanNodePosition entries with sensible defaults.
// -----------------------------------------------------------------------------

const FRESH_LAYERS: NodeLayerDto[] = [
  { type: 'context', status: 'complete' },
  { type: 'tests', status: 'complete' },
  { type: 'documentation', status: 'complete' },
  { type: 'connections', status: 'complete' },
  { type: 'status', status: 'complete' },
]

const STALE_LAYERS: NodeLayerDto[] = [
  { type: 'context', status: 'complete' },
  { type: 'tests', status: 'partial' },
  { type: 'documentation', status: 'partial' },
  { type: 'connections', status: 'complete' },
  { type: 'status', status: 'partial' },
]

const MISSING_LAYERS: NodeLayerDto[] = [
  { type: 'context', status: 'missing' },
  { type: 'tests', status: 'missing' },
  { type: 'documentation', status: 'missing' },
  { type: 'connections', status: 'missing' },
  { type: 'status', status: 'missing' },
]

function mockModule(
  id: string,
  name: string,
  col: number,
  row: number,
  layers: NodeLayerDto[],
  status: 'fresh' | 'stale' | 'missing',
): OceanNodePosition {
  return {
    module_id: id,
    module_name: name,
    module_path: '',
    col,
    row,
    user_placed: true,
    layers,
    node_status: status,
    node_variant: 'module',
    lighthouse_id: null,
    section_count: 0,
    subtype: null,
    states: [],
  }
}

function mockKnowledge(
  id: string,
  name: string,
  col: number,
  row: number,
  sectionCount: number,
  subtype: KnowledgeNodeSubtype | null = 'concept',
  lighthouseId: string | null = null,
): OceanNodePosition {
  // Mirror the section count into layers so OceanConnections can attach edges
  // at the right vertical anchor. Production uses synthLayers at render time
  // and leaves the DTO empty; here we precompute so the catalog reads cleanly.
  const layers: NodeLayerDto[] = Array.from(
    { length: Math.max(1, sectionCount) },
    () => ({ type: 'context', status: 'complete' }),
  )
  return {
    module_id: id,
    module_name: name,
    module_path: '',
    col,
    row,
    user_placed: true,
    layers,
    node_status: 'fresh',
    node_variant: 'knowledge_node',
    lighthouse_id: lighthouseId,
    section_count: sectionCount,
    subtype,
    states: [],
  }
}

function mockLighthouse(
  id: string,
  name: string,
  col: number,
  row: number,
): OceanNodePosition {
  return {
    module_id: id,
    module_name: name,
    module_path: '',
    col,
    row,
    user_placed: true,
    layers: [],
    node_status: 'fresh',
    node_variant: 'lighthouse',
    lighthouse_id: null,
    section_count: 0,
    subtype: null,
    states: [],
  }
}

/** Buoy / cylinder are code-representational variants like `module`. They
 *  ignore the DTO `layers` (geometry is fixed) but otherwise flow through the
 *  same pipeline, so the catalog can dispatch them via OceanNode just like the
 *  rest. */
function mockBuoy(id: string, name: string, col: number, row: number): OceanNodePosition {
  return {
    module_id: id,
    module_name: name,
    module_path: '',
    col,
    row,
    user_placed: true,
    layers: [],
    node_status: 'fresh',
    node_variant: 'buoy',
    lighthouse_id: null,
    section_count: 0,
    subtype: null,
    states: [],
  }
}

function mockCylinder(id: string, name: string, col: number, row: number): OceanNodePosition {
  return {
    module_id: id,
    module_name: name,
    module_path: '',
    col,
    row,
    user_placed: true,
    layers: [],
    node_status: 'fresh',
    node_variant: 'cylinder',
    lighthouse_id: null,
    section_count: 0,
    subtype: null,
    states: [],
  }
}

// -----------------------------------------------------------------------------
// Catalog dataset — fixed grid of every element type.
// -----------------------------------------------------------------------------
//
// Sections (one per row band):
//   Row  0  Variantes de nodo (Module ×3, KNode ×2, Lighthouse, Buoy, Cylinder)
//   Row  4  Subtypes del knowledge_node
//   Row  8  Conexiones (manual + dependency)
//   Row 12  Isla con color custom (faro + 2 hijos)
//   Row 20  Hosts bajo los decoradores v1

const ISLAND_FARO_ID = 'demo-faro'
const ISLAND_COLOR = '#ff7e5f'

const CATALOG_NODES: OceanNodePosition[] = [
  // Row 0 — Variants (all go through the OceanNode → BaseNode → body pipeline)
  mockModule('m-fresh', 'Module · fresh', -10, 0, FRESH_LAYERS, 'fresh'),
  mockModule('m-stale', 'Module · stale', -7, 0, STALE_LAYERS, 'stale'),
  mockModule('m-missing', 'Module · missing', -4, 0, MISSING_LAYERS, 'missing'),
  mockKnowledge('k-empty', 'KNode · empty', -1, 0, 0, 'concept'),
  mockKnowledge('k-multi', 'KNode · 4 sections', 2, 0, 4, 'concept'),
  mockLighthouse('l-default', 'Lighthouse', 5, 0),
  mockBuoy('v-buoy', 'Buoy · utils', 8, 0),
  mockCylinder('v-cylinder', 'Cylinder · external services', 11, 0),

  // Row 4 — Subtypes
  mockKnowledge('s-concept', 'Concept', -10, 4, 1, 'concept'),
  mockKnowledge('s-feature', 'Feature', -7, 4, 2, 'feature'),
  mockKnowledge('s-decision', 'Decision', -4, 4, 2, 'decision'),
  mockKnowledge('s-finding', 'Finding', -1, 4, 3, 'finding'),
  mockKnowledge('s-question', 'Question', 2, 4, 1, 'question'),

  // Row 8 — Conexiones
  mockKnowledge('c-manual-from', 'manual ▸ from', -10, 8, 1, 'concept'),
  mockKnowledge('c-manual-to', 'manual ▸ to', -7, 8, 1, 'concept'),
  mockModule('c-dep-from', 'dep ▸ from', 2, 8, FRESH_LAYERS, 'fresh'),
  mockModule('c-dep-to', 'dep ▸ to', 5, 8, FRESH_LAYERS, 'fresh'),

  // Row 12 — Isla con color custom
  mockLighthouse(ISLAND_FARO_ID, 'Faro de la isla', -7, 12),
  mockKnowledge('demo-child-a', 'Hijo A', -4, 12, 2, 'feature', ISLAND_FARO_ID),
  mockKnowledge('demo-child-b', 'Hijo B', -1, 12, 1, 'finding', ISLAND_FARO_ID),

  // Row 20 — Hosts under the v1 decorators (placeholders so the decorator
  // wraps a real-looking module). Each tape variant lands on a host whose
  // status matches its semantic colour:
  //   red ERROR  → missing  · yellow WARNING → stale  · blue EN USO → fresh
  mockModule('host-tape-error',  'host: missing',     -10, 20, MISSING_LAYERS, 'missing'),
  mockModule('host-tape-warn',   'host: stale',       -7,  20, STALE_LAYERS,   'stale'),
  mockModule('host-tape-in-use', 'host: fresh',       -4,  20, FRESH_LAYERS,   'fresh'),
  mockModule('host-ring',        'host: missing',     -1,  20, MISSING_LAYERS, 'missing'),
  mockKnowledge('host-rover',    'host: 3 secciones', 2,   20, 3, 'finding'),
  mockModule('host-presence',    'host: fresh',       5,   20, FRESH_LAYERS,   'fresh'),
]

const CATALOG_CONNECTIONS: OceanConnectionDto[] = [
  { id: 'edge-manual', from_id: 'c-manual-from', to_id: 'c-manual-to', kind: 'manual' },
  { id: 'edge-dep', from_id: 'c-dep-from', to_id: 'c-dep-to', kind: 'dependency' },
]

// Section headers laid flat on the floor (readable from the isometric camera).
const SECTION_HEADERS: Array<{ text: string; col: number; row: number }> = [
  { text: 'Variantes de nodo', col: -3, row: -2 },
  { text: 'Subtypes (knowledge_node)', col: -3, row: 2 },
  { text: 'Conexiones', col: -3, row: 6 },
  { text: 'Isla con color custom', col: -3, row: 10 },
  { text: 'Decoradores experimentales (v1)', col: -3, row: 18 },
]

// Mirror of OceanNode.computeModuleHeight — content height of a stack with
// `layerCount` cubes laid out at GRID_CONFIG.layerHeight + layerGap apart.
function stackHeight(layerCount: number): number {
  const { layerHeight, layerGap } = GRID_CONFIG
  return layerCount * (layerHeight + layerGap) - layerGap
}

// Cells where v1 decorators sit. Most wrap a placeholder module rendered via
// CATALOG_NODES (so they look like a real status overlay). Each carries the
// host's content height so the decorator can scale to actually wrap the node
// instead of staying at v1's default size regardless of host.
//
// SecurityPerimeter is now parameterized — same component, three semantic
// variants by changing color + text. CriticalCage was removed; the red tape
// communicates "critical / error" more clearly with literal text.
const V1_DECORATORS = [
  { kind: 'tape-error'   as const, label: 'SecurityPerimeter',  sublabel: 'rojo · "ERROR"',     col: -10, row: 20, hostHeight: stackHeight(MISSING_LAYERS.length) },
  { kind: 'tape-warn'    as const, label: 'SecurityPerimeter',  sublabel: 'yellow · "WARNING"',  col: -7,  row: 20, hostHeight: stackHeight(STALE_LAYERS.length) },
  { kind: 'tape-in-use'  as const, label: 'SecurityPerimeter',  sublabel: 'blue · "IN USE"',     col: -4,  row: 20, hostHeight: stackHeight(FRESH_LAYERS.length) },
  { kind: 'ring'         as const, label: 'OverflowHalo',       sublabel: 'overflow halo',       col: -1,  row: 20, hostHeight: stackHeight(MISSING_LAYERS.length) },
  { kind: 'rover'        as const, label: 'ScannerRover',       sublabel: 'scanner in orbit',    col: 2,   row: 20, hostHeight: stackHeight(3) },
  { kind: 'presence'     as const, label: 'PresencePin',        sublabel: 'user present',        col: 5,   row: 20, hostHeight: stackHeight(FRESH_LAYERS.length) },
]

// -----------------------------------------------------------------------------
// Component
// -----------------------------------------------------------------------------

interface OceanCatalogScreenProps {
  onBack?: () => void
}

export function OceanCatalogScreen({ onBack }: OceanCatalogScreenProps) {
  // Apply the custom lighthouse color so IslandTiles + LighthouseBody pick it up.
  useEffect(() => {
    const store = useLighthouseColorsStore.getState()
    const previous = store.overrides
    store.setOverrides({ [ISLAND_FARO_ID]: ISLAND_COLOR })
    return () => {
      store.setOverrides(previous)
    }
  }, [])

  const islandNodes = CATALOG_NODES
    .filter((n) => n.module_id === ISLAND_FARO_ID || n.lighthouse_id === ISLAND_FARO_ID)
    .map((n) => ({
      id: n.module_id,
      col: n.col,
      row: n.row,
      lighthouseId: n.lighthouse_id,
    }))

  return (
    <div className="relative h-full w-full bg-background">
      {/* Top bar */}
      <div className="absolute top-0 left-0 right-0 z-10 flex items-center justify-between gap-3 border-b border-border bg-background/80 px-4 py-2 backdrop-blur">
        <div className="flex items-center gap-3">
          {onBack && (
            <Button variant="ghost" size="sm" onClick={onBack} className="gap-2">
              <ArrowLeft className="h-4 w-4" />
              Volver
            </Button>
          )}
          <div>
            <h1 className="text-sm font-semibold text-foreground">Ocean — Catalog of elements</h1>
            <p className="text-xs text-foreground-muted">
              Reference view. {CATALOG_NODES.length} nodes · {CATALOG_CONNECTIONS.length} connections · 1 island · {V1_DECORATORS.length} v1 decorators.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3 text-[11px] text-foreground-muted">
          <Legend swatch="#ffffff20" label="dependency" />
          <Legend swatch="#10b981" label="manual" />
          <Legend swatch={ISLAND_COLOR} label="island" />
        </div>
      </div>

      {/* 3D canvas */}
      <Canvas
        orthographic
        // 'always' instead of 'demand' so the decorator useFrame loops keep
        // ticking even when the camera is idle (SecurityPerimeter scroll,
        // CriticalCage pulse, OverflowHalo wave, ScannerRover scan/travel,
        // PresencePin scale-in). The catalog is a small static scene so the
        // continuous loop is cheap.
        frameloop="always"
        camera={{
          position: CAMERA_CONFIG.position,
          near: CAMERA_CONFIG.near,
          far: CAMERA_CONFIG.far,
          zoom: CAMERA_CONFIG.baseZoom,
        }}
        gl={{ antialias: true, alpha: true, powerPreference: 'low-power' }}
        style={{ background: 'transparent' }}
      >
        <MapControls
          enabled
          enablePan
          enableZoom
          enableRotate={false}
          screenSpacePanning
          minZoom={CAMERA_CONFIG.minZoom}
          maxZoom={CAMERA_CONFIG.maxZoom}
          dampingFactor={CAMERA_CONFIG.dampingFactor}
          makeDefault
        />
        <OceanLighting />
        <OceanGrid />

        {/* Section headers (flat on floor, visible from the isometric angle) */}
        {SECTION_HEADERS.map((h) => {
          const [x, , z] = cellToWorld(h.col, h.row)
          return (
            <Text
              key={h.text}
              position={[x, 0.05, z]}
              rotation={[-Math.PI / 2, 0, 0]}
              fontSize={0.85}
              color="#94a3b8"
              anchorX="center"
              anchorY="middle"
              outlineWidth={0.02}
              outlineColor="#0a1628"
            >
              {h.text}
            </Text>
          )
        })}

        {/* Island territory tiles */}
        <IslandTiles lighthouseId={ISLAND_FARO_ID} nodes={islandNodes} />

        {/* All nodes */}
        {CATALOG_NODES.map((n) => (
          <OceanNode
            key={n.module_id}
            id={n.module_id}
            position={cellToWorld(n.col, n.row)}
            label={n.module_name}
            status={n.node_status as 'fresh' | 'stale' | 'missing' | 'loading'}
            layers={n.node_variant === 'knowledge_node' ? synthLayers(n.section_count) : (n.layers as any)}
            variant={n.node_variant}
          />
        ))}

        {/* Connections */}
        <OceanConnections nodes={CATALOG_NODES} connections={CATALOG_CONNECTIONS} />

        {/* v1 decorators — animated wrappers around (or independent of) a node.
            Each cell has its own group so positions are local to the decorator
            (which assumes origin at 0,0). The host module, when applicable, is
            already rendered above as part of CATALOG_NODES. Sizes are scaled
            to the host's actual content height so the decorator wraps it
            instead of staying at v1's default sizing. */}
        {V1_DECORATORS.map((d) => {
          const [x, , z] = cellToWorld(d.col, d.row)
          const h = d.hostHeight
          // Two tapes spread across each host's height — same prop set for the
          // three tape variants, only color + text change.
          const tapeProps = {
            baseHeight: h * 0.25,
            spacing: Math.max(0.5, h * 0.55),
            tapeCount: 2,
          }
          return (
            <group key={d.kind} position={[x, 0, z]}>
              {d.kind === 'tape-error' && (
                <SecurityPerimeter {...tapeProps} color="#ef4444" text="ERROR" />
              )}
              {d.kind === 'tape-warn' && (
                <SecurityPerimeter {...tapeProps} color="#fbbf24" text="WARNING" />
              )}
              {d.kind === 'tape-in-use' && (
                <SecurityPerimeter {...tapeProps} color="#60a5fa" text="EN USO" />
              )}
              {d.kind === 'ring' && (
                // 4 ropes spread evenly across the host height
                <OverflowHalo
                  baseHeight={h * 0.2}
                  spacing={Math.max(0.4, (h * 0.85) / 3)}
                  ringCount={4}
                />
              )}
              {d.kind === 'rover' && (
                // Focal orb floats above the host's top
                <ScannerRover positions={[{ x: 0, z: 0 }]} height={h + 1.4} />
              )}
              {d.kind === 'presence' && (
                // Pin floats just above the host's top
                <PresencePin
                  initials="EM"
                  color="#01e8a2"
                  level="editing"
                  height={h + 0.6}
                />
              )}
              <Text
                position={[0, 0.05, 1.1]}
                rotation={[-Math.PI / 2, 0, 0]}
                fontSize={0.32}
                color="#cbd5e1"
                anchorX="center"
                outlineWidth={0.015}
                outlineColor="#0a1628"
              >
                {d.label}
              </Text>
              <Text
                position={[0, 0.05, 1.45]}
                rotation={[-Math.PI / 2, 0, 0]}
                fontSize={0.22}
                color="#64748b"
                anchorX="center"
                outlineWidth={0.012}
                outlineColor="#0a1628"
              >
                {d.sublabel}
              </Text>
            </group>
          )
        })}
      </Canvas>
    </div>
  )
}

// Mirrors OceanNodes.synthesizeKnowledgeLayers — knowledge nodes don't carry
// code-derived layers, so the visual stack scales with section count.
function synthLayers(sectionCount: number) {
  const n = Math.max(1, sectionCount)
  return Array.from({ length: n }, () => ({
    type: 'context' as const,
    status: 'complete' as const,
  }))
}

function Legend({ swatch, label }: { swatch: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span
        className="inline-block h-2.5 w-2.5 rounded-sm border border-white/10"
        style={{ background: swatch }}
      />
      {label}
    </span>
  )
}
