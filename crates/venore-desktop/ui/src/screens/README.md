# Screens - Main Application Views

This directory contains the main full-screen views of the Venore application.

## 📱 Screen Flow

```
BootScreen → LauncherScreen → ProjectView
   (Load)      (Select)         (Work)
```

## 🖼️ Screens

### 1. BootScreen

**Purpose**: Initial loading screen during app initialization

**When shown**: First thing user sees when opening the app

**Duration**: ~450ms (checks run sequentially)

**Features**:
- Animated spinner
- Current phase indicator (booting → checking → ready)
- List of checks with status icons:
  - ○ Pending (gray)
  - ⟳ Running (brand color, animated)
  - ✓ Success (brand color)
  - ✗ Failed (error color)
- Duration display for each check
- Total duration on completion
- Error handling with retry option

**States**:
- `booting` - Starting app
- `checking` - Running system checks
- `ready` - All checks passed
- `error` - Startup failed

**Checks performed**:
1. Tauri backend
2. Configuration loading
3. Database initialization

**Callbacks**:
- `onReady()` - Called when boot completes successfully
- `onError(error: string)` - Called if boot fails

---

### 2. LauncherScreen

**Purpose**: Main screen for opening/creating projects

**When shown**: After successful boot, when no project is open

**Features**:

#### Left Panel (Actions):
- Venore logo and branding
- **New Project** button (highlighted)
  - Opens project setup wizard
  - Analyzes project structure
- **Open Existing Project** button
  - Opens folder picker dialog
  - Scans and loads project
- Drag & drop hint
- Version number (bottom left)

#### Right Panel (Recent Projects):
- List of recently opened projects
- Shows:
  - Project name
  - Last opened time (relative: "2 hours ago")
- Actions:
  - Click to open
  - Hover to show delete button
  - "Clear all" button in header
- Empty state when no recent projects

**Storage**:
- Recent projects stored in `localStorage` as `venore-recent-projects`
- Max 10 projects kept
- JSON format: `{ path, name, lastOpened }`

**Drag & Drop**:
- Drop project folder → Opens project
- Drop `.venore` workspace file → Opens workspace
- Visual overlay during drag

**Callbacks**:
- `onProjectOpen(projectPath: string)` - Called when project is opened

**TODO**:
- [ ] Integrate with Tauri folder picker
- [ ] Implement actual project loading
- [ ] Add workspace file support
- [ ] Complete drag & drop with Tauri

---

### 3. ProjectView (Not Implemented Yet)

**Purpose**: Main workspace when project is open

**Features** (planned):
- 3D Canvas with project visualization
- Islands view
- Node details panel
- Context generation
- Search and navigation
- Settings

**When to implement**: After Phase 2-6 of NEXT_STEPS.md are complete

---

## 🎨 Design Guidelines

### Colors
All screens use Venore's design system:
- **Brand**: `#01e8a2` (turquoise)
- **Background**: `#09090b` (dark)
- **Foreground**: `#fafafa` (light text)
- See `DESIGN_SYSTEM.md` for full palette

### Components
Use shadcn/ui components from `@/components/ui/`:
- Button
- Card
- Badge
- Input
- Separator

### Layout
- All screens are full-screen (`h-screen w-screen`)
- Use flexbox for centering and layout
- Responsive gaps and padding

### Animations
- Spinners: `animate-spin`
- Transitions: `transition-colors`, `transition-opacity`
- Hover states on interactive elements

---

## 🔧 Development

### Adding a new screen:

1. Create `NewScreen.tsx` in this directory
2. Export from `index.ts`
3. Add phase to `App.tsx`:
   ```typescript
   type AppPhase = 'boot' | 'launcher' | 'project' | 'newscreen'
   ```
4. Add conditional render in `App.tsx`

### Testing:

To test a screen in isolation:
```typescript
// In App.tsx, change initial phase:
const [phase, setPhase] = useState<AppPhase>('launcher') // Skip boot
```

---

## 📂 File Structure

```
screens/
├── README.md              # This file
├── index.ts              # Exports
├── BootScreen.tsx        # Initial loading
├── LauncherScreen.tsx    # Project selection
└── ProjectView.tsx       # TODO: Active project workspace
```

---

## 🚀 Next Steps

1. **Integrate Tauri APIs**
   - Folder picker for opening projects
   - File system access for drag & drop
   - Project scanning and loading

2. **Implement ProjectView**
   - 3D canvas with Three.js
   - Island visualization
   - Panel system

3. **Add State Management**
   - Consider Zustand for global state
   - Project state
   - UI state (panels, selection)

4. **Persist Settings**
   - User preferences
   - Window size/position
   - Last opened project

---

## 📝 Notes

- Screens are **stateless** where possible
- Communication via callbacks (props)
- No direct Tauri calls in screens (keep them in App.tsx or separate services)
- Each screen is self-contained and can be developed/tested independently
