// =============================================================================
// WorkspaceCanvas - Central canvas area of the workspace
// =============================================================================
// Hosts the 3D OceanCanvas and layers HTML overlays (toolbar, etc.) on top.
// pointer-events layering: WebGL canvas receives pan/zoom, overlay children
// receive clicks via pointer-events-auto.

import type { ReactNode } from 'react'
import { OceanCanvas } from '@/components/ocean'

interface WorkspaceCanvasProps {
  projectPath: string
  /** Toolbar or other overlays rendered inside the canvas */
  children?: ReactNode
}

export function WorkspaceCanvas({ projectPath, children }: WorkspaceCanvasProps) {
  return (
    <div className="relative flex-1 flex bg-background-secondary overflow-hidden">
      {/* 3D canvas fills container */}
      <OceanCanvas projectPath={projectPath} className="absolute inset-0" />

      {/* HTML overlays on top of WebGL */}
      <div className="absolute inset-0 z-10 pointer-events-none">
        <div className="pointer-events-auto">
          {children}
        </div>
      </div>
    </div>
  )
}
