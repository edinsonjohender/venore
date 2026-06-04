# Releasing Venore

How a new version of Venore is built and published. The installer is **built by
CI from a git tag** — you never compile locally and upload the file by hand.

## TL;DR

```bash
# 1. Bump the version (two files, keep them in sync)
#    - crates/venore-desktop/tauri.conf.json   ->  "version": "X.Y.Z"
#    - crates/venore-desktop/package.json       ->  "version": "X.Y.Z"

# 2. Commit on main and push
git commit -am "release: bump version to X.Y.Z"
git push origin main

# 3. Tag and push the tag -> this triggers the build
git tag vX.Y.Z
git push origin vX.Y.Z
```

The tag push starts the **Release** workflow. When it finishes it creates a
**draft** GitHub Release named `Venore vX.Y.Z` with the installer attached
(`Venore_X.Y.Z_x64-setup.exe`). Go to the repo's **Releases** tab, open the
draft, confirm the `.exe` is in *Assets*, and click **Publish release**. Delete
any older obsolete drafts from the same page.

## How it works

- Workflow: [`.github/workflows/release.yml`](../.github/workflows/release.yml).
- Trigger: pushing a tag matching `v*`.
- Builder: [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action)
  on `windows-latest`. It installs deps, runs the frontend build (Tauri's
  `beforeBuildCommand`), compiles the Rust app, bundles the NSIS installer, and
  uploads it to the Release.
- Auth: the workflow declares `permissions: contents: write`, so the default
  `GITHUB_TOKEN` can create the Release. No secrets to configure. The repo's
  *Settings → Actions → Workflow permissions* can stay on read-only — the
  workflow grants itself what it needs.

## Version source of truth

The installer's name and the app version come from **`tauri.conf.json`**'s
`version` field. `package.json`'s `version` is kept in sync for hygiene. Bump
both. Nothing else needs editing for a normal patch/minor release.

## Re-running a failed release

If a tagged build fails, fix the problem on `main`, then **move the tag** to the
new commit and re-push (a failed run does not create a published Release, so
there is nothing to clean up):

```bash
git push origin :refs/tags/vX.Y.Z   # delete the remote tag
git tag -d vX.Y.Z                    # delete the local tag
git tag vX.Y.Z                       # recreate on the fixed HEAD
git push origin vX.Y.Z
```

## Known gotcha: the desktop pnpm lockfile

`crates/venore-desktop/pnpm-lock.yaml` **must stay at `lockfileVersion: '6.0'`**
(pnpm 8), matching the pnpm version CI uses and the `ui/` lockfile. If you run
`pnpm install` in `crates/venore-desktop` with **pnpm 9 or 10**, it silently
rewrites the lockfile to `9.0`, which CI's pnpm 8 then rejects with
`ERR_PNPM_NO_LOCKFILE` — and the release fails at the install step.

If it drifts back to `9.0`, regenerate it with pnpm 8:

```bash
cd crates/venore-desktop
CI=true npx --yes pnpm@8.15.0 install --lockfile-only --no-frozen-lockfile
head -1 pnpm-lock.yaml      # should print: lockfileVersion: '6.0'
```

## The installer is unsigned

The Windows `.exe` is **not code-signed**, so SmartScreen shows *"Windows
protected your PC"*. Users click **More info → Run anyway**. This is stated in
the Release body. Signing would need a paid code-signing certificate; deferred.

## Platform scope

Currently **Windows only** — `tauri.conf.json` pins `bundle.targets` to
`["nsis"]`. macOS and Linux are not built yet. To add them later:

1. Give each platform its own targets via a per-platform config
   (`tauri.linux.conf.json` with `["deb", "appimage"]`, a macOS equivalent),
   following the existing `tauri.macos.conf.json` pattern.
2. Expand the `release.yml` job into a matrix over `windows-latest`,
   `ubuntu-latest` (with the GTK/WebKit `apt` deps the CI `rust` job installs),
   and `macos-latest`.

Tauri does not cross-compile, so each OS must be built on its own runner.

## Local build (not for publishing)

To produce an installer locally for testing — not the canonical release path:

```bash
cd crates/venore-desktop
cargo tauri build
# -> target/release/bundle/nsis/Venore_X.Y.Z_x64-setup.exe
```
