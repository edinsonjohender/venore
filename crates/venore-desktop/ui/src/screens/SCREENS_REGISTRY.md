# Screens Registry — Venore V2

Complete registry of every screen in the application.

---

## 📊 Summary

| Screen | Description | Status | Phase | File | Props |
|--------|-------------|--------|-------|------|-------|
| BootScreen | Initial loading screen with system checks | ✅ Implemented | boot | `BootScreen.tsx` | `onReady`, `onError` |
| LauncherScreen | Project picker and recent-projects manager | ✅ Implemented | launcher | `LauncherScreen.tsx` | `onProjectOpen` |
| ProjectView | Active workspace with analysis and visualization | ⏳ Pending | project | - | TBD |

---

## 🔍 Per-screen detail

### 1. BootScreen

| Aspect | Detail |
|--------|--------|
| **Purpose** | Initial loading screen during app startup |
| **Status** | ✅ Implemented (mock) |
| **Flow phase** | `boot` (first screen) |
| **File** | `ui/src/screens/BootScreen.tsx` |
| **Lines of code** | ~264 |

#### Props/Interface
```typescript
interface BootScreenProps {
  onReady?: () => void
  onError?: (error: string) => void
}
```

#### Implemented features ✅
- [x] Animated spinner with transitions
- [x] Phase system (booting → checking → ready → error)
- [x] Check list with states (pending, running, success, failed)
- [x] State icons (○ ⟳ ✓ ✗)
- [x] Per-check duration display
- [x] Total duration on completion
- [x] Error handling
- [x] Visual states (spinner, success icon, error icon)

#### TODOs / pending 📝
- [ ] Wire real checks to the Tauri backend
- [ ] Integrate real database verification
- [ ] Add a real configuration check (no mock)
- [ ] Handle per-check specific errors
- [ ] Add retry logic when a check fails
- [ ] Persist boot logs to a file

#### Current checks
1. **Tauri backend** — 150ms (mock)
2. **Configuration** — 100ms (mock)
3. **Database** — 200ms (mock)

#### Design notes
- Background: `bg-background` (#09090b)
- Brand color: `border-t-brand` (#01e8a2)
- Animation: `animate-spin` on the spinner
- Max width: `max-w-md`

---

### 2. LauncherScreen

| Aspect | Detail |
|--------|--------|
| **Purpose** | Main screen to create/open projects and manage recents |
| **Status** | ✅ Implemented (mock — no Tauri integration yet) |
| **Flow phase** | `launcher` (after boot) |
| **File** | `ui/src/screens/LauncherScreen.tsx` |
| **Lines of code** | ~322 |

#### Props/Interface
```typescript
interface LauncherScreenProps {
  onProjectOpen?: (projectPath: string) => void
}

interface RecentProject {
  path: string
  name: string
  lastOpened: number
}
```

#### Implemented features ✅
- [x] 2-pane layout (Xcode-style)
- [x] Left pane: logo + actions
- [x] Right pane: recents list
- [x] "New Project" button
- [x] "Open Project" button
- [x] Recent projects list (max 10)
- [x] Relative timestamps ("2 hours ago")
- [x] Hover-to-show delete buttons
- [x] Clear all recent projects
- [x] Empty-state visual
- [x] Drag & drop visual overlay
- [x] localStorage persistence
- [x] Real Venore logo
- [x] Version display (v2.0.0 Alpha)

#### TODOs / pending 📝
- [ ] Integrate the Tauri folder picker (`invoke('open_folder')`)
- [ ] Implement the new-project wizard
- [ ] Wire opening recent projects through Tauri
- [ ] Implement working drag & drop (with Tauri file APIs)
- [ ] Validate that the project path exists
- [ ] Show project preview/info on hover
- [ ] Add keyboard shortcuts (Ctrl+O, etc.)
- [ ] Add search/filter for recent projects
- [ ] Show last commit or project status
- [ ] Add favorites (pin projects)

#### UI components used
- `Button` (shadcn/ui)
- `Separator` (shadcn/ui)
- Icons: `FolderOpen`, `Clock`, `Trash2`, `Sparkles` (lucide-react)

#### Layout
```
┌─────────────────┬──────────────┐
│                 │   Recent     │
│   Logo          │   Projects   │
│   Actions       │   (sidebar)  │
│   (centered)    │              │
│                 │              │
│   Version       │              │
└─────────────────┴──────────────┘
   flex-1           w-80
```

#### Design notes
- Left pane: `flex-1` with centered content
- Right pane: `w-80` (320px) with `bg-background-secondary`
- Border: `border-r border-border`
- Drag overlay: `z-50` with `backdrop-blur-sm`

---

### 3. ProjectView

| Aspect | Detail |
|--------|--------|
| **Purpose** | Main view of the active project (workspace) |
| **Status** | ⏳ Pending |
| **Flow phase** | `project` (after launcher) |
| **File** | - |
| **Lines of code** | 0 |

#### Props/Interface (proposed)
```typescript
interface ProjectViewProps {
  projectPath: string
  onClose?: () => void
  onError?: (error: string) => void
}
```

#### Planned features 📋
- [ ] File-tree visualization
- [ ] Analysis panel
- [ ] Dependency graph view
- [ ] File search/filter
- [ ] LLM integration
- [ ] Main toolbar
- [ ] Tools sidebar
- [ ] Status bar
- [ ] Multi-pane layout

#### TODOs / pending 📝
- [ ] Design the overall layout
- [ ] Build base components
- [ ] Define the panel system
- [ ] Implement the file tree
- [ ] Wire to backend (Phases 2-6)

#### Depends on:
- ✅ Phase 1: Validation (DONE)
- ⏳ Phase 2: Core Infrastructure
- ⏳ Phase 3: Repository Layer
- ⏳ Phase 4: Domain Logic
- ⏳ Phase 5: Application Layer
- ⏳ Phase 6: Integration

#### Notes
This screen will be implemented **after** completing phases 2-6 of the master plan (NEXT_STEPS.md), since it requires the full backend infrastructure.

---

## 🔄 Navigation flow

```
App Start
    ↓
┌─────────────┐
│ BootScreen  │ (boot phase)
└──────┬──────┘
       │ onReady()
       ↓
┌─────────────────┐
│ LauncherScreen  │ (launcher phase)
└──────┬──────────┘
       │ onProjectOpen(path)
       ↓
┌─────────────┐
│ ProjectView │ (project phase)
└─────────────┘
```

---

## 📐 Design conventions

### Naming
- Suffix: `Screen` for main screens
- PascalCase for components
- Props interface: `{ComponentName}Props`

### Props pattern
Every screen follows the callback pattern:
- `onReady()` — when the screen finishes its task
- `onError(error)` — when an error occurs
- `on{Action}(data)` — for specific actions

### File structure
```
screens/
├── BootScreen.tsx
├── LauncherScreen.tsx
├── ProjectView.tsx (pending)
├── index.ts (exports)
├── README.md (general documentation)
└── SCREENS_REGISTRY.md (this file)
```

### Exports
```typescript
// screens/index.ts
export { BootScreen } from './BootScreen'
export { LauncherScreen } from './LauncherScreen'
export { ProjectView } from './ProjectView'
```

---

## 🎨 Design tokens

### Backgrounds
- `bg-background` — #09090b (main)
- `bg-background-secondary` — for secondary panels
- `bg-background-tertiary` — for hover states

### Colors
- `text-brand` — #01e8a2 (turquoise)
- `text-foreground` — #fafafa (white)
- `text-foreground-muted` — light gray
- `text-semantic-error` — error red

### Spacing
- Screens: `h-screen w-screen`
- Containers: `max-w-md`, `max-w-lg`, etc.
- Gaps: `gap-3`, `gap-4`, `gap-6`, `gap-8`

---

## 📝 Checklist for a new screen

When creating a new screen, make sure to:

- [ ] Create the file under `ui/src/screens/`
- [ ] Define the Props interface
- [ ] Implement callbacks (`onReady`, `onError`, etc.)
- [ ] Add it to `screens/index.ts`
- [ ] Document it in this file (SCREENS_REGISTRY.md)
- [ ] Add it to `App.tsx` if it is a main phase
- [ ] Follow the naming conventions
- [ ] Use design tokens consistently
- [ ] Handle loading/error states
- [ ] Add TODO comments for any pending work

---

## 🔧 Maintenance

**Last updated**: 2026-01-19

**Update this file when**:
- ✅ A new screen ships
- ✅ A TODO is completed
- ✅ New features land in an existing screen
- ✅ The purpose or design of a screen changes
- ✅ A screen is deprecated or removed

**Owner**: keep this up to date with every commit that touches screens.
