// =============================================================================
// FloatingNodePanels — Renders all open node floating panels with cascade
// =============================================================================

import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useNodeFloatingStore, type NodePanelData } from '@/stores/nodeFloatingStore'
import { useNodePopoutStore } from '@/stores/nodePopoutStore'
import { useAIConnectionStore } from '@/stores/aiConnectionStore'
import { focusNodePanel } from '@/stores/openNodes'
import type { AiWriteProposedEvent } from '@/lib/tauri'
import { FloatingNodePanel } from './FloatingNodePanel'

interface FloatingNodePanelsProps {
  canvasZoneRef: React.RefObject<HTMLDivElement | null>
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

export function FloatingNodePanels({ canvasZoneRef }: FloatingNodePanelsProps) {
  const panels = useNodeFloatingStore((s) => s.panels)

  // Esc closes the topmost (highest zIndex) logbook. Skipped when the
  // user is mid-typing inside an input/textarea/contentEditable so the
  // shortcut never steals focus from the markdown editor or rename fields.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      const target = e.target as HTMLElement | null
      if (target) {
        const tag = target.tagName
        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable) {
          return
        }
      }
      const list = useNodeFloatingStore.getState().panels
      if (list.length === 0) return
      const topmost = list.reduce((a, b) => (b.zIndex > a.zIndex ? b : a), list[0])
      useNodeFloatingStore.getState().closePanel(topmost.panelId)
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  // Pop-out → in-app: the OS window emits this when the user clicks the
  // "dock back" action. We re-open the logbook as a floating panel; the
  // pop-out is already closing itself. Drop it from the popout store so the
  // canvas overlay doesn't keep painting "IN USE" alongside the
  // freshly-restored floating panel.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<NodePanelData>('node-popout-dock', (event) => {
      if (cancelled) return
      useNodeFloatingStore.getState().openPanel(event.payload)
      useNodePopoutStore.getState().remove(event.payload.moduleId)
    })
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [])

  // Pop-out closed (X / alt-F4 / programmatic close that didn't go through
  // the dock-back flow) — drop it from the popout store so the "EN USO"
  // perimeter clears on the canvas.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<{ projectPath: string; moduleId: string }>(
      'node-popout-closed',
      (event) => {
        if (cancelled) return
        useNodePopoutStore.getState().remove(event.payload.moduleId)
        // The popout owned the AI connection while it was open; with the
        // OS window gone there is no source for the line anywhere, so drop
        // the registry entry to match the "close = disconnect" UX.
        useAIConnectionStore
          .getState()
          .unregisterConnection(`node:${event.payload.moduleId}`)
      },
    )
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [])

  // AI proposed a logbook write — surface the target node's panel so the
  // user lands on the diff/preview without hunting for it. Routed through
  // `focusNodePanel` so a node that's currently popped out doesn't get a
  // duplicate floating panel: the OS window is focused instead. Initial
  // proposals carry node_name + variant; re-emits on discard/regenerate
  // omit them and we skip the call because the panel is already mounted.
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<AiWriteProposedEvent>('ai-write-proposed', (event) => {
      if (cancelled) return
      const p = event.payload
      if (!p.node_name) return
      focusNodePanel({
        projectPath: p.project_path,
        moduleId: p.node_id,
        moduleName: p.node_name,
        modulePath: p.module_path ?? '',
        nodeVariant: p.node_variant ?? 'knowledge_node',
      })
    })
    return () => {
      cancelled = true
      unlisten.then((fn) => fn())
    }
  }, [])

  return (
    <>
      {panels.map((instance, index) => (
        <FloatingNodePanel
          key={instance.panelId}
          panelId={instance.panelId}
          data={instance.data}
          zIndex={instance.zIndex}
          canvasZoneRef={canvasZoneRef}
          initialPosition={getCascadePosition(index)}
        />
      ))}
    </>
  )
}
