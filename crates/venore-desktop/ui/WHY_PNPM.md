# Why is pnpm required in Venore V2?

This project **requires pnpm** and **actively blocks npm/yarn**.

---

## Technical reasons

### 1. Prevents "phantom dependencies"

**npm/yarn allow this (problem):**
```tsx
// Not declared in package.json, but works (by accident)
import { something } from 'transitive-dependency'
```

**pnpm does NOT allow it:**
```
Error: Cannot find module 'transitive-dependency'
```

This **forces** every dependency to be declared explicitly.

### 2. Faster

```
Benchmark — npm vs pnpm:
npm:   ~45 seconds
pnpm:  ~15 seconds (3x faster)
```

### 3. Saves disk space

```
npm:  ~500MB per project (node_modules)
pnpm: ~50MB per project (symlinks)

If you have 10 projects using React:
npm:  10 copies of React = 5GB
pnpm: 1 copy of React = 500MB
```

pnpm uses a global **content-addressable store**.

### 4. More efficient workspaces

Venore V2 is a Cargo monorepo. pnpm handles workspaces better:
```
crates/
  venore-desktop/ui/     ← pnpm workspace
    node_modules/        ← symlinks only
```

### 5. Reproducibility

`pnpm-lock.yaml` is stricter than `package-lock.json`:
- Guarantees identical versions in CI/CD
- Prevents "works on my machine"

### 6. Tauri recommends pnpm

From the [Tauri documentation](https://tauri.app/):
> "We recommend using pnpm for its speed and disk space efficiency."

---

## Lessons from Venore V1

### V1 problem (with npm):

1. **Phantom dependencies**
   ```tsx
   // Imported this without declaring it in package.json
   import { something } from 'lodash'
   // Worked because another package pulled it in
   ```

2. **Bloated node_modules**
   ```
   node_modules/  ~800MB
   ```

3. **"Works on my machine"**
   - Dev: has the dependencies installed globally
   - CI: fails because it doesn't
   - Hard to debug

4. **Slow installs**
   - Each `npm install` took 1-2 minutes
   - Worse in CI/CD

### V2 solution (with pnpm):

1. ✅ **Explicit dependencies** — error if it isn't in package.json
2. ✅ **Smaller node_modules** — symlinks only (~50MB)
3. ✅ **Reproducible** — strict pnpm-lock.yaml
4. ✅ **Fast installs** — 15 seconds

---

## How is it enforced?

### 1. preinstall script
```json
// package.json
"scripts": {
  "preinstall": "npx only-allow pnpm"
}
```

If you try `npm install`:
```bash
$ npm install
npm ERR! Use "pnpm install" instead.
```

### 2. Strict engine
```json
// package.json
"engines": {
  "pnpm": ">=8.0.0"
},
"packageManager": "pnpm@8.15.0"
```

```
// .npmrc
engine-strict=true
```

### 3. Clear documentation
- README.md mentions pnpm
- PNPM_REQUIRED.md explains how to install it
- WHY_PNPM.md (this file) explains the why

---

## Installing pnpm

**Option 1: with npm (paradoxical but works)**
```bash
npm install -g pnpm
```

**Option 2: standalone script (recommended)**
```bash
# Linux/macOS
curl -fsSL https://get.pnpm.io/install.sh | sh -

# Windows (PowerShell)
iwr https://get.pnpm.io/install.ps1 -useb | iex
```

**Option 3: OS package manager**
```bash
# macOS (Homebrew)
brew install pnpm

# Windows (Chocolatey)
choco install pnpm

# Windows (Scoop)
scoop install pnpm
```

Verify:
```bash
pnpm --version
```

---

## Common commands

```bash
# Install dependencies
pnpm install

# Add a dependency
pnpm add react

# Remove
pnpm remove react

# Scripts (you can omit "run")
pnpm dev
pnpm build
pnpm test

# Update
pnpm update

# Clean
pnpm store prune
```

---

## npm vs pnpm comparison

| Feature | npm | pnpm |
|---------|-----|------|
| Speed | ⚠️ Slow | ✅ 3x faster |
| Disk space | ❌ ~500MB/project | ✅ ~50MB/project |
| Phantom deps | ❌ Allowed | ✅ Blocked |
| Lock file | ⚠️ Less strict | ✅ Very strict |
| Workspaces | ⚠️ Basic | ✅ Advanced |
| Reproducibility | ⚠️ Good | ✅ Excellent |

---

## Conclusion

**pnpm is not optional in Venore V2.**

It is an architectural decision to:
- ✅ Avoid dependency errors
- ✅ Improve performance
- ✅ Save disk space
- ✅ Guarantee reproducibility
- ✅ Avoid V1 problems

If you try to use npm, the project **will intentionally fail**.

**Install pnpm and use it. Your future self will thank you.** 🚀

---

## References

- [pnpm.io](https://pnpm.io/)
- [Why pnpm?](https://pnpm.io/motivation)
- [pnpm vs npm benchmark](https://pnpm.io/benchmarks)
- [Tauri + pnpm](https://tauri.app/v1/guides/getting-started/setup/)
