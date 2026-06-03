# Venore Design System

**MANDATORY RULES**: This document defines the visual identity of Venore. **Always** follow these rules when creating or modifying UI components.

---

## 🎨 Color palette

### Brand (primary color)
```
brand           #01e8a2   ← Logo color, primary buttons
brand-hover     #00c589   ← Primary button hover
brand-muted     #01e8a2   ← With opacity for subtle backgrounds
```

### Backgrounds (dark theme)
```
background           #09090b   ← Main background
background-secondary #0c0c0e   ← Panels, cards
background-tertiary  #18181b   ← Elevated elements, hovers
```

### Foreground (text)
```
foreground        #fafafa   ← Primary text
foreground-muted  #a1a1aa   ← Secondary text
foreground-subtle #71717a   ← Disabled text, hints
```

### Borders
```
border        #27272a   ← Default borders
border-hover  #3f3f46   ← Borders on hover/focus
```

### Semantic (states)
Only use these four colors for states:
```
semantic-success  #01e8a2   ← Success, confirmations (= brand)
semantic-warning  #f59e0b   ← Warnings, caution
semantic-error    #ef4444   ← Errors, danger
semantic-info     #3b82f6   ← Information, neutral
```

---

## 📦 UI components

**Rule #1**: **Always** use components from `@/components/ui/`.

**Rule #2**: **Never** create custom buttons, inputs, or cards outside of `ui/`.

**Rule #3**: If you need a new component, add it to `ui/` first.

---

## 🔘 Button

### Import
```tsx
import { Button } from "@/components/ui/button"
```

### Variants

#### Primary (default)
```tsx
<Button variant="default">Primary Action</Button>
```
**Use case**: primary action of a screen (save, create, confirm).

#### Secondary
```tsx
<Button variant="secondary">Secondary Action</Button>
```
**Use case**: secondary actions (cancel, back).

#### Ghost
```tsx
<Button variant="ghost">Tertiary Action</Button>
```
**Use case**: tertiary actions, icons, menus.

#### Outline
```tsx
<Button variant="outline">Outlined Action</Button>
```
**Use case**: alternative to secondary, more visible.

#### Destructive
```tsx
<Button variant="destructive">Delete</Button>
```
**Use case**: dangerous actions (delete, undo).

#### Link
```tsx
<Button variant="link">View More</Button>
```
**Use case**: links that look like buttons.

### Sizes
```tsx
<Button size="sm">Small</Button>
<Button size="default">Default</Button>
<Button size="lg">Large</Button>
<Button size="icon"><Icon /></Button>
```

### ❌ Don't
```tsx
// Forbidden — do not create custom buttons
<button className="px-4 py-2 bg-green-500">Custom Button</button>

// Forbidden — do not use inline styles
<button style={{ backgroundColor: '#01e8a2' }}>Button</button>
```

---

## 📝 Input

### Import
```tsx
import { Input } from "@/components/ui/input"
```

### Basic usage
```tsx
<Input placeholder="Enter project path..." />
<Input type="email" placeholder="Email..." />
<Input type="password" placeholder="Password..." />
```

### With label
```tsx
<div className="space-y-2">
  <label className="text-sm font-medium text-foreground">
    Project Path
  </label>
  <Input placeholder="/path/to/project" />
</div>
```

### ❌ Don't
```tsx
// Forbidden — do not create custom inputs
<input className="w-full p-2 bg-gray-800..." />
```

---

## 🃏 Card

### Import
```tsx
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter
} from "@/components/ui/card"
```

### Full usage
```tsx
<Card>
  <CardHeader>
    <CardTitle>Project Analysis</CardTitle>
    <CardDescription>
      View detailed analysis of your project structure
    </CardDescription>
  </CardHeader>
  <CardContent>
    <p>Card content here...</p>
  </CardContent>
  <CardFooter>
    <Button variant="default">Analyze</Button>
  </CardFooter>
</Card>
```

### Simple usage
```tsx
<Card className="p-4">
  <p>Simple card with custom padding</p>
</Card>
```

### ❌ Don't
```tsx
// Forbidden — do not create custom cards
<div className="border rounded p-4 bg-gray-800...">
  Custom card
</div>
```

---

## 🏷️ Badge

### Import
```tsx
import { Badge } from "@/components/ui/badge"
```

### Variants

#### Success (default)
```tsx
<Badge variant="default">Active</Badge>
<Badge variant="default">Success</Badge>
```

#### Warning
```tsx
<Badge variant="warning">Pending</Badge>
<Badge variant="warning">Warning</Badge>
```

#### Destructive (error)
```tsx
<Badge variant="destructive">Error</Badge>
<Badge variant="destructive">Failed</Badge>
```

#### Info
```tsx
<Badge variant="info">Info</Badge>
<Badge variant="info">Processing</Badge>
```

#### Secondary
```tsx
<Badge variant="secondary">Draft</Badge>
```

#### Outline
```tsx
<Badge variant="outline">Neutral</Badge>
```

### ❌ Don't
```tsx
// Forbidden — do not create custom badges
<span className="px-2 py-1 bg-green-500 rounded text-xs">
  Custom Badge
</span>
```

---

## ➖ Separator

### Import
```tsx
import { Separator } from "@/components/ui/separator"
```

### Usage
```tsx
// Horizontal (default)
<Separator />

// Vertical
<Separator orientation="vertical" />
```

---

## ✍️ Typography

### Headings
```tsx
<h1 className="text-4xl font-bold text-foreground">Main Title</h1>
<h2 className="text-3xl font-bold text-foreground">Section Title</h2>
<h3 className="text-2xl font-semibold text-foreground">Subsection</h3>
<h4 className="text-xl font-semibold text-foreground">Card Title</h4>
```

### Body text
```tsx
<p className="text-base text-foreground">Primary text</p>
<p className="text-sm text-foreground-muted">Secondary text</p>
<p className="text-xs text-foreground-subtle">Hint text</p>
```

### Code / monospace
```tsx
<code className="font-mono text-sm text-brand">/path/to/file</code>
```

---

## 📐 Spacing

**Rule**: use multiples of 4 (the Tailwind spacing scale).

```tsx
p-1  (4px)   // Minimum
p-2  (8px)   // Small
p-3  (12px)  // Medium
p-4  (16px)  // Standard
p-6  (24px)  // Large
p-8  (32px)  // Extra large
```

### Gaps between elements
```tsx
<div className="space-y-4">  // Vertical spacing
  <div>Item 1</div>
  <div>Item 2</div>
</div>

<div className="flex gap-2">  // Horizontal gap
  <Button>Button 1</Button>
  <Button>Button 2</Button>
</div>
```

---

## 🔄 Transitions

**Always** use `transition-colors` for interactive elements:

```tsx
// Correct
<div className="hover:bg-background-tertiary transition-colors">
  Hoverable
</div>

// Incorrect — no transition
<div className="hover:bg-background-tertiary">
  Jumpy hover
</div>
```

---

## 📱 Responsive design

Use Tailwind breakpoints:

```tsx
<div className="flex flex-col md:flex-row gap-4">
  <div className="w-full md:w-1/2">Column 1</div>
  <div className="w-full md:w-1/2">Column 2</div>
</div>
```

Breakpoints:
```
sm:  640px
md:  768px
lg:  1024px
xl:  1280px
2xl: 1536px
```

---

## 🎯 Common patterns

### Form layout
```tsx
<div className="space-y-4">
  <div className="space-y-2">
    <label className="text-sm font-medium text-foreground">
      Project Name
    </label>
    <Input placeholder="my-project" />
  </div>

  <div className="space-y-2">
    <label className="text-sm font-medium text-foreground">
      Path
    </label>
    <Input placeholder="/path/to/project" />
  </div>

  <Button variant="default">Submit</Button>
</div>
```

### List item
```tsx
<div className="flex items-center justify-between p-4 rounded-lg border border-border hover:bg-background-tertiary transition-colors">
  <div>
    <h4 className="font-medium text-foreground">Item Title</h4>
    <p className="text-sm text-foreground-muted">Description</p>
  </div>
  <Button variant="ghost" size="sm">Action</Button>
</div>
```

### Modal / dialog content
```tsx
<Card className="w-full max-w-md">
  <CardHeader>
    <CardTitle>Confirm Action</CardTitle>
    <CardDescription>
      Are you sure you want to proceed?
    </CardDescription>
  </CardHeader>
  <CardContent>
    <p className="text-sm text-foreground-muted">
      This action cannot be undone.
    </p>
  </CardContent>
  <CardFooter className="gap-2">
    <Button variant="outline">Cancel</Button>
    <Button variant="destructive">Confirm</Button>
  </CardFooter>
</Card>
```

---

## ⚠️ Golden rules

### ✅ Do

1. **Use `ui/` components**
```tsx
import { Button } from "@/components/ui/button"
<Button variant="default">Click</Button>
```

2. **Use Venore color tokens**
```tsx
<div className="bg-background text-foreground border border-border">
  Content
</div>
```

3. **Use `cn()` for conditional classes**
```tsx
import { cn } from "@/lib/utils"

<div className={cn(
  "base-classes",
  isActive && "active-classes",
  className
)}>
  Content
</div>
```

### ❌ Don't

1. **Do not create UI components outside `ui/`**
```tsx
// Forbidden
<button className="px-4 py-2 bg-green-500...">
  Custom Button
</button>
```

2. **Do not use raw Tailwind colors**
```tsx
// Forbidden
<div className="bg-green-500 text-white">
  Wrong colors
</div>

// Correct
<div className="bg-brand text-background">
  Venore colors
</div>
```

3. **Do not use inline styles**
```tsx
// Forbidden
<div style={{ color: '#01e8a2', padding: '1rem' }}>
  Content
</div>

// Correct
<div className="text-brand p-4">
  Content
</div>
```

4. **Do not use gradients**
```tsx
// Forbidden
<div className="bg-gradient-to-r from-green-400 to-blue-500">
  Gradient
</div>
```

---

## 🔍 Debugging

### Verify colors
If a color does not render correctly:
1. Confirm you are using the right class: `bg-brand`, `text-foreground`, etc.
2. Confirm the CSS variables are defined in `index.css`.
3. Confirm the colors are mapped in `tailwind.config.ts`.

### Component not styled
1. Confirm the import comes from `@/components/ui/...`.
2. Confirm the component exists under `src/components/ui/`.
3. Confirm Tailwind is processing the styles (see `index.css`).

---

## 📚 References

- **shadcn/ui docs**: https://ui.shadcn.com/
- **Tailwind CSS docs**: https://tailwindcss.com/docs
- **Radix UI docs**: https://www.radix-ui.com/docs/primitives

---

## 🎨 Visual summary

```
┌─────────────────────────────────────────────────────────────────┐
│  VENORE COLOR PALETTE                                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  BRAND                    SEMANTIC                              │
│  ┌──────────┐            ┌──────────┬──────────┬──────────┬────┤
│  │ #01e8a2  │            │ success  │ warning  │ error    │info│
│  │ Primary  │            │ #01e8a2  │ #f59e0b  │ #ef4444  │#3b8│
│  └──────────┘            └──────────┴──────────┴──────────┴────┤
│                                                                 │
│  BACKGROUNDS              TEXT                                  │
│  ┌──────────┬──────────┐ ┌──────────┬──────────┬──────────┐    │
│  │ #09090b  │ #18181b  │ │ #fafafa  │ #a1a1aa  │ #71717a  │    │
│  │ base     │ elevated │ │ primary  │ muted    │ subtle   │    │
│  └──────────┴──────────┘ └──────────┴──────────┴──────────┘    │
│                                                                 │
│  BORDERS                                                        │
│  ┌──────────┬──────────┐                                       │
│  │ #27272a  │ #3f3f46  │                                       │
│  │ default  │ hover    │                                       │
│  └──────────┴──────────┘                                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**This design system is mandatory for all Venore components.**
