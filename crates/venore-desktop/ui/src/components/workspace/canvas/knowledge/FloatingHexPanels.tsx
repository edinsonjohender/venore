// =============================================================================
// FloatingHexPanels — Renders all open hexagon floating panels with cascade
// =============================================================================
// Same pattern as FloatingNodePanels.

import { useHexFloatingStore } from '@/stores/hexFloatingStore'
import { FloatingHexPanel } from './FloatingHexPanel'

interface FloatingHexPanelsProps {
  boundsRef: React.RefObject<HTMLDivElement | null>
  /** Active knowledge project path — needed so hex panels can build their
   *  AI-connection target (the resolver needs the project to find the
   *  ocean/feature DB). Empty string is acceptable as a noop fallback. */
  projectPath: string
}

const CASCADE_BASE = { x: 80, y: 60 }
const CASCADE_OFFSET = 30

function getCascadePosition(index: number) {
  const wrapped = index % 8
  return {
    x: CASCADE_BASE.x + wrapped * CASCADE_OFFSET,
    y: CASCADE_BASE.y + wrapped * CASCADE_OFFSET,
  }
}

export function FloatingHexPanels({ boundsRef, projectPath }: FloatingHexPanelsProps) {
  const panels = useHexFloatingStore((s) => s.panels)

  return (
    <>
      {panels.map((instance, index) => (
        <FloatingHexPanel
          key={instance.panelId}
          panelId={instance.panelId}
          data={instance.data}
          zIndex={instance.zIndex}
          boundsRef={boundsRef}
          initialPosition={getCascadePosition(index)}
          projectPath={projectPath}
        />
      ))}
    </>
  )
}
