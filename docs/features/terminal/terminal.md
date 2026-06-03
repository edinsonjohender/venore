# Terminal

> Estado: implementado
> Módulo core: `crates/venore-core/src/terminal/`
> Tools del LLM: `crates/venore-core/src/tools/executor.rs`
> Bridge Tauri: `crates/venore-desktop/src/commands/terminal.rs`
> UI: `crates/venore-desktop/ui/src/components/workspace/TerminalPanel.tsx`

## TL;DR

El terminal de Venore es un **PTY real** (no una simulación) embebido en la app, que sirve a dos consumidores simultáneamente:

1. **El usuario** — escribe y ve la terminal en un xterm.js con colores, scrollback y links clicables.
2. **El agente LLM** — ejecuta comandos y lee la salida vía tools (`run_terminal_command`, `read_terminal_output`, `run_app`, `check_health`).

Ambos comparten **la misma sesión de shell**. Cuando la IA corre un comando, el usuario lo ve en vivo; cuando el usuario escribe algo, queda en el buffer que la IA puede leer. El estado de la shell (`cwd`, env vars, alias) persiste entre comandos.

---

## Arquitectura en una imagen

```
┌──────────────────────────────────────────────────────────────────┐
│  Frontend (React + xterm.js v6)                                  │
│                                                                  │
│   TerminalPanel.tsx ──► useTerminal.ts ──► xterm.js Terminal     │
│         │                    ▲                                   │
│         │ tauriApi.write()   │ term.write(data)                  │
│         │ tauriApi.resize()  │  event "terminal:output"          │
└─────────┼────────────────────┼───────────────────────────────────┘
          │                    │
          ▼                    │
┌─────────────────────────────────────────────────────────────────┐
│  Tauri Bridge (commands/terminal.rs)                            │
│                                                                 │
│   spawn_terminal · write_terminal · resize_terminal             │
│   kill_terminal  · list_terminals                               │
│                                                                 │
│   start_terminal_read_loop (tokio::spawn_blocking, buf 4096)    │
│         ├─► app.emit("terminal:output", raw_chunk)              │
│         ├─► mgr.append_output(stripped_ansi)                    │
│         └─► on EOF: remove_dead_session + emit "terminal:dead"  │
└──────────┬──────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────┐
│  Core (venore-core/src/terminal/)                               │
│                                                                 │
│   TerminalSessionManager   (singleton: Arc<Mutex<>>)            │
│     ├── sessions:        HashMap<String, TerminalSession>       │
│     ├── output_buffers:  HashMap<String, VecDeque<String>>      │
│     │                    (MAX_BUFFER_LINES = 500, FIFO)         │
│     ├── line_counters:   HashMap<String, u64>  (monotónico)     │
│     ├── session_to_terminal: HashMap<dev_session, terminal>     │
│     └── terminal_to_session: HashMap<terminal, dev_session>     │
│                                                                 │
│   TerminalSession  (portable-pty)                               │
│     ├── master:  Box<dyn MasterPty>                             │
│     ├── child:   Box<dyn Child>  ← cmd.exe / $SHELL             │
│     ├── reader:  Arc<Mutex<Box<dyn Read>>>                      │
│     └── writer:  Arc<Mutex<Box<dyn Write>>>                     │
└─────────────────────────────────────────────────────────────────┘
           │
           ▼
       Shell process (cmd.exe en Windows, $SHELL en Unix)
```

---

## Estructura del módulo core

`crates/venore-core/src/terminal/`

| Archivo | Líneas | Responsabilidad |
|---------|--------|-----------------|
| `mod.rs` | 11 | Re-exporta `TerminalSessionManager` y `TerminalSession` |
| `session.rs` | ~185 | Wrapper sobre un PTY individual (spawn, write, resize, kill) + resolución de path de PowerShell |
| `manager.rs` | 239 | Singleton global, buffer de salida, binding con dev sessions |
| `debug.rs` | ~165 | Logger opt-in de bytes raw del PTY (ver "Instrumentación") |

### `TerminalSession`

`session.rs`

```rust
pub struct TerminalSession {
    id: String,
    master: Box<dyn MasterPty + Send>,           // PTY master
    child: Box<dyn Child + Send + Sync>,         // Proceso shell
    reader: Arc<Mutex<Box<dyn Read + Send>>>,    // Lectura (clonable)
    writer: Arc<Mutex<Box<dyn Write + Send>>>,   // Escritura
    cols: u16,
    rows: u16,
}
```

#### Spawn

Usa `portable_pty::native_pty_system()`. Abre par master/slave, lanza la shell en el slave, suelta el slave, se queda con el master:

```rust
let pty_system = native_pty_system();
let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
let child = pair.slave.spawn_command(cmd)?;
drop(pair.slave);
let reader = pair.master.try_clone_reader()?;
let writer = pair.master.take_writer()?;
```

#### Shell por plataforma

**Windows** (`#[cfg(windows)]`) — PowerShell:
```rust
let shell = resolve_powershell_path();
let mut cmd = CommandBuilder::new(shell);
cmd.arg("-NoLogo");
cmd.arg("-NoExit");
cmd.arg("-Command");
let safe_label = label.replace('\'', "''");
cmd.arg(format!("function prompt {{ '[{}] > ' }}", safe_label));
```

Usamos PowerShell (no `cmd.exe`) porque tiene `clear`, `cls` y comandos Unix-like (`ls`, `cat`, `rm`…) como aliases. PowerShell 5.1 viene preinstalado en todas las versiones soportadas de Windows, así que no hay problema de disponibilidad. El prompt se inyecta vía `-Command "function prompt { ... }"` que define la función en la sesión interactiva, y `-NoExit` mantiene la shell viva tras ejecutarla.

`resolve_powershell_path()` resuelve a path absoluto (el proceso de Tauri no siempre hereda el PATH completo, así que `powershell.exe` por nombre falla con `os error 2`). Preferencia:

1. `C:\Program Files\PowerShell\7\pwsh.exe` (PS 7+) si existe
2. `C:\Program Files (x86)\PowerShell\7\pwsh.exe` si existe
3. `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` (PS 5.1) — siempre existe en Windows soportado

**Unix/Mac** (`#[cfg(not(windows))]`):
```rust
let shell = std::env::var("SHELL").unwrap_or("/bin/sh");
let mut cmd = CommandBuilder::new(shell);
cmd.env("PS1", format!("[{}] \\$ ", label));
```

Respeta la shell preferida del usuario (bash, zsh, fish…). `clear` es un binario presente en todas las distros.

El `label` se deriva del último segmento del `cwd` si no se pasa explícito (`venore` como último fallback).

#### Otros métodos

- `write(&self, data: &[u8])` — toma el lock del writer, hace `write_all` + `flush`.
- `resize(&mut self, cols, rows)` — `master.resize(PtySize { ... })` y actualiza los campos.
- `kill(&mut self)` — `child.kill()`, loggea warn si falla.
- `clone_reader()` — devuelve `Arc<Mutex<...>>` del reader. Lo usa el read-loop para no retener el lock del manager.

### `TerminalSessionManager`

`manager.rs`

```rust
const MAX_BUFFER_LINES: usize = 500;

pub struct TerminalSessionManager {
    sessions: HashMap<String, TerminalSession>,
    counter: u32,                                          // Para IDs incrementales
    output_buffers: HashMap<String, VecDeque<String>>,     // 500 líneas máx, FIFO
    line_counters: HashMap<String, u64>,                   // Monotónico
    session_to_terminal: HashMap<String, String>,
    terminal_to_session: HashMap<String, String>,
}
```

Singleton clásico de Venore:

```rust
pub fn global() -> Arc<Mutex<Self>> {
    static INSTANCE: Lazy<Arc<Mutex<TerminalSessionManager>>> =
        Lazy::new(|| Arc::new(Mutex::new(TerminalSessionManager::new())));
    INSTANCE.clone()
}
```

IDs generados como `terminal-{counter}` (counter incrementa al spawn, nunca retrocede).

#### API completa

| Método | Para qué |
|--------|----------|
| `spawn(cwd, cols, rows, label)` | Crea sesión, devuelve `(terminal_id, reader_arc)` |
| `write(id, bytes)` | Manda keystrokes/comandos al PTY |
| `resize(id, cols, rows)` | Sincroniza tamaño con xterm |
| `kill(id)` | Mata el child, remueve sesión, limpia binding |
| `list() -> Vec<String>` | IDs de todas las sesiones activas |
| `count() -> usize` | Número de sesiones |
| `clear()` | Mata todas las sesiones (app exit) |
| `append_output(id, data)` | **Strip ANSI** + push a buffer. Llamado por read-loop |
| `line_counter(id) -> u64` | Snapshot del contador (baseline) |
| `get_output_after(id, after, max)` | Líneas con counter > `after`, máximo `max` |
| `get_recent_output(id, lines)` | Últimas N líneas (sin baseline) |
| `remove_dead_session(id)` | Limpia sesión muerta (sin matar child) |
| `bind_session(dev_session_id, terminal_id)` | Binding 1:1 bidireccional |
| `unbind_session(dev_session_id)` | Rompe el binding |
| `get_session_terminal(dev_session_id) -> Option<&str>` | Resuelve terminal por dev session (verifica que sigue viva) |
| `list_unbound() -> Vec<String>` | Terminales sin dev session asociada |
| `is_session_terminal(terminal_id) -> bool` | ¿Está atada a una dev session? |

#### Detalle: `append_output` y los counters

```rust
pub fn append_output(&mut self, id: &str, data: &str) {
    let clean = crate::utils::strip_ansi_escapes(data);
    let buffer = self.output_buffers.entry(id.to_string()).or_default();
    let counter = self.line_counters.entry(id.to_string()).or_insert(0);
    for line in clean.lines() {
        buffer.push_back(line.to_string());
        *counter += 1;
        if buffer.len() > MAX_BUFFER_LINES {
            buffer.pop_front();          // FIFO: descarta la más vieja
        }
    }
}
```

El `line_counter` **siempre crece** aunque el buffer haga FIFO. Esto permite que `get_output_after` calcule correctamente la posición incluso después de drops:

```rust
let total = self.line_counters.get(id).copied().unwrap_or(0);
let buffer_start = total.saturating_sub(buffer.len() as u64);  // counter de la línea más vieja viva
let skip = after.saturating_sub(buffer_start) as usize;
buffer.iter().skip(skip).take(max).collect::<Vec<_>>().join("\n")
```

#### Cleanup en bind/kill/remove_dead

Las tres rutas que pueden romper un binding (`kill`, `remove_dead_session`, `bind_session` cuando ya existía uno) actualizan **ambos** mapas para mantener el invariante bidireccional.

---

## Errores

`crates/venore-core/src/error.rs`

| Variant | Cuándo |
|---------|--------|
| `TerminalError(String)` | Errores genéricos (mutex poisoned, I/O en write/resize, terminal no disponible) |
| `TerminalSessionNotFound(String)` | `write`/`resize`/`kill`/`get_*_output` sobre ID inexistente |
| `TerminalSpawnFailed(String)` | Fallos de `openpty`, `spawn_command`, `try_clone_reader`, `take_writer` |

Cada uno tiene su código de error en serialización (`TERMINAL_ERROR`, `TERMINAL_SESSION_NOT_FOUND`, `TERMINAL_SPAWN_FAILED`) para que el frontend distinga.

---

## Tauri bridge

`crates/venore-desktop/src/commands/terminal.rs` (185 líneas)

### Comandos expuestos

| Comando | DTO request | Devuelve |
|---------|-------------|----------|
| `spawn_terminal` | `{ cwd?, cols?, rows?, label? }` | `{ terminal_id }` |
| `write_terminal` | `{ terminal_id, data }` | — |
| `resize_terminal` | `{ terminal_id, cols, rows }` | — |
| `kill_terminal` | `{ terminal_id }` | — |
| `list_terminals` | — | `{ terminal_ids: Vec<String> }` |

DTOs en `crates/venore-desktop/src/commands/dto/terminal.rs`. Defaults en `spawn_terminal`:
- `cwd` → `dirs::home_dir()` (fallback `"."`)
- `cols` → `80`
- `rows` → `24`
- `label` → `None` (se deriva del cwd en `TerminalSession::spawn`)

### El read-loop

```rust
pub fn start_terminal_read_loop(
    app: AppHandle,
    terminal_id: String,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
) {
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = match reader.lock()?.read(&mut buf) {
                Ok(0) => break,        // EOF
                Ok(n) => n,
                Err(_) => break,
            };
            let data = String::from_utf8_lossy(&buf[..n]).to_string();

            // 1) Buffer para la IA (sin ANSI)
            TerminalSessionManager::global().lock()
                .map(|mut m| m.append_output(&tid, &data));

            // 2) Evento para xterm (raw con ANSI)
            app.emit("terminal:output", TerminalOutputPayload {
                terminal_id: tid.clone(), data,
            });
        }

        // Cleanup al salir del loop
        TerminalSessionManager::global().lock()
            .map(|mut m| m.remove_dead_session(&tid));
        app.emit("terminal:dead", TerminalDeadPayload { terminal_id: tid });
    });
}
```

Esta **doble salida** es la pieza central que permite que usuario e IA vean cosas coherentes desde el mismo PTY. Notar que `start_terminal_read_loop` es `pub` — lo usa también `helpers.rs::resolve_or_spawn_terminal` cuando la IA auto-spawnea una terminal.

### Eventos emitidos

`crates/venore-desktop/src/commands/dto/terminal.rs`

| Evento | Payload | Cuándo se emite |
|--------|---------|-----------------|
| `terminal:output` | `{ terminal_id, data }` | Cada chunk leído del PTY |
| `terminal:dead` | `{ terminal_id }` | EOF en el read-loop |
| `terminal:ai-spawned` | `{ terminal_id }` | La IA creó terminal unbound |
| `terminal:session-spawned` | `{ terminal_id, dev_session_id, label }` | La IA creó terminal atada a dev session |

---

## Frontend

`crates/venore-desktop/ui/`

### Stack

- **xterm.js v6** (`@xterm/xterm`) — emulador en canvas/DOM.
- **FitAddon** (`@xterm/addon-fit`) — recalcula cols/rows al redimensionar.
- **WebLinksAddon** (`@xterm/addon-web-links`) — URLs clicables.

### `useTerminal.ts` (210 líneas)

Hook que une xterm con Tauri.

**Configuración del terminal**:
```ts
new Terminal({
  theme: TERMINAL_THEME,        // 18 colores zinc-950/zinc/red/.../white
  fontFamily: "'Geist Mono', 'Cascadia Code', 'Fira Code', Consolas, monospace",
  fontSize: 13,
  lineHeight: 1.2,
  cursorBlink: true,
  cursorStyle: 'bar',
  scrollback: 5000,
  allowProposedApi: true,
})
```

**Flujo de datos**:
1. **Input** — `term.onData(data => tauriApi.writeTerminal({ terminal_id, data }))`. Cada tecla va al PTY directamente.
2. **Output** — `listen<TerminalOutputPayload>('terminal:output', e => { if (e.payload.terminal_id === terminalId) term.write(e.payload.data) })`. Filtra por id porque hay múltiples terminales.
3. **Resize** — `fit()` calcula nuevas dimensiones y llama `tauriApi.resizeTerminal({ terminal_id, cols, rows })`.

**Atajos personalizados** (`attachCustomKeyEventHandler`):

| Tecla | Comportamiento |
|-------|----------------|
| `Ctrl+C` | Si hay selección → copia a clipboard. Si no → deja pasar (SIGINT al proceso) |
| `Ctrl+Shift+C` | Siempre copia selección |
| `Ctrl+V` / `Ctrl+Shift+V` | Paste vía `writeTerminal` |
| `Ctrl+L` | `term.clear()` local (no manda nada al PTY) |

**Métodos expuestos**: `{ fit, clear, copySelection, paste, focus }`.

### `TerminalPanel.tsx` (235 líneas)

Panel dock-bottom con tabs.

**Constantes**: `INITIAL_HEIGHT=250`, `MIN_HEIGHT=100`, `MAX_HEIGHT=600`, `PANEL_ANIM_DURATION=200`.

**Comportamiento clave**:
- **Auto-spawn al abrir sin tabs**: `spawnNewTab(projectPath, addTab)` con `cwd=projectPath` y `label` = último segmento del path.
- **`TerminalTabContent` siempre montado**: cada tab renderiza su `useTerminal`. Las inactivas tienen `invisible` (CSS), no `display: none` — preservan scrollback y procesos.
- **ResizeObserver** sobre el contenedor: cuando cambia el tamaño y el tab está activo, llama `fit()`.

**Listeners de eventos**:
- `terminal:ai-spawned` → `addTab(terminalId)` + `open()`.
- `terminal:session-spawned` → `addSessionTab(terminalId, devSessionId, label)` (incluye el label como nombre del tab).
- `terminal:dead` → `removeTab(terminalId)`.

**Cierre de tab**: `tauriApi.killTerminal({ terminal_id })` + `removeTab` local.

### `terminalStore.ts` (Zustand)

```ts
interface TerminalTab {
  id: string                  // terminal_id
  name: string                // Display name
  devSessionId?: string       // undefined = unbound
}

interface TerminalStoreState {
  isOpen: boolean
  tabs: TerminalTab[]
  activeTabId: string | null
  _counter: number            // Para nombrar "Terminal N"

  open / close / toggle
  addTab(terminalId)                       // "Terminal {N}"
  addSessionTab(terminalId, devSessionId, label)  // Usa label como nombre, abre panel
  activateSessionTerminal(devSessionId)    // Cambia activeTabId al de esa session
  removeTab(terminalId)                    // Si era activo, activa el último restante
  setActiveTab(terminalId)
  renameTab(terminalId, name)
}
```

---

## Cómo lo usa el agente LLM

### Las 4 tools

Definidas en `crates/venore-core/src/tools/definitions.rs`:

| Tool | Para qué |
|------|----------|
| `run_terminal_command` | Ejecuta un comando shell arbitrario. Para builds, tests, git, npm, etc. |
| `read_terminal_output` | Lee últimas N líneas (default 50). Para revisar output pasado |
| `run_app` | Para apps long-running (servers, containers). Detección de puerto, health checks |
| `check_health` | Verifica vía HTTP que una app responda correctamente. Pareja de `run_app` |

**Disponibilidad por modo**:
- `default_tools()` y `chat_tools()` — incluyen terminal completo.
- `plan_mode_tools()` — **excluyen** terminal (modo read-only).
- `executor_tools()` — terminal sin `run_app` (sub-agente executor solo usa `run_app`).

### Resolución de terminal: `resolve_or_spawn_terminal`

`crates/venore-desktop/src/commands/chat/helpers.rs:92`

Decide qué terminal usar para una tool call. Dos paths:

**Modo session-bound** (`dev_session_id` presente):
1. Si la dev session ya tiene terminal atada → la reusa.
2. Si no → `spawn(cwd, 80, 24, label)`, `bind_session`, emite `terminal:session-spawned`, arranca read-loop.

La terminal persiste durante toda la dev session. `cd`, env vars, jobs en background — todo se mantiene.

**Modo unbound** (sin `dev_session_id`):
1. Reusa la primera terminal de `list_unbound()` si hay.
2. Si no → spawn + emite `terminal:ai-spawned`.

`cwd` = `project_path` o `dirs::home_dir()`. Las terminales unbound se reciclan entre tareas no relacionadas.

### `resolve_terminal_id` en el executor

```rust
fn resolve_terminal_id(ctx: &ToolExecutionContext) -> Result<&str> {
    ctx.terminal_id
        .as_deref()
        .ok_or_else(|| VenoreError::ToolExecutionFailed("No terminal available".into()))
}
```

El executor no spawnea — espera que el dispatcher (en `helpers.rs`) ya haya inyectado el `terminal_id` en el `ToolExecutionContext` antes de invocar la tool.

### El patrón baseline

**`execute_run_command`**:

```rust
let baseline = mgr.line_counter(terminal_id);             // snapshot ANTES
mgr.write(terminal_id, format!("{}\r", command).as_bytes())?;  // \r = Enter
Ok(ToolExecutionResult { success: true, baseline: Some(baseline), ... })
```

**`post_process_terminal`** (`crates/venore-desktop/src/commands/chat/tool_dispatch.rs:1482`):

```rust
if let (Some(tid), Some(baseline)) = (active_terminal_id, tool_result.baseline) {
    venore_core::tools::wait_for_output(tid, baseline, 15).await;

    let output = TerminalSessionManager::global().lock().ok()
        .and_then(|m| m.get_output_after(tid, baseline, 50).ok());

    if let Some(output) = output {
        let combined = format!("{}\n\nTerminal output:\n{}", tool_result.output, output);
        llm_messages.push(LlmMessage { role: Tool, content: truncate_for_llm(&combined, ...), ... });
        return true;  // Reemplaza el push normal
    }
}
```

Se ejecuta en `tool_dispatch.rs:905` y también en `sub_agent.rs:223` (para sub-agentes).

### `wait_for_output` — estabilidad de salida

`crates/venore-core/src/tools/executor.rs:568`

```rust
pub async fn wait_for_output(terminal_id: &str, baseline: u64, max_secs: u64) {
    let poll = Duration::from_millis(200);          // Polleo cada 200ms
    let stability = Duration::from_millis(500);     // Estable = sin líneas nuevas por 500ms
    let deadline = Instant::now() + Duration::from_secs(max_secs);

    let mut last_lines = mgr.line_counter(terminal_id).saturating_sub(baseline);
    let mut stable_since = Instant::now();

    loop {
        tokio::time::sleep(poll).await;
        if Instant::now() >= deadline { break; }
        let current = mgr.line_counter(terminal_id).saturating_sub(baseline);
        if current != last_lines {
            last_lines = current;
            stable_since = Instant::now();           // Reset estabilidad
        } else if stable_since.elapsed() >= stability {
            break;                                    // Estable → salir
        }
    }
}
```

Sale cuando la salida está "estable" (sin líneas nuevas por 500ms) o por timeout. Timeout default para `post_process_terminal` = 15s.

### `read_terminal_output`

```rust
fn execute_read_output(args, ctx) -> Result<ToolExecutionResult> {
    let lines = args["lines"].as_u64().unwrap_or(50) as usize;
    let output = mgr.get_recent_output(terminal_id, lines)?;
    Ok(ToolExecutionResult { output, baseline: None, ... })
}
```

Usa `get_recent_output` (últimas N líneas, sin baseline). Diferente de `get_output_after` que sí requiere baseline.

---

## `run_app` — el flujo de apps long-running

Es la tool más sofisticada del módulo. ~150 líneas en `execute_run_app` (`executor.rs:597`) más helpers.

### Pasos

1. **Parse**: `command`, `port` (opcional — si no, lo extrae del comando), `wait_timeout_secs` (default 15, cap 60).

2. **Detección de docker detached**: `command.contains("docker") && contains("run") && contains("-d")`.

3. **Preflight port check** (`preflight_port_check`):
   - Extrae puertos del comando con `extract_ports_from_command`.
   - Verifica cada uno con `is_port_available` (IPv4 + IPv6).
   - Si hay alguno ocupado:
     - `detect_docker_on_ports` — `docker ps` para ver si es un container Venore previo.
     - `suggest_available_ports` — sugiere 3 puertos libres cercanos.
     - **BLOQUEA**: devuelve `ToolExecutionResult { success: false }` sin ejecutar nada.

4. **Ejecuta** en el PTY: snapshot baseline, write `{cmd}\r`.

5. **Espera arranque** según el caso:
   - **Docker detached**: `wait_for_output(terminal_id, baseline, 3)` — esperar a que aparezca el container ID.
   - **Foreground con puerto conocido**: `wait_for_port_listen(port, timeout_secs)` — polleo del puerto cada 500ms.
   - **Sin puerto**: `wait_for_output(terminal_id, baseline, timeout_secs)` — espera estabilidad de salida.

6. **Lee output**: `get_output_after(terminal_id, baseline, 30)`.

7. **Docker health check** (`check_docker_container_health`) si era detached:
   - `extract_container_id` del output (busca la última línea hex válida).
   - `docker inspect --format {{.State.Running}}` para ver si sigue vivo.
   - Si crashed: `docker logs --tail 50` y devuelve warning con esos logs.
   - Si vivo: `fetch_docker_logs_if_detached` + `check_port_mismatch` (compara puerto del comando con el real en logs vía regex `RE_LISTEN_PORT`).

8. **Verificación final de puerto**: si seguía `RUNNING` y hay puerto → `is_port_available(port)` otra vez. Si está libre = la app no arrancó realmente → `FAILED`.

9. **Output estructurado**:
   ```
   Status: RUNNING|FAILED
   Command: {command}
   URL: http://localhost:{port}   (si hay puerto)
   {warnings...}

   IMPORTANT: ... You MUST call check_health now ...  (si RUNNING)

   Terminal output (last 30 lines):
   {output}
   ```

### `extract_ports_from_command`

Tres regex precompilados:

| Regex | Captura | Casos |
|-------|---------|-------|
| `RE_DOCKER_PORT_MAP` | grupo 1 = host, grupo 2 = container | `-p 5173:5173`, `--publish 8080:80/tcp` |
| `RE_PORT_FLAG` | grupo 1 = puerto | `--port 3000`, `--port=3000` |
| `RE_SHORT_P_FLAG` | grupo 1 = puerto | `-p 3000` (sin colon, no-docker como `next dev -p 3000`) |

Resultado sorted + dedup.

### `is_port_available`

```rust
fn is_port_available(port: u16) -> bool {
    let timeout = Duration::from_millis(300);
    // IPv4
    if TcpStream::connect_timeout(&v4_localhost, timeout).is_ok() { return false; }
    // IPv6 — Node.js/Vite en Windows bindean a [::1]
    if TcpStream::connect_timeout(&v6_localhost, timeout).is_ok() { return false; }
    true
}
```

**Importante**: chequea IPv4 **y** IPv6. En Windows, Node/Vite suelen bindear solo a `[::1]`, así que chequear solo IPv4 daría falso negativo.

### `check_port_mismatch`

Compara puerto **container-side** del comando docker (grupo 2 de `RE_DOCKER_PORT_MAP`) vs puerto que realmente reporta la app en sus logs (matched por `RE_LISTEN_PORT`). Si no coinciden, devuelve warning con el comando corregido sugerido.

Caso típico: usuario hace `docker run -p 8080:8080 my-app` pero la app dentro escucha en `3000`. La tool detecta y avisa `docker run -p 8080:3000 my-app`.

### `check_health` — el cierre

Tool aparte, mismo file (`executor.rs:749`). Hace GET HTTP con `reqwest`, valida status code y opcionalmente contenido. Retry 3 veces con 1s entre intentos.

El prompt de `run_app` instruye explícitamente al LLM: *"You MUST call check_health now"*. La pareja `run_app` + `check_health` cubre arranque + verificación.

---

## Flujo completo end-to-end

Un `run_terminal_command` paso a paso:

```
1. LLM emite tool_call: run_terminal_command(command: "cargo test")
2. tool_dispatch llama resolve_or_spawn_terminal()
   → reusa o spawnea, devuelve terminal_id
   → inyecta terminal_id en ToolExecutionContext
3. execute_run_command():
   - baseline = mgr.line_counter(terminal_id)
   - mgr.write(terminal_id, "cargo test\r")
   - return { success, baseline }
4. El PTY corre el comando. La salida fluye al read-loop (4KB chunks):
   ├─► event "terminal:output" → usuario ve en vivo en xterm.js
   └─► mgr.append_output() → strip ANSI → buffer + counters
5. post_process_terminal():
   - wait_for_output(terminal_id, baseline, 15s) — estable o timeout
   - output = mgr.get_output_after(terminal_id, baseline, 50)
   - llm_messages.push(combined, truncado a límite de tool)
6. Siguiente turno del LLM: ve el output, decide qué hacer
```

---

## Decisiones de diseño relevantes

### Por qué PTY real (`portable-pty`) y no `std::process::Command`

- **El usuario ve lo mismo que la IA.** No hay dos "vistas" del comando.
- **Apps interactivas funcionan.** Prompts, REPLs, npm init, etc.
- **Colores ANSI.** xterm los renderiza; la IA recibe la versión sin ANSI.
- **Estado persistente.** `cd`, `export`, alias se mantienen entre comandos.

### Por qué strip ANSI para la IA pero raw para xterm

La IA no necesita códigos de escape — confunden y queman tokens. xterm sí los necesita para colores y cursor. El read-loop bifurca: raw a un lado, limpio al otro.

### Por qué `Arc<Mutex<>>` separados para reader y writer

El read-loop retiene su lock por minutos esperando output. Si fuera el mismo lock del writer, no se podría escribir mientras lee. Separados, el usuario puede teclear y la IA mandar comandos sin bloquear.

### Por qué baseline (line_counter) y no "leer todo desde el comando"

Tres razones:
1. **Eco**: la shell hace eco del comando — el baseline lo descarta (se incrementó al imprimirse).
2. **Output previo**: el buffer tiene líneas viejas; sin baseline las leeríamos otra vez.
3. **Concurrencia**: dos comandos solapados en la misma terminal no se interfieren.

Y `line_counter` es **monotónico** incluso cuando el buffer hace FIFO — `get_output_after` calcula el offset real usando `total - buffer.len()`.

### Por qué chequeo dual IPv4 + IPv6 en `is_port_available`

Node.js, Vite, Next.js en Windows suelen bindear solo a `[::1]`. Si solo chequeáramos IPv4, `preflight_port_check` no detectaría el conflicto y `wait_for_port_listen` se quedaría esperando para siempre.

### Por qué PowerShell y no cmd.exe en Windows

cmd.exe es la shell legacy de Windows y solo conoce su set propio de comandos (`cls`, `dir`, `copy`…). Comandos comunes como `clear`, `ls`, `cat`, `rm` — que cualquier desarrollador con experiencia Unix tipea sin pensar — fallan con `'X' is not recognized`. PowerShell 5.1+ los tiene todos como aliases nativos. Como PS 5.1 viene de fábrica en todas las versiones soportadas de Windows, no introducimos dependencia nueva.

### Por qué `TerminalTabContent` nunca se desmonta

Al minimizar el panel, las terminales se quedan con `invisible` (CSS) pero el componente sigue vivo. Si las desmontáramos, perderíamos los `useTerminal` (y sus instancias xterm), lo que reseteaba scrollback y reabriría listeners. Procesos largos quedarían huérfanos visualmente.

---

## Instrumentación: `VENORE_PTY_DEBUG`

`crates/venore-core/src/terminal/debug.rs`

Logger opt-in que captura todos los bytes que entran y salen de cualquier PTY. Útil para diagnosticar problemas de escape sequences, interacciones con ConPTY, o discrepancias entre lo que la shell emite y lo que xterm renderiza.

### Activación

```powershell
$env:VENORE_PTY_DEBUG="1"
# (arrancar Venore)
```

Cuando la env var **no** está, el logger es no-op: early return al inicio de `log()`, sin allocations ni I/O. Cero coste en producción.

### Path del archivo

Resolución (primer match gana):

1. `$VENORE_PTY_DEBUG_LOG` — override explícito
2. `%TEMP%/venore-dev/pty-debug.jsonl` — debug builds
3. `~/.venore/pty-debug.jsonl` — release builds

### Formato

Una línea JSON por evento, append-only:

```json
{"ts":"2026-05-27T05:12:46.532Z","terminal_id":"terminal-1","dir":"write","len":6,"hex":"63 6c 65 61 72 0d","text":"clear\\r"}
{"ts":"2026-05-27T05:12:46.540Z","terminal_id":"terminal-1","dir":"read","len":N,"hex":"1b 5b 32 4a 1b 5b 48 ...","text":"\\x1b[2J\\x1b[H..."}
```

Campos:

| Campo | Significado |
|-------|-------------|
| `ts` | ISO 8601 UTC con milisegundos |
| `terminal_id` | ID de la sesión PTY (mismo que en `TerminalSessionManager`) |
| `dir` | `"write"` (usuario o IA → PTY) o `"read"` (PTY → app) |
| `len` | Bytes en el chunk |
| `hex` | Hex dump separado por espacios |
| `text` | Representación printable: ASCII como tal, escape sequences como `\x1b`, control chars como `\r` / `\n` / `\t`, no-printable como `\xNN` |

### Puntos de captura

- **Writes**: `TerminalSessionManager::write()` (`manager.rs`). Cubre todos los caminos: usuario tipeando en xterm, IA mandando comandos vía `run_terminal_command`/`run_app`, etc.
- **Reads**: dentro de `start_terminal_read_loop` (`commands/terminal.rs`), justo después del `read()` del PTY master y antes del `String::from_utf8_lossy` que va a xterm.

Que el read se loggee **antes** de la conversión UTF-8 es deliberado: permite detectar bytes que se corrompen al convertir.

---

## Limitaciones conocidas

### `exit` puede no cerrar el tab automáticamente (Windows)

Cuando el usuario escribe `exit` en una sesión PowerShell, el comportamiento esperado es:

1. PowerShell termina
2. ConPTY cierra el master
3. Read-loop ve `Ok(0)` (EOF) → emite `terminal:dead`
4. Frontend remueve el tab

En la práctica, ConPTY en Windows tiene un problema documentado: cuando el proceso hijo (PowerShell) termina, la pipe del PTY master no siempre se cierra inmediatamente. El `read()` se queda bloqueado, el read-loop no detecta EOF, y el tab permanece visible aunque la shell ya no responda.

**Workaround actual**: cerrar el tab manualmente desde el botón × del TerminalTabBar (que llama `kill_terminal`).

**Fix futuro pendiente**: poll periódico de `child.try_wait()` en el read-loop. Si el child está muerto pero el read sigue bloqueado, romper el loop manualmente. Esto requiere meter un `tokio::select!` o un timeout corto en cada iteración.

### `strip_ansi_escapes` regex incompleto

`crates/venore-core/src/utils/string.rs`

El regex actual matchea SGR (color), cursor `H`/`G`/`J`/`K`, OSC con BEL, y modos privados `h`/`l`/`r`. **No matchea**:

- Cursor movement `A`/`B`/`C`/`D`/`E`/`F`
- Save/restore `s`/`u`
- Single-char escapes: `ESC c` (RIS), `ESC =`, `ESC >`, `ESC 7`, `ESC 8`
- DCS/SOS/PM/APC

Afecta solo al **buffer de la IA** — la IA puede recibir residuos de escape en su output. xterm (que recibe raw) no está afectado.

### Addons xterm.js no usados

Mejoras potenciales que no hemos cableado:

- `WebglAddon` — render por GPU (más rápido y nítido en pantallas HiDPI)
- `Unicode11Addon` — anchos correctos para emoji y CJK
- `SearchAddon` — Ctrl+F sobre el scrollback
- `SerializeAddon` — persistir scrollback al cerrar/reabrir el panel

---

## Archivos clave

| Archivo | Qué hay ahí |
|---------|-------------|
| `crates/venore-core/src/terminal/session.rs` | `TerminalSession`, spawn PTY, `build_shell_command` por OS, `resolve_powershell_path` |
| `crates/venore-core/src/terminal/manager.rs` | Singleton, buffers, line_counters, binding |
| `crates/venore-core/src/terminal/debug.rs` | Logger opt-in de bytes raw del PTY (`VENORE_PTY_DEBUG`) |
| `crates/venore-core/src/tools/definitions.rs` | `terminal_tools()` — 4 definiciones para el LLM |
| `crates/venore-core/src/tools/executor.rs` | Implementación de las 4 tools + helpers (port detection, docker health) |
| `crates/venore-core/src/error.rs` | `TerminalError`, `TerminalSessionNotFound`, `TerminalSpawnFailed` |
| `crates/venore-desktop/src/commands/terminal.rs` | Tauri commands + `start_terminal_read_loop` |
| `crates/venore-desktop/src/commands/dto/terminal.rs` | DTOs request/response y payloads de eventos |
| `crates/venore-desktop/src/commands/chat/helpers.rs` | `resolve_or_spawn_terminal` (session-bound vs unbound) |
| `crates/venore-desktop/src/commands/chat/tool_dispatch.rs` | `post_process_terminal` (wait_for_output + combine) |
| `crates/venore-desktop/ui/src/hooks/useTerminal.ts` | Bridge xterm ↔ Tauri, key handler, theme |
| `crates/venore-desktop/ui/src/components/workspace/TerminalPanel.tsx` | Panel multi-tab, ResizeObserver, listeners de eventos |
| `crates/venore-desktop/ui/src/stores/terminalStore.ts` | Estado Zustand: tabs, isOpen, activeTabId |
