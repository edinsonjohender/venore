// =============================================================================
// ResearchGraph — Honeycomb SVG (scroll vertical, fill width)
// =============================================================================
// Click on a hexagon opens a floating panel via hexFloatingStore.

import { useMemo } from 'react'
import { HexNode } from './HexNode'
import { computeLayout, hexPoints, HEX_R } from './hex-layout'
import { PHASE_COLORS, PHASE_LABELS } from './hex-colors'
import { useHexFloatingStore } from '@/stores/hexFloatingStore'
import { useShallow } from 'zustand/react/shallow'
import type { Feature, Hexagon } from './mock-data'

interface ResearchGraphProps {
  feature: Feature
}

export function ResearchGraph({ feature }: ResearchGraphProps) {
  const { hexagons, evidence } = feature
  const openPanel = useHexFloatingStore((s) => s.openPanel)
  const openPanelIds = useHexFloatingStore(
    useShallow((s) => new Set(s.panels.map((p) => p.data.hex.id))),
  )

  const layout = useMemo(() => computeLayout(hexagons), [hexagons])
  const hexMap = useMemo(() => {
    const m = new Map<string, Hexagon>()
    for (const h of hexagons) m.set(h.id, h)
    return m
  }, [hexagons])

  const viewBox = `0 0 ${layout.width} ${layout.height}`

  const handleHexClick = (hex: Hexagon) => {
    const hexEvidence = evidence.filter((ev) => ev.hexagonId === hex.id)
    openPanel({ hex, evidence: hexEvidence })
  }

  return (
    <div className="flex-1 overflow-y-auto overflow-x-hidden min-h-0">
      <svg
        viewBox={viewBox}
        className="w-full"
        style={{ height: 'auto', display: 'block' }}
        preserveAspectRatio="xMinYMin meet"
      >
        {/* Phase bands */}
        {layout.bands.map((band) => {
          const tabW = 2.8
          const tabX = 0.2
          const tabY = band.y
          const tabH = band.height
          const color = PHASE_COLORS[band.phase]
          const labelX = tabX + tabW / 2
          const labelY = tabY + tabH / 2

          return (
            <g key={band.phase}>
              <rect
                x={tabX}
                y={tabY}
                width={tabW}
                height={tabH}
                fill={color}
                fillOpacity={0.25}
              />
              <text
                x={labelX}
                y={labelY}
                textAnchor="middle"
                dominantBaseline="central"
                fill={color}
                fontSize={0.7}
                fontWeight={700}
                letterSpacing={0.2}
                transform={`rotate(-90, ${labelX}, ${labelY})`}
              >
                {PHASE_LABELS[band.phase].toUpperCase()}
              </text>
            </g>
          )
        })}

        {/* Hex nodes (ghost + data) */}
        {layout.nodes.map((node) => {
          if (node.ghost) {
            return (
              <polygon
                key={node.id}
                points={hexPoints(node.cx, node.cy, HEX_R)}
                fill="white"
                fillOpacity={0.04}
              />
            )
          }
          const hex = hexMap.get(node.id)
          if (!hex) return null
          return (
            <HexNode
              key={node.id}
              hex={hex}
              cx={node.cx}
              cy={node.cy}
              isSelected={openPanelIds.has(hex.id)}
              onClick={() => handleHexClick(hex)}
            />
          )
        })}
      </svg>
    </div>
  )
}
