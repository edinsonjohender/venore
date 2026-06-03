// =============================================================================
// OceanCanvas — R3F Canvas + responsive CameraController + mode switching
// =============================================================================
// Grid is large enough that edges are never visible (ocean-style infinite grid).
// R3F handles container resize natively via internal ResizeObserver.
// Uses frameloop="demand" for zero GPU usage when idle.
//
// Modes (keyboard): H = navigate (pan/zoom), N = move-node (drag nodes)
// Camera: saved to backend on interaction end (500ms debounce).

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { Canvas, useThree, useFrame, type ThreeEvent } from '@react-three/fiber'
import { MapControls } from '@react-three/drei'
import { ArrowUpFromLine, ExternalLink, Eye, Lightbulb, Link as LinkIcon, Network, Palette, Pencil, Plus, Scissors, Trash2, Unlink } from 'lucide-react'
import { OceanGrid } from './OceanGrid'
import { OceanLighting } from './OceanLighting'
import { OceanNodes, type LighthouseOption } from './OceanNodes'
import { CAMERA_CONFIG, worldToCell } from './ocean-config'
import { CreateNodeDialog, type CreateNodeKind } from './CreateNodeDialog'
import { RenameNodeDialog } from './RenameNodeDialog'
import { OceanContextMenu, type OceanContextMenuItem } from './OceanContextMenu'
import { PickLighthouseDialog } from './PickLighthouseDialog'
import { ConnectionsDialog } from './ConnectionsDialog'
import { PickIslandColorDialog } from './PickIslandColorDialog'
import { useLighthouseColorsStore } from '@/stores/lighthouseColorsStore'
import { tauriApi, type OceanNodePosition } from '@/lib/tauri'
import { useSelectedNodesStore } from '@/stores/selectedNodesStore'
import { useNodeDragStateStore } from '@/stores/nodeDragStateStore'
import { useNodeFloatingStore } from '@/stores/nodeFloatingStore'
import { useRoverActiveStore } from '@/stores/roverActiveStore'
import {
  getOceanMode,
  setOceanMode,
  subscribeOceanMode,
} from './ocean-mode'
import {
  getIsPanelAnimating,
  subscribePanelAnim,
} from '@/components/workspace/panel-anim-signal'

// -----------------------------------------------------------------------------
// CameraController — MapControls + debounced save to backend
// -----------------------------------------------------------------------------

function CameraController({
  panEnabled,
  projectPath,
}: {
  panEnabled: boolean
  projectPath: string
}) {
  const { camera } = useThree()
  const controlsRef = useRef<any>(null)
  const saveTimeout = useRef<number>(0)

  // Save camera to backend on interaction end (debounced 500ms)
  useEffect(() => {
    const controls = controlsRef.current
    if (!controls) return

    const handleEnd = () => {
      window.clearTimeout(saveTimeout.current)
      saveTimeout.current = window.setTimeout(() => {
        tauriApi.saveOceanCamera({
          project_path: projectPath,
          x: camera.position.x,
          z: camera.position.z,
          zoom: 'zoom' in camera ? (camera as any).zoom : CAMERA_CONFIG.baseZoom,
        }).catch(() => {
          // Silently ignore save failures
        })
      }, 500)
    }

    controls.addEventListener('end', handleEnd)
    return () => {
      controls.removeEventListener('end', handleEnd)
      window.clearTimeout(saveTimeout.current)
    }
  }, [camera, projectPath])

  return (
    <MapControls
      ref={controlsRef}
      enabled
      enablePan={panEnabled}
      enableZoom
      enableRotate={false}
      screenSpacePanning
      minZoom={CAMERA_CONFIG.minZoom}
      maxZoom={CAMERA_CONFIG.maxZoom}
      dampingFactor={CAMERA_CONFIG.dampingFactor}
      makeDefault
    />
  )
}

// -----------------------------------------------------------------------------
// SyncResize — Force canvas to match container size every frame
// -----------------------------------------------------------------------------
// R3F uses ResizeObserver (async, 1-2 frame lag) to detect container size
// changes. During panel animations this causes visible gaps. SyncResize
// reads the container's actual size in useFrame (forces sync reflow) and
// calls gl.setSize() + updates camera projection directly. This makes the
// canvas resize in the SAME frame as the CSS transition, with zero lag.
// Only active when frameloop="always" (during panel animations).

function SyncResize() {
  const prevW = useRef(0)
  const prevH = useRef(0)

  // useFrame only runs when frameloop="always" (during docked panel animations).
  // Reading clientWidth forces a synchronous reflow, giving us the current
  // CSS-transition-interpolated container size for this exact frame.
  useFrame(({ gl, camera, size }) => {
    const parent = gl.domElement.parentElement
    if (!parent) return

    const w = parent.clientWidth
    const h = parent.clientHeight

    if (w === 0 || h === 0) return

    // Skip if size hasn't changed since our last update OR R3F already caught up
    if ((w === prevW.current && h === prevH.current) ||
        (w === size.width && h === size.height)) return

    prevW.current = w
    prevH.current = h

    gl.setSize(w, h)

    if ((camera as any).isOrthographicCamera) {
      (camera as any).left = w / -2
      ;(camera as any).right = w / 2
      ;(camera as any).top = h / 2
      ;(camera as any).bottom = h / -2
      camera.updateProjectionMatrix()
      camera.updateMatrixWorld()
    }
  })

  return null
}

// -----------------------------------------------------------------------------
// OceanCanvas — Main export
// -----------------------------------------------------------------------------

interface OceanCanvasProps {
  projectPath: string
  className?: string
}

type RenameTarget = { nodeId: string; currentName: string } | null
type MenuState = {
  open: boolean
  position: { x: number; y: number } | null
  items: OceanContextMenuItem[]
}
const EMPTY_MENU: MenuState = { open: false, position: null, items: [] }

export function OceanCanvas({ projectPath, className }: OceanCanvasProps) {
  const mode = useSyncExternalStore(subscribeOceanMode, getOceanMode)
  const isPanelAnimating = useSyncExternalStore(subscribePanelAnim, getIsPanelAnimating)
  const isNodeDragging = useNodeDragStateStore((s) => s.isDragging)
  // Drive `frameloop="always"` while any animated decorator is on screen
  // (OverflowHalo's pulse, ScannerRover's sweep). Without this the
  // useFrame ticks freeze unless the user nudges the camera.
  const isRoverActive = useRoverActiveStore((s) => s.isActive)
  const hasAnimatedDecorators = useRoverActiveStore((s) => s.hasAnimatedDecorators)
  const needsContinuousFrame = isPanelAnimating || isRoverActive || hasAnimatedDecorators

  // Knowledge node / lighthouse creation flow — UI only, backend owns validation
  const [reloadKey, setReloadKey] = useState(0)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [targetCell, setTargetCell] = useState<{ col: number; row: number } | null>(null)
  const [pendingKind, setPendingKind] = useState<CreateNodeKind>('node')

  // Track shift so we can disable MapControls while the user is drawing
  // a box-selection rectangle (drei <Select multiple box>).
  const [isShiftPressed, setIsShiftPressed] = useState(false)

  // Right-click context menu + rename dialog + lighthouse picker
  const [menu, setMenu] = useState<MenuState>(EMPTY_MENU)
  const [renameTarget, setRenameTarget] = useState<RenameTarget>(null)
  const [renameOpen, setRenameOpen] = useState(false)
  const [pickLighthouseOpen, setPickLighthouseOpen] = useState(false)
  const [pickLighthouseState, setPickLighthouseState] = useState<{
    nodeId: string
    currentLighthouseId: string | null
    options: LighthouseOption[]
  } | null>(null)

  // Manual-connection dialog source (the node whose connections we're editing)
  const [connectionsOpen, setConnectionsOpen] = useState(false)
  const [connectionsSource, setConnectionsSource] = useState<{
    module_id: string
    module_name: string
    node_variant: string
  } | null>(null)

  // Per-lighthouse color picker dialog
  const [colorDialogOpen, setColorDialogOpen] = useState(false)
  const [colorDialogSource, setColorDialogSource] = useState<{
    lighthouse_id: string
    name: string
  } | null>(null)

  const closeMenu = useCallback(() => setMenu(EMPTY_MENU), [])

  const triggerReload = useCallback(() => setReloadKey((k) => k + 1), [])

  // Keyboard shortcuts + shift tracking (for box-select)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Shift') {
        setIsShiftPressed(true)
        return
      }
      if (e.repeat) return
      const el = e.target as HTMLElement
      const tag = el.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable) return

      switch (e.key.toLowerCase()) {
        case 'h': setOceanMode('navigate'); break
        case 'n': setOceanMode('move-node'); break
        case 'escape': useSelectedNodesStore.getState().clear(); break
      }
    }
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'Shift') setIsShiftPressed(false)
    }
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', onKeyUp)
    }
  }, [])

  // Click on the empty floor (without modifier) clears the selection — only
  // meaningful in move-node mode where selection exists. In navigate mode
  // (hand) clicks on the floor are no-ops; the canvas is just for panning.
  const handleFloorClick = useCallback((event: ThreeEvent<MouseEvent>) => {
    if (mode !== 'move-node') return
    if (event.nativeEvent.ctrlKey || event.nativeEvent.metaKey) return
    if (event.nativeEvent.shiftKey) return
    useSelectedNodesStore.getState().clear()
  }, [mode])

  // Double-click on the floor → translate hit point to grid cell, open dialog
  // for a regular node (the quick-create path).
  // Pure UI translation: backend will reject if the cell turns out to be occupied.
  const handleFloorDoubleClick = useCallback((event: ThreeEvent<MouseEvent>) => {
    event.stopPropagation()
    const point = event.point
    const { col, row } = worldToCell(point.x, point.z)
    setTargetCell({ col, row })
    setPendingKind('node')
    setDialogOpen(true)
  }, [])

  const handleConfirmCreate = useCallback(async (name: string) => {
    if (!targetCell) return
    setDialogOpen(false)
    try {
      const result =
        pendingKind === 'lighthouse'
          ? await tauriApi.createLighthouse({
              project_path: projectPath,
              name,
              col: targetCell.col,
              row: targetCell.row,
            })
          : await tauriApi.createKnowledgeNode({
              project_path: projectPath,
              name,
              col: targetCell.col,
              row: targetCell.row,
            })
      if (result.accepted) {
        triggerReload()
      } else {
        console.warn(`${pendingKind} creation rejected:`, result.reason)
      }
    } catch (err) {
      console.error(`Failed to create ${pendingKind}:`, err)
    }
  }, [projectPath, targetCell, pendingKind, triggerReload])

  // Right-click on the floor → menu with "Create node here" + "Create lighthouse here".
  const handleFloorContextMenu = useCallback((event: ThreeEvent<MouseEvent>) => {
    event.stopPropagation()
    event.nativeEvent.preventDefault()
    const point = event.point
    const { col, row } = worldToCell(point.x, point.z)
    const screenX = event.nativeEvent.clientX
    const screenY = event.nativeEvent.clientY
    setMenu({
      open: true,
      position: { x: screenX, y: screenY },
      items: [
        {
          id: 'create-node-here',
          label: 'Create node here',
          icon: <Plus className="h-4 w-4" />,
          onSelect: () => {
            setTargetCell({ col, row })
            setPendingKind('node')
            setDialogOpen(true)
          },
        },
        {
          id: 'create-lighthouse-here',
          label: 'Create lighthouse here',
          icon: <Lightbulb className="h-4 w-4" />,
          onSelect: () => {
            setTargetCell({ col, row })
            setPendingKind('lighthouse')
            setDialogOpen(true)
          },
        },
      ],
    })
  }, [])

  // Right-click on a node → build the menu items based on the node variant.
  const handleNodeContextMenu = useCallback(
    (node: OceanNodePosition, event: ThreeEvent<MouseEvent>, lighthouses: LighthouseOption[]) => {
      event.stopPropagation()
      event.nativeEvent.preventDefault()
      const screenX = event.nativeEvent.clientX
      const screenY = event.nativeEvent.clientY
      const items: OceanContextMenuItem[] = []
      const nodeId = node.module_id

      // Open actions — same for every variant. The floating detail panel is
      // multi-instance, so opening N times stacks N panels. Pop-out spawns
      // a separate OS window with the same content.
      items.push({
        id: 'open',
        label: 'Abrir',
        icon: <Eye className="h-4 w-4" />,
        onSelect: () => {
          useNodeFloatingStore.getState().openPanel({
            projectPath,
            moduleId: node.module_id,
            moduleName: node.module_name,
            modulePath: node.module_path,
            nodeVariant: node.node_variant,
          })
        },
      })
      items.push({
        id: 'open-popout',
        label: 'Abrir en ventana emergente',
        icon: <ExternalLink className="h-4 w-4" />,
        onSelect: async () => {
          try {
            await tauriApi.openNodeWindow(
              projectPath,
              node.module_id,
              node.module_name,
              node.node_variant,
            )
          } catch (err) {
            console.error('Failed to open node window:', err)
          }
        },
      })
      items.push({
        id: 'manage-connections',
        label: 'Conectar con...',
        icon: <Network className="h-4 w-4" />,
        onSelect: () => {
          setConnectionsSource({
            module_id: node.module_id,
            module_name: node.module_name,
            node_variant: node.node_variant,
          })
          setConnectionsOpen(true)
        },
      })

      if (node.node_variant === 'lighthouse') {
        items.push({
          id: 'rename-lighthouse',
          label: 'Renombrar isla',
          icon: <Pencil className="h-4 w-4" />,
          onSelect: () => {
            setRenameTarget({ nodeId, currentName: node.module_name })
            setRenameOpen(true)
          },
        })
        items.push({
          id: 'change-color',
          label: 'Cambiar color...',
          icon: <Palette className="h-4 w-4" />,
          onSelect: () => {
            setColorDialogSource({ lighthouse_id: nodeId, name: node.module_name })
            setColorDialogOpen(true)
          },
        })
        items.push({
          id: 'dissolve',
          label: 'Disolver isla',
          icon: <Scissors className="h-4 w-4" />,
          onSelect: async () => {
            try {
              const result = await tauriApi.dissolveLighthouse({
                project_path: projectPath,
                lighthouse_id: nodeId,
              })
              if (result.ok) triggerReload()
              else console.warn('Dissolve failed: lighthouse not found')
            } catch (err) {
              console.error('Failed to dissolve lighthouse:', err)
            }
          },
        })
        items.push({
          id: 'delete-cluster',
          label: 'Borrar isla y todos sus nodos',
          icon: <Trash2 className="h-4 w-4" />,
          danger: true,
          onSelect: async () => {
            try {
              const result = await tauriApi.deleteLighthouseCluster({
                project_path: projectPath,
                lighthouse_id: nodeId,
              })
              if (result.ok) triggerReload()
              else console.warn('Delete cluster failed: lighthouse not found')
            } catch (err) {
              console.error('Failed to delete lighthouse cluster:', err)
            }
          },
        })
      } else {
        items.push({
          id: 'rename',
          label: 'Renombrar',
          icon: <Pencil className="h-4 w-4" />,
          onSelect: () => {
            setRenameTarget({ nodeId, currentName: node.module_name })
            setRenameOpen(true)
          },
        })
        items.push({
          id: 'move-to-lighthouse',
          label: node.lighthouse_id ? 'Cambiar de isla...' : 'Mover a una isla...',
          icon: <LinkIcon className="h-4 w-4" />,
          disabled: lighthouses.length === 0 && !node.lighthouse_id,
          onSelect: () => {
            setPickLighthouseState({
              nodeId,
              currentLighthouseId: node.lighthouse_id,
              options: lighthouses,
            })
            setPickLighthouseOpen(true)
          },
        })
        if (node.node_variant === 'knowledge_node') {
          items.push({
            id: 'promote-lighthouse',
            label: 'Promover a faro',
            icon: <ArrowUpFromLine className="h-4 w-4" />,
            onSelect: async () => {
              try {
                const result = await tauriApi.promoteToLighthouse({
                  project_path: projectPath,
                  node_id: nodeId,
                })
                if (result.accepted) triggerReload()
                else console.warn('Promote rejected:', result.reason)
              } catch (err) {
                console.error('Failed to promote node:', err)
              }
            },
          })
        }
        if (node.lighthouse_id) {
          items.push({
            id: 'detach',
            label: 'Sacar de la isla',
            icon: <Unlink className="h-4 w-4" />,
            onSelect: async () => {
              try {
                const result = await tauriApi.setNodeLighthouse({
                  project_path: projectPath,
                  node_id: nodeId,
                  lighthouse_id: null,
                })
                if (result.accepted) triggerReload()
                else console.warn('Detach rejected:', result.reason)
              } catch (err) {
                console.error('Failed to detach node:', err)
              }
            },
          })
        }
        items.push({
          id: 'delete',
          label: 'Borrar nodo',
          icon: <Trash2 className="h-4 w-4" />,
          danger: true,
          onSelect: async () => {
            try {
              const result = await tauriApi.deleteOceanNode({
                project_path: projectPath,
                node_id: nodeId,
              })
              if (result.ok) triggerReload()
              else console.warn('Delete failed: node not found')
            } catch (err) {
              console.error('Failed to delete node:', err)
            }
          },
        })
      }

      setMenu({ open: true, position: { x: screenX, y: screenY }, items })
    },
    [projectPath, triggerReload],
  )

  const handleConfirmPickLighthouse = useCallback(
    async (lighthouseId: string | null) => {
      if (!pickLighthouseState) return
      setPickLighthouseOpen(false)
      try {
        const result = await tauriApi.setNodeLighthouse({
          project_path: projectPath,
          node_id: pickLighthouseState.nodeId,
          lighthouse_id: lighthouseId,
        })
        if (result.accepted) triggerReload()
        else console.warn('Set lighthouse rejected:', result.reason)
      } catch (err) {
        console.error('Failed to set node lighthouse:', err)
      }
    },
    [projectPath, pickLighthouseState, triggerReload],
  )

  const handleConfirmRename = useCallback(async (newName: string) => {
    if (!renameTarget) return
    setRenameOpen(false)
    try {
      const result = await tauriApi.renameOceanNode({
        project_path: projectPath,
        node_id: renameTarget.nodeId,
        new_name: newName,
      })
      if (result.ok) triggerReload()
      else console.warn('Rename failed: node not found')
    } catch (err) {
      console.error('Failed to rename node:', err)
    }
  }, [projectPath, renameTarget, triggerReload])

  return (
    <div className={className}>
      <Canvas
        orthographic
        frameloop={needsContinuousFrame ? 'always' : 'demand'}
        camera={{
          position: CAMERA_CONFIG.position,
          near: CAMERA_CONFIG.near,
          far: CAMERA_CONFIG.far,
          zoom: CAMERA_CONFIG.baseZoom,
        }}
        gl={{ antialias: true, alpha: true, powerPreference: 'low-power' }}
        style={{ background: 'transparent' }}
      >
        <SyncResize />
        <CameraController
          panEnabled={mode === 'navigate' && !isShiftPressed && !isNodeDragging}
          projectPath={projectPath}
        />
        <OceanLighting />
        <OceanGrid
          onFloorDoubleClick={handleFloorDoubleClick}
          onFloorContextMenu={handleFloorContextMenu}
          onFloorClick={handleFloorClick}
        />
        <OceanNodes
          projectPath={projectPath}
          reloadKey={reloadKey}
          onNodeContextMenu={handleNodeContextMenu}
        />
      </Canvas>
      <CreateNodeDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        targetCell={targetCell}
        kind={pendingKind}
        onConfirm={handleConfirmCreate}
      />
      <RenameNodeDialog
        open={renameOpen}
        onOpenChange={setRenameOpen}
        currentName={renameTarget?.currentName ?? ''}
        onConfirm={handleConfirmRename}
      />
      <OceanContextMenu
        open={menu.open}
        position={menu.position}
        items={menu.items}
        onClose={closeMenu}
      />
      <PickLighthouseDialog
        open={pickLighthouseOpen}
        onOpenChange={setPickLighthouseOpen}
        lighthouses={pickLighthouseState?.options ?? []}
        currentLighthouseId={pickLighthouseState?.currentLighthouseId ?? null}
        onPick={handleConfirmPickLighthouse}
      />
      <ConnectionsDialog
        open={connectionsOpen}
        onOpenChange={setConnectionsOpen}
        source={connectionsSource}
        projectPath={projectPath}
      />
      <ColorDialogPortal
        open={colorDialogOpen}
        onOpenChange={setColorDialogOpen}
        source={colorDialogSource}
        projectPath={projectPath}
      />
    </div>
  )
}

// Wraps PickIslandColorDialog with the live override map so the dialog can
// highlight the current swatch + know whether to enable "Por defecto".
function ColorDialogPortal({
  open,
  onOpenChange,
  source,
  projectPath,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  source: { lighthouse_id: string; name: string } | null
  projectPath: string
}) {
  const overrides = useLighthouseColorsStore((s) => s.overrides)
  const currentColor = source ? (overrides[source.lighthouse_id] ?? null) : null
  const hasOverride = source ? overrides[source.lighthouse_id] != null : false
  return (
    <PickIslandColorDialog
      open={open}
      onOpenChange={onOpenChange}
      source={source}
      currentColor={currentColor}
      hasOverride={hasOverride}
      projectPath={projectPath}
    />
  )
}
