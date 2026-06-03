// =============================================================================
// Ocean Canvas — Configuration constants
// =============================================================================

export const GRID_CONFIG = {
  cellSize: 2.5,
  nodeSize: 1.8,
  layerHeight: 0.36,
  layerGap: 0.07,
  oceanGridSize: 400,
} as const

export const GRID_TOTAL_SIZE = GRID_CONFIG.cellSize * GRID_CONFIG.oceanGridSize // 1000
export const GRID_HALF_CELL = GRID_CONFIG.cellSize / 2

/** Snap a world coordinate to the nearest cell center. */
export const snapToCell = (v: number) =>
  Math.round((v - GRID_HALF_CELL) / GRID_CONFIG.cellSize) * GRID_CONFIG.cellSize + GRID_HALF_CELL

/** Convert grid cell (col, row) → world position [x, y, z]. */
export function cellToWorld(col: number, row: number): [number, number, number] {
  return [col * GRID_CONFIG.cellSize + GRID_HALF_CELL, 0, row * GRID_CONFIG.cellSize + GRID_HALF_CELL]
}

/** Convert world position → grid cell (col, row). Inverse of cellToWorld. */
export function worldToCell(x: number, z: number): { col: number; row: number } {
  return {
    col: Math.round((x - GRID_HALF_CELL) / GRID_CONFIG.cellSize),
    row: Math.round((z - GRID_HALF_CELL) / GRID_CONFIG.cellSize),
  }
}

export const OCEAN_COLORS = {
  floorColor: '#0a1628',
  floorOpacity: 0.95,
  gridLineColor: '#1e3a5f',
  gridLineOpacity: 0.3,
} as const

export const CAMERA_CONFIG = {
  position: [30, 40, 30] as [number, number, number],
  near: -1000,
  far: 1000,
  baseZoom: 30,
  minZoom: 10,
  maxZoom: 100,
  dampingFactor: 0.1,
} as const
