// =============================================================================
// AIConnectionLayer - One SVG per connection, each matching its panel's z-index
// =============================================================================
// SVG elements are created once; the RAF loop only updates the `d` attribute
// so CSS animations (stroke-dashoffset flow) keep running uninterrupted.
// Rainbow gradient animated via SVG <animateTransform>.

import { useRef, useEffect, useCallback } from 'react'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'

const NS = 'http://www.w3.org/2000/svg'

// Same colors as .rainbow-border in index.css
const RAINBOW_STOPS = [
  '#01e8a2', '#00ffc6', '#8b5cf6', '#a855f7',
  '#ef4444', '#f97316', '#01e8a2',
]

function getTopCenter(el: Element) {
  const r = el.getBoundingClientRect()
  return { x: r.left + r.width / 2, y: r.top }
}

function getBottomCenter(el: Element) {
  const r = el.getBoundingClientRect()
  return { x: r.left + r.width / 2, y: r.bottom }
}

function getPanelZIndex(el: Element): number {
  const panel = el.closest('.absolute') as HTMLElement | null
  if (panel) {
    const z = parseInt(panel.style.zIndex, 10)
    if (!isNaN(z)) return z
  }
  return 40
}

// -----------------------------------------------------------------------------
// Build SVG children once — returns refs to mutable elements
// -----------------------------------------------------------------------------

const GRAD_SPAN = 200

function createSvgElements(svg: SVGSVGElement) {
  // --- Defs: animated rainbow gradient ---
  const defs = document.createElementNS(NS, 'defs')

  const grad = document.createElementNS(NS, 'linearGradient')
  grad.id = 'rainbow'
  grad.setAttribute('gradientUnits', 'userSpaceOnUse')
  grad.setAttribute('x1', '0')
  grad.setAttribute('y1', '0')
  grad.setAttribute('x2', String(GRAD_SPAN))
  grad.setAttribute('y2', '0')
  grad.setAttribute('spreadMethod', 'repeat')

  RAINBOW_STOPS.forEach((color, i) => {
    const stop = document.createElementNS(NS, 'stop')
    stop.setAttribute('offset', `${(i / (RAINBOW_STOPS.length - 1)) * 100}%`)
    stop.setAttribute('stop-color', color)
    grad.appendChild(stop)
  })

  const anim = document.createElementNS(NS, 'animateTransform')
  anim.setAttribute('attributeName', 'gradientTransform')
  anim.setAttribute('type', 'translate')
  anim.setAttribute('from', '0 0')
  anim.setAttribute('to', String(GRAD_SPAN) + ' 0')
  anim.setAttribute('dur', '4s')
  anim.setAttribute('repeatCount', 'indefinite')
  grad.appendChild(anim)

  defs.appendChild(grad)
  svg.appendChild(defs)

  // --- Main animated dashed path (rainbow) ---
  const line = document.createElementNS(NS, 'path')
  line.setAttribute('fill', 'none')
  line.setAttribute('stroke', 'url(#rainbow)')
  line.setAttribute('stroke-width', '1.5')
  line.setAttribute('stroke-opacity', '0.3')
  line.setAttribute('stroke-linecap', 'round')
  line.setAttribute('stroke-dasharray', '8 4')
  line.classList.add('animate-ai-line-flow')

  // --- Endpoint dots (brand green) ---
  const dotStart = document.createElementNS(NS, 'circle')
  dotStart.setAttribute('r', '3')
  dotStart.setAttribute('fill', '#01e8a2')
  dotStart.setAttribute('fill-opacity', '0.9')

  const dotEnd = document.createElementNS(NS, 'circle')
  dotEnd.setAttribute('r', '3')
  dotEnd.setAttribute('fill', '#01e8a2')
  dotEnd.setAttribute('fill-opacity', '0.9')

  svg.appendChild(line)
  svg.appendChild(dotStart)
  svg.appendChild(dotEnd)

  return { line, dotStart, dotEnd }
}

// -----------------------------------------------------------------------------
// Single connection line — owns its own SVG + RAF loop
// -----------------------------------------------------------------------------

function ConnectionLine({ connectionId }: { connectionId: string }) {
  const svgRef = useRef<SVGSVGElement>(null)
  const rafRef = useRef<number>(0)
  const elsRef = useRef<ReturnType<typeof createSvgElements> | null>(null)

  const draw = useCallback(() => {
    const svg = svgRef.current
    if (!svg) return

    // Create children once
    if (!elsRef.current) {
      elsRef.current = createSvgElements(svg)
    }

    const { line, dotStart, dotEnd } = elsRef.current

    const svgRect = svg.getBoundingClientRect()
    const indicator = document.querySelector('[data-ai-indicator]')
    const el = document.querySelector(`[data-connection-id="${connectionId}"]`)

    if (!indicator || !el) {
      line.setAttribute('d', '')
      rafRef.current = requestAnimationFrame(draw)
      return
    }

    // Match panel z-index
    svg.style.zIndex = String(getPanelZIndex(el))

    // Start: top-center of the Sparkles button
    const sAbs = getTopCenter(el)
    const sx = sAbs.x - svgRect.left
    const sy = sAbs.y - svgRect.top

    // End: bottom-center of the AI indicator
    const eAbs = getBottomCenter(indicator)
    const ex = eAbs.x - svgRect.left
    const ey = eAbs.y - svgRect.top

    // Bézier control points — vertical exit/enter
    const cpLen = Math.abs(sy - ey) * 0.4
    const d = `M ${sx} ${sy} C ${sx} ${sy - cpLen}, ${ex} ${ey + cpLen}, ${ex} ${ey}`

    // Update only attributes — elements stay alive, animations keep running
    line.setAttribute('d', d)
    dotStart.setAttribute('cx', String(sx))
    dotStart.setAttribute('cy', String(sy))
    dotEnd.setAttribute('cx', String(ex))
    dotEnd.setAttribute('cy', String(ey))

    rafRef.current = requestAnimationFrame(draw)
  }, [connectionId])

  useEffect(() => {
    rafRef.current = requestAnimationFrame(draw)
    return () => cancelAnimationFrame(rafRef.current)
  }, [draw])

  return (
    <svg
      ref={svgRef}
      className="absolute inset-0 w-full h-full pointer-events-none"
    />
  )
}

// -----------------------------------------------------------------------------
// Layer — renders one ConnectionLine per active connection
// -----------------------------------------------------------------------------

export function AIConnectionLayer() {
  const connections = useAIConnectionStore((s) => s.connections)
  const activeIds = Object.entries(connections)
    .filter(([, entry]) => entry.active)
    .map(([id]) => id)

  if (activeIds.length === 0) return null

  return (
    <>
      {activeIds.map((id) => (
        <ConnectionLine key={id} connectionId={id} />
      ))}
    </>
  )
}
