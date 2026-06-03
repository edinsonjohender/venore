# Ocean Canvas — Architecture Notes

## Core Principle: Backend is the Single Source of Truth

**ALL logic lives in the backend. The frontend only renders and sends user intents.**

The backend (venore-core) decides:
- Where nodes are positioned on the grid
- Which moves are valid (occupancy, permissions, constraints)
- When and how nodes are generated from project analysis
- The complete state of the ocean at any point in time

The frontend (this directory) only:
- Renders the 3D scene from backend state
- Captures user interactions (drag, click, keyboard)
- Sends intents to the backend ("user wants to move node X to cell 3,5")
- Applies the backend response (new authoritative state)

## Why This Matters

This app will migrate to SaaS mode — real-time sync across devices, like a
multiplayer game. The backend must own all state so that:

1. Multiple clients see the same ocean state
2. Conflict resolution happens server-side
3. Validation is never bypassed by a client
4. State can be persisted, restored, and replayed

## Current State (Stage 2 — Feb 2026)

The frontend currently has **temporary scaffolding** for testing:
- `OceanTestNodes.tsx` — hardcoded test data + local occupancy check
- `OceanNode.tsx` — snap-to-cell calculation
- `ocean-config.ts` — `snapToCell()` utility

This scaffolding validates the visual and interaction layer (drag, snap, body
block). It will be replaced when the backend provides:

1. **Node generation** — backend analyzes project → produces node list with positions
2. **Position management** — backend owns the grid occupancy map
3. **Move validation** — frontend sends intent, backend accepts/rejects
4. **Real-time sync** — backend pushes state updates via events (Tauri events now, WebSocket/SSE for SaaS)

## Target Flow

```
User drags node → frontend sends intent { nodeId, targetCell }
                → backend validates (occupancy, rules, constraints)
                → backend updates state
                → backend emits event with new node position
                → frontend applies new position (no local logic)
```

## What Stays in the Frontend

- 3D rendering (R3F, Three.js, drei)
- Camera controls (pan, zoom)
- Mode switching (navigate / move-node)
- Visual config (colors, sizes, animations)
- Input capture (pointer events, keyboard)
