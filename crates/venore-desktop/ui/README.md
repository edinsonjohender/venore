# Venore Desktop UI

React + TypeScript frontend for the Venore Tauri desktop application.

---

## Tech stack

- **React 18** — UI framework
- **TypeScript** — type safety
- **Vite** — build tool (hot reload)
- **Tailwind CSS** — utility-first styling
- **shadcn/ui** — component system (customized with Venore palette)
- **Radix UI** — headless UI primitives
- **Lucide React** — icons

---

## Initial setup

### 1. Install dependencies

```bash
pnpm install
```

> pnpm is required; do not use npm or yarn.

### 2. Run in development

```bash
# Option A: frontend only (http://localhost:5173)
pnpm run dev

# Option B: full Tauri shell (recommended)
cd ..
cargo tauri dev
```

---

## Design system

**Important:** read [`DESIGN_SYSTEM.md`](./src/DESIGN_SYSTEM.md) before creating any UI component.

### Mandatory rules

1. **Always** use components from `@/components/ui/`.
2. **Never** create custom buttons, inputs, or cards.
3. **Always** use Venore color tokens (`bg-brand`, `text-foreground`, etc.).
4. **Never** use raw Tailwind colors (`bg-green-500`, etc.).

---

## Available components

### UI primitives (customized shadcn)

```tsx
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
```

Full examples in [`DESIGN_SYSTEM.md`](./src/DESIGN_SYSTEM.md).

---

## Adding a new shadcn component

If you need a new component (Dialog, Dropdown, Tabs, etc.):

```bash
npx shadcn@latest add dialog
npx shadcn@latest add dropdown-menu
npx shadcn@latest add tabs
```

This will:
1. Download the component into `src/components/ui/`.
2. Apply the Venore palette automatically (via `tailwind.config.ts`).
3. Make it ready for use.

Full catalog: https://ui.shadcn.com/docs/components

---

## Layout

```
ui/
├── src/
│   ├── components/
│   │   ├── ui/              ← shadcn components (do not edit)
│   │   │   ├── button.tsx
│   │   │   ├── input.tsx
│   │   │   └── ...
│   │   │
│   │   └── features/        ← business components
│   │       ├── project/
│   │       ├── canvas/
│   │       └── workspace/
│   │
│   ├── lib/
│   │   ├── utils.ts         ← cn() utility
│   │   └── tauri.ts         ← Tauri API wrapper
│   │
│   ├── index.css            ← CSS variables (Venore palette)
│   ├── App.tsx              ← entry point
│   ├── main.tsx
│   └── DESIGN_SYSTEM.md     ← read this first
│
├── components.json          ← shadcn config
├── tailwind.config.ts       ← Tailwind + Venore palette
└── package.json
```

---

## Venore colors

### From JSX

```tsx
<div className="bg-brand text-background">
  Brand color button
</div>

<div className="bg-background-secondary text-foreground">
  Card background
</div>

<p className="text-foreground-muted">
  Secondary text
</p>
```

### From CSS

```css
.my-component {
  background-color: hsl(var(--brand));
  color: hsl(var(--foreground));
  border-color: hsl(var(--border));
}
```

### Full palette

See `src/index.css`:
- `--brand`, `--brand-hover`, `--brand-muted`
- `--background`, `--background-secondary`, `--background-tertiary`
- `--foreground`, `--foreground-muted`, `--foreground-subtle`
- `--border`, `--border-hover`
- `--semantic-success`, `--semantic-warning`, `--semantic-error`, `--semantic-info`

---

## Utilities

### `cn()` — merge classes

```tsx
import { cn } from "@/lib/utils"

<div className={cn(
  "base-classes",
  isActive && "active-classes",
  className,
)}>
  Content
</div>
```

### Tauri API

```tsx
import { tauriApi } from "@/lib/tauri"

const result = await tauriApi.analyzeProject({ path: "/path" })
const projects = await tauriApi.listProjects()
```

---

## Scripts

```bash
pnpm run dev          # Frontend dev server (http://localhost:5173)
pnpm run build        # Production build
pnpm run lint         # ESLint
pnpm run typecheck    # TypeScript check
pnpm run preview      # Preview the production build
```

---

## Recommended workflow

### Creating a new feature

1. **Create a folder under `features/`**
   ```
   src/components/features/my-feature/
   ├── components/
   │   ├── MyFeature.tsx
   │   └── MyFeatureItem.tsx
   ├── hooks/
   │   └── useMyFeature.ts
   └── types.ts
   ```

2. **Use only `ui/` primitives**
   ```tsx
   // Good
   import { Button } from "@/components/ui/button"
   <Button variant="default">Click</Button>

   // Forbidden
   <button className="px-4 py-2 bg-green-500">Click</button>
   ```

3. **Use the Tauri API for backend calls**
   ```tsx
   import { tauriApi } from "@/lib/tauri"

   async function handleAnalyze() {
     const result = await tauriApi.analyzeProject({ path })
     // ...
   }
   ```

---

## Troubleshooting

### Path alias `@/components/...` does not resolve

Check:
1. `tsconfig.json` includes `"@/*": ["./src/*"]`.
2. `vite.config.ts` declares the same alias.
3. Restart the TS server in your editor.

### Colors do not render

1. Ensure `index.css` is imported from `main.tsx`.
2. Verify the CSS variables in `src/index.css`.
3. Confirm the colors are mapped in `tailwind.config.ts`.

### Component is unstyled

1. Confirm the import comes from `@/components/ui/...`.
2. Check the file exists under `src/components/ui/`.
3. Restart the dev server.

---

## References

- [Design System](./src/DESIGN_SYSTEM.md) — **read first**
- [shadcn/ui docs](https://ui.shadcn.com/)
- [Tailwind CSS docs](https://tailwindcss.com/docs)
- [Tauri docs](https://tauri.app/)

---

**Reminder:** read [`DESIGN_SYSTEM.md`](./src/DESIGN_SYSTEM.md) before writing any UI.
