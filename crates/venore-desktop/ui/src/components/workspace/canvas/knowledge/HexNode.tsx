// =============================================================================
// HexNode — SVG hexagon component for the research graph
// =============================================================================

import { hexPoints, HEX_R } from './hex-layout'
import { PHASE_COLORS, fillOpacityForPercentage } from './hex-colors'
import type { Hexagon } from './mock-data'

interface HexNodeProps {
  hex: Hexagon
  cx: number
  cy: number
  isSelected: boolean
  onClick: () => void
}

export function HexNode({ hex, cx, cy, isSelected, onClick }: HexNodeProps) {
  const color = hex.isDeadEnd ? PHASE_COLORS['dead-end'] : PHASE_COLORS[hex.phase]
  const fillOpacity = fillOpacityForPercentage(hex.percentage)

  return (
    <g
      onClick={onClick}
      style={{ cursor: 'pointer' }}
    >
      <title>{`${hex.title} — ${hex.percentage}% (${hex.phase})${hex.isDeadEnd ? ' ✕ Dead End' : ''}`}</title>

      {/* Glow ring when selected */}
      {isSelected && (
        <polygon
          points={hexPoints(cx, cy, HEX_R + 0.3)}
          fill="none"
          stroke={color}
          strokeWidth={0.15}
          opacity={0.7}
        />
      )}

      {/* Main hexagon — no stroke */}
      <polygon
        points={hexPoints(cx, cy, HEX_R)}
        fill={color}
        fillOpacity={fillOpacity}
      />

      {/* Dead-end X */}
      {hex.isDeadEnd && (
        <>
          <line x1={cx - 0.6} y1={cy - 0.6} x2={cx + 0.6} y2={cy + 0.6} stroke="#ef4444" strokeWidth={0.15} opacity={0.7} />
          <line x1={cx + 0.6} y1={cy - 0.6} x2={cx - 0.6} y2={cy + 0.6} stroke="#ef4444" strokeWidth={0.15} opacity={0.7} />
        </>
      )}

      {/* Percentage only */}
      <text
        x={cx}
        y={cy}
        textAnchor="middle"
        dominantBaseline="central"
        fill="white"
        fontSize={0.9}
        fontWeight={400}
        opacity={0.8}
      >
        {hex.percentage}%
      </text>
    </g>
  )
}
