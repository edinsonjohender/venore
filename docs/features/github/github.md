# Git / GitHub

> Estado: parcial (ver tabla por feature)
> Módulo core GitHub: `crates/venore-core/src/github/`
> Módulo core sesiones (git local): `crates/venore-core/src/session/`
> Bridge Tauri: `crates/venore-desktop/src/commands/github.rs`, `commands/session.rs`
> UI: `crates/venore-desktop/ui/src/components/workspace/panels/GitHubPanel.tsx`, `components/github/CloneFromGitHubModal.tsx`

## Alcance

Todo lo que toca git o GitHub se agrupa en seis features distintos. Comparten la
dependencia de git/GitHub pero hacen cosas diferentes y se revisan por separado.

| # | Feature | Qué hace | Estado |
|---|---------|----------|--------|
| 1 | Conexión de cuenta | Autenticar contra GitHub (PAT, GCM, Device Flow) + detectar el repo | Auditado a fondo. PAT y detección OK; GCM frágil; Device Flow muerto |
| 2 | Explorar GitHub | Listar PRs, issues, comentarios, archivos de PR | Implementado, sin auditar a fondo |
| 3 | Análisis de PR (IA) | El LLM analiza un PR | Implementado pero degradado (lee `.context.md` deprecado) |
| 4 | Clonar desde GitHub | Traer un repo remoto como proyecto nuevo | Implementado, sin auditar a fondo |
| 5 | Sesiones / worktrees | Aislar cambios de la IA en una rama/worktree (git local, sin red) | Implementado, sin auditar a fondo |
| 6 | Monitor de contexto | Detectar commits nuevos para regenerar contexto | Deprecado; aún corre `git fetch` al abrir proyecto |

Solo el feature 1 (Conexión) está auditado en detalle. El resto se documenta a
nivel de superficie.

---

## 1. Conexión de cuenta

Hay tres formas de autenticar más la detección del repo del proyecto.

### Almacenamiento del token

El token vive en el keyring del SO (Windows Credential Manager / macOS Keychain /
Secret Service). Servicio `venore.ai`, clave `github_token`. Lo usan todas las
operaciones de GitHub (listar PRs, issues, clonar, análisis).
Funciones: `auth::store_token`, `get_stored_token`, `remove_token` en
`crates/venore-core/src/github/auth.rs`.

### PAT (Personal Access Token)

Vía principal de la UI. El usuario pega un token (`ghp_...`) con scopes `repo` y
`read:org`.

```
<input> en GitHubPanel.tsx
  -> tauriApi.githubStorePat
    -> github_store_pat (commands/github.rs:236)
      -> auth::store_pat (auth.rs:289)
        -> GitHubClient::validate_token() [GET /user]
        -> store_token() [keyring]
```

Estado: funciona. Verificado en vivo (conexión correcta, valida y guarda).
El JSX del input está duplicado en `GitHubPanel.tsx` (variante con-GCM y sin-GCM)
y en `CloneFromGitHubModal.tsx`.

### GCM (Git Credential Manager)

Lee las credenciales de GitHub ya guardadas por `git` en el sistema, vía
`git credential fill`. Función `auth::try_git_credential_token` (auth.rs:220).

Estado: funciona pero frágil. Problemas:

- Timeout de 5 s (auth.rs:240): si GCM abre el selector nativo de cuentas (caso de
  varias cuentas) y el usuario tarda más de 5 s en elegir, Venore abandona la
  llamada y reporta "sin credenciales". La selección posterior se pierde.
- Se dispara automáticamente en el chequeo de auth (al abrir el panel), abriendo un
  diálogo bloqueante del SO sin que el usuario lo pida.
- `auth::resolve_token` (auth.rs:267) cae a GCM en silencio (keyring -> GCM). La
  capa de API usa ese token sin aceptación explícita del usuario; la UI lo evita al
  no llamar a la API hasta estar autenticado, pero el backend lo permitiría.

### Device Flow (login de un clic)

Implementado pero no funcional y sin usar.

- `GITHUB_CLIENT_ID` es un placeholder (`Ov23li0000000000000`, auth.rs:18). GitHub
  rechaza toda petición. Requiere registrar una OAuth App real con Device Flow.
- Ningún componente de la UI llama a `githubStartDeviceFlow`. La cadena existe en
  core (`request_device_code`, `poll_for_token`), comandos
  (`github_start_device_flow`, `github_cancel_device_flow`), registro en `main.rs`
  y wrapper en `lib/tauri.ts`, pero se corta en la UI.
- Bug menor: el manejo de `slow_down` resetea el intervalo en vez de aumentarlo
  (commands/github.rs:213).

### Detección de repo

Decide a qué repo de GitHub está ligado un proyecto (`owner/repo`). Es lo que
produce "No GitHub remote found in .git/config".

```
detect_github_repo (repo.rs:20)
  -> git remote get-url origin
  -> fallback: parsear .git/config
  -> parse_github_remote(): HTTPS, SSH (git@), SSH URL (ssh://)
```

Estado: completo, con 15 tests unitarios. Limitaciones de alcance:

- Solo mira el remote `origin` (no detecta remotes con otro nombre, p. ej. `upstream`).
- Solo `github.com` hardcodeado (sin GitHub Enterprise).

---

## 2. Explorar GitHub

Listar y ver datos del repo remoto, solo lectura. Comandos en
`commands/github.rs`: `github_list_pulls`, `github_list_issues`,
`github_get_pr_detail`, `github_get_pr_files`, `github_get_comments`.
UI en `components/workspace/panels/GitHubPanel.tsx` (tabs PRs/Issues) y vistas de
detalle en `components/workspace/canvas/`. Sin auditar a fondo.

## 3. Análisis de PR (IA)

`commands/github.rs::github_analyze_pr` -> `github/pr_analyzer.rs`. Ensambla
contexto del PR (parches + contexto de proyecto) y lo pasa al LLM con niveles de
profundidad. Degradado: lee `.context.md` por módulo, que está deprecado y
sustituido por Project Memory (`crate::memory`). Funciona pero con contexto viejo.

## 4. Clonar desde GitHub

`commands/github.rs::github_clone_repo` -> `github/clone.rs`. Clone con progreso
parseado del stderr de git e inyección de token para repos privados. UI en
`components/github/CloneFromGitHubModal.tsx`. Sin auditar a fondo.

## 5. Sesiones / worktrees (git local)

No es GitHub: es git local sin red. Aísla los cambios de la IA en una rama o git
worktree. Core en `crates/venore-core/src/session/` (`git_ops.rs`,
`repository.rs`), comandos en `commands/session.rs` (`create_session`,
`abandon_session`, `session_diff_files`, `session_commits`, `revert_to_snapshot`).
Persistencia en SQLite. Sin auditar a fondo.

## 6. Monitor de contexto (deprecado)

`crates/venore-core/src/context_updater/`. Vigilaba commits nuevos para regenerar
`.context.md`. Marcado como deprecado (sustituido por Project Memory); los comandos
en `commands/context_updater.rs` están registrados pero no los llama la UI. Sigue
ejecutándose `git fetch` al abrir un proyecto (vía branch_monitor), lo que genera
ruido en proyectos sin `.git`. Pendiente de eliminación.

---

## Procesos de consola en Windows

Toda invocación a git/docker/LSP pasa por `crate::utils::quiet_command` /
`quiet_tokio_command` (`crates/venore-core/src/utils/process.rs`), que aplica
`CREATE_NO_WINDOW` en Windows para no abrir una ventana de consola por cada spawn.

---

## Bugs y decisiones pendientes (feature 1)

1. GCM: timeout de 5 s abandona el selector interactivo; debería esperar más o no
   ser interactivo.
2. GCM: se dispara solo en el chequeo de auth; debería ser bajo botón explícito.
3. `resolve_token` cae a GCM en silencio; debería usar solo el token aceptado.
4. Device Flow: decidir entre eliminarlo o revivirlo (OAuth App + cablear UI).
5. `slow_down` resetea el intervalo en vez de aumentarlo (commands/github.rs:213).
6. Input de PAT duplicado en tres puntos; extraer a un componente.
7. Detección de repo: soporte opcional para remotes no-`origin` y GitHub Enterprise.

## Tres cosas distintas que se llaman "GitHub"

- Conectar repo (este documento): PAT/GCM/Device Flow para ver PRs y clonar.
- Login social con GitHub: identidad para Venore Cloud
  (`authStore.ts::signInWithOAuth('github')`). Medio implementado, UI en "Soon".
- Venore Cloud: la SaaS de colaboración (Supabase). Backend presente, UI apagada.
