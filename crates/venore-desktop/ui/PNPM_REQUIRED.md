# ⚠️ pnpm is REQUIRED

This project **requires pnpm**. npm and yarn are not supported.

---

## Why pnpm?

1. **Faster** — installs dependencies 2-3x faster than npm
2. **Less disk space** — uses symlinks, saving GB of disk usage
3. **Stricter** — prevents phantom dependencies
4. **Monorepo friendly** — more efficient workspaces
5. **Tauri-compatible** — recommended for Tauri projects

---

## Installing pnpm

### Windows
```bash
# With npm (ironic but it works)
npm install -g pnpm

# With PowerShell (recommended)
iwr https://get.pnpm.io/install.ps1 -useb | iex

# With Scoop
scoop install pnpm

# With Chocolatey
choco install pnpm
```

### macOS/Linux
```bash
# With curl
curl -fsSL https://get.pnpm.io/install.sh | sh -

# With Homebrew (macOS)
brew install pnpm

# With npm
npm install -g pnpm
```

Verify the install:
```bash
pnpm --version
```

---

## What happens if you try to use npm?

The project **blocks** the install:

```bash
$ npm install
npm ERR! code ELIFECYCLE
npm ERR! Use "pnpm install" instead.
```

This is **intentional** and configured in:
- **`package.json`** → `preinstall` script with `only-allow`
- **`.npmrc`** → `engine-strict=true`
- **`package.json`** → `engines` and `packageManager` fields

---

## Common commands

### Install dependencies
```bash
pnpm install
```

### Add a dependency
```bash
pnpm add react
pnpm add -D typescript
```

### Remove a dependency
```bash
pnpm remove react
```

### Scripts
```bash
pnpm run dev        # Run the dev server
pnpm run build      # Production build
pnpm run lint       # Linting
pnpm run typecheck  # Type checking
```

### Update dependencies
```bash
pnpm update              # Update all
pnpm update react        # Update a specific one
pnpm update --latest     # Update to latest (ignores semver)
```

### Clean
```bash
pnpm store prune        # Clean the pnpm store
rm -rf node_modules     # Remove node_modules
pnpm install            # Reinstall
```

---

## Differences vs npm

| npm command | pnpm command |
|-------------|--------------|
| `npm install` | `pnpm install` |
| `npm install react` | `pnpm add react` |
| `npm uninstall react` | `pnpm remove react` |
| `npm run dev` | `pnpm run dev` or `pnpm dev` |
| `npm update` | `pnpm update` |
| `npx some-package` | `pnpm dlx some-package` |

**Note**: pnpm lets you omit `run` in scripts:
```bash
pnpm dev      # Equivalent to: pnpm run dev
pnpm build    # Equivalent to: pnpm run build
```

---

## Troubleshooting

### Error: "Use pnpm install"
✅ **Fix**: simply use `pnpm install` instead of `npm install`

### pnpm is not installed
✅ **Fix**: install pnpm (see instructions above)

### Corrupt dependencies
```bash
rm -rf node_modules pnpm-lock.yaml
pnpm install
```

### Corrupt cache
```bash
pnpm store prune
pnpm install
```

### "Module not found" after install
```bash
# Full clean
rm -rf node_modules .pnpm-store pnpm-lock.yaml
pnpm install
```

---

## Resources

- [pnpm docs](https://pnpm.io/)
- [pnpm CLI](https://pnpm.io/cli/install)
- [Why pnpm?](https://pnpm.io/motivation)
- [pnpm vs npm](https://pnpm.io/benchmarks)

---

## TL;DR

```bash
# 1. Install pnpm
npm install -g pnpm

# 2. Install project dependencies
cd venore_v2/crates/venore-desktop/ui
pnpm install

# 3. Run
pnpm dev
```

**And forget about npm!** 🚀
