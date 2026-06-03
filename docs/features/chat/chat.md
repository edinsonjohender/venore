# Chat

> Estado: implementado
> Módulo core: `crates/venore-core/src/chat/`
> Bridge Tauri: `crates/venore-desktop/src/commands/chat/`
> Stores UI: `crates/venore-desktop/ui/src/stores/chatStore.ts`, `chatSessionStore.ts`
> UI: `crates/venore-desktop/ui/src/components/workspace/panels/chat/`, `screens/ChatWindow.tsx`

## TL;DR

El chat es la interfaz conversacional con el agente LLM. Vive como **un panel** dentro del
workspace (no una pantalla), con barra de **tabs** estilo navegador, streaming en vivo vía
**eventos Tauri**, y persistencia en **SQLite**.

El estado está partido en **dos stores Zustand** con responsabilidades disjuntas:

- `chatSessionStore` — las **conversaciones (sesiones) y sus tabs**.
- `chatStore` — los **mensajes y el streaming de la sesión activa**.

Las sesiones siguen **persistencia lazy**: una conversación nueva es un *draft* en memoria
(sin fila en DB) hasta que se manda el primer mensaje. Así los chats vacíos nunca tocan la DB
ni aparecen en el historial.

---

## Arquitectura en una imagen

```
┌─ UI (React) ──────────────────────────────────────────────────────────┐
│  ChatPanel.tsx  (switch vista chat / history)                          │
│   ├─ ChatSessionTabs.tsx   ← barra de tabs (openChatTabs)              │
│   └─ ChatErrorBoundary                                                  │
│       ├─ ChatMessages.tsx  → ChatMessage.tsx                            │
│       │      ├─ ChatToolCall.tsx   (tool calls)                         │
│       │      ├─ ChatSubAgent.tsx   (sub-agentes)                        │
│       │      └─ ChatTaskList.tsx   (tareas del agente)                  │
│       └─ ChatInput.tsx     → ChatToolbar / overlays / attachments       │
│  ChatWindow.tsx            ← misma UI en ventana pop-out independiente  │
└───────────┬─────────────────────────────────────────▲──────────────────┘
            │ tauriApi.sendChatMessage(...)            │ eventos chat-*
            ▼                                          │
┌─ Tauri bridge (commands/chat/) ───────────────────────────────────────┐
│  stream.rs    send_chat_message → agentic_loop → emite eventos          │
│  session.rs   create/list/delete/rename, get_messages, snapshots…       │
│  actions.rs   approve_tool_call, respond_to_agent, approve_plan…        │
└───────────┬────────────────────────────────────────────────────────────┘
            ▼
┌─ Core (venore-core/src/chat/) ────────────────────────────────────────┐
│  ChatRepository  tablas chat_sessions / chat_messages (SQLite)          │
│  orquestador + context builder + compaction + guardrails                │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Los dos stores

La separación es la clave para razonar sobre bugs: un evento de stream **siempre** se filtra
por `activeSessionId` antes de tocar la UI.

### `chatSessionStore` — sesiones + tabs

| Campo | Origen | Para qué |
|-------|--------|----------|
| `sessions` | `loadSessions` (merge), `createSession`, `ensureDraft` | lista maestra (DB + draft) |
| `openChatTabs` | `createSession`, `openChatTab`, `closeChatTab`, `ensureDraft` | ids de tabs visibles |
| `activeSessionId` | `switchSession`, `createSession`, `ensureDraft` | sesión enfocada |
| `poppedOutSessionIds` | pop-out | tabs movidos a su propia ventana |
| `sessionHasMessages` | `sendMessage`, `chat-stream-done`, `switchSession` | ¿la sesión tiene contenido? |
| `sessionCache` | `switchSession`/`createSession` (snapshot) | estado in-memory por sesión |
| `chatView` | `setChatView` | `'chat'` \| `'history'` |

**Invariante:** todo id en `openChatTabs` existe en `sessions`. El tab bar lo asume
(`chatTabItems = openChatTabs.map(id => sessions.find(id))`). Romperla → tab invisible.

### `chatStore` — mensajes + streaming

`messages`, `isStreaming`, `currentStreamId`, `tokenUsage`, `providerInfo`, `error`, y el estado
de interacción del agente: `pendingConfirm`, `pendingAskUser`, `pendingPlan`, `tasks`,
`subAgents`, `snapshots`, `lastCompaction`.

Solo refleja **la sesión activa**. Los eventos de sesiones no-activas se desvían a
`sessionCache` (vía `updateSessionCache`) en vez de a la UI.

---

## Ciclo de vida de una sesión (persistencia lazy)

```
   abrir panel
       │  ensureDraft()
       ▼
  ┌─────────┐   primer mensaje    ┌──────────────┐   stream-done   ┌────────────┐
  │  DRAFT  │ ───createSession──► │  REAL (DB)   │ ──autoName────► │  NOMBRADA  │
  │ (memoria)│   (INSERT + swap)   │  + mensajes  │  (LLM title)    │            │
  └─────────┘                     └──────────────┘                 └────────────┘
       ▲                                                                  │
       └──────────────── GC borra vacías legacy / abortadas ◄────────────┘
```

- **DRAFT** — sesión en memoria con `id = DRAFT_SESSION_ID` (`'draft'`). Vive en `sessions` +
  `openChatTabs` como una real, así toda la maquinaria keyed-by-id funciona sin cambios. Sin
  fila en DB. Singleton: como máximo un draft. No se puede popear.
- **Materialización** — al primer envío, `getOrCreateSendableSession` llama a `createSession`,
  que hace el `INSERT` y **promociona** el draft en sitio: cambia `'draft'` → id real en
  `openChatTabs` y `sessions`, preservando la posición del tab.
- **Auto-naming** — al terminar el stream (`chat-stream-done`), `autoNameSession` pone un nombre
  truncado del primer mensaje y, en paralelo, pide un título al LLM (`generate_chat_title`).
- **GC** — `ChatRepository::delete_empty_sessions()` borra sesiones plain (no dev) sin mensajes;
  se llama best-effort al inicio de `list_sessions`.

### Acciones relevantes

| Acción | Comportamiento |
|--------|----------------|
| `ensureDraft()` | crea/activa el draft (idempotente, sin DB) |
| `getOrCreateEmptySession()` | botón "New Chat": reusa tab vacío o cae a draft. **No inserta.** |
| `getOrCreateSendableSession()` | path de envío: devuelve sesión real, materializando el draft |
| `createSession()` | único punto de `INSERT`; promociona el draft |
| `switchSession('draft')` | atajo sin llamada a DB (no hay mensajes que cargar) |
| `closeChatTab()` | si no quedan tabs → `ensureDraft` (nunca crea fila) |
| `deleteSession()` | guarda para draft; si no queda nada → `ensureDraft` |

> Decisión de UX (intencional): "New Chat" **reusa** el chat vacío actual en vez de crear otro
> tab. `getOrCreateEmptySession` detecta `!sessionHasMessages[activeSessionId]` y no hace nada.

---

## Tabs

`ChatSessionTabs.tsx` pinta `openChatTabs` (excluyendo los popeados). Al montar, un único efecto
secuencial: `await loadSessions(projectId)` → leer estado fresco → `getOrCreateEmptySession`. La
secuencia evita la carrera en la que el `set` de `loadSessions` pisaba la sesión recién creada y
dejaba el tab bar vacío.

`loadSessions` hace **merge** (no overwrite) al refrescar `sessions`: conserva cualquier sesión
referenciada por un tab abierto que aún no esté en la lista de DB (p.ej. el draft, o una sesión
creada durante el `await`). Esto sostiene la invariante `openChatTabs ⊆ sessions`.

Etiqueta del tab: `sessionHasMessages[id] ? session.name : "New Chat"`.

---

## Streaming (eventos Tauri)

El streaming no es request/response: `sendChatMessage` devuelve rápido y el backend emite
eventos. Los listeners se registran **una vez** en `setupStreamListeners()` (al cargar el módulo,
así todas las ventanas los reciben). **Cada listener filtra por `session_id === activeSessionId`**;
si no coincide, desvía a `sessionCache`.

| Evento | Efecto en la UI |
|--------|-----------------|
| `chat-stream-delta` | append de texto al último mensaje |
| `chat-stream-done` | cierra streaming, fija `tokenUsage`/`providerInfo`, auto-nombra |
| `chat-stream-error` | banner de error |
| `chat-tool-call` | añade tool call (estado `running`) al último mensaje assistant |
| `chat-tool-result` | actualiza el tool call a `completed`/`error` con su output |
| `chat-tool-confirm` | overlay de confirmación (`pendingConfirm`) |
| `chat-ask-user` | overlay de pregunta del agente (`pendingAskUser`) |
| `chat-plan-ready` | overlay de plan (`pendingPlan`) |
| `chat-task-update` | lista de tareas del agente (`tasks`) |
| `chat-sub-agent` | tarjeta de sub-agente (`subAgents`) |
| `chat-snapshot` | asocia `commitHash` a un tool call (para revert) |
| `chat-compacted` | aviso de compactación de contexto (`lastCompaction`) |
| `chat-popout-closed` | reabsorbe un pop-out cerrado en la ventana principal |

**Watchdog de inactividad:** si no llega actividad de stream en 3 min, se aborta y se muestra
error `INACTIVITY_TIMEOUT`.

---

## Cache por sesión y cambio de tab

Al cambiar de tab, `switchSession` hace snapshot del estado de mensajes de la sesión saliente en
`sessionCache` (solo si es real y tiene contenido) y lo restaura al volver — preserva streaming,
tool calls y overlays sin recargar de DB. La entry se **consume** (borra) al restaurarse.

`_switchEpoch` es un contador que se incrementa en cada switch; un `loadMessages` asíncrono que
termine tras otro switch se **descarta** (evita pintar mensajes de la sesión equivocada).

---

## Pop-out

Un tab se puede mover a su propia ventana OS (`ChatWindow.tsx`):

1. Snapshot completo a `localStorage` (síncrono) **antes** de abrir la ventana.
2. `openChatWindow` crea la ventana; el tab se marca `poppedOut`.
3. La ventana hidrata desde `localStorage` (o DB como fallback) y `reconnectToActiveStream`
   re-engancha el stream activo (los canales oneshot —confirmaciones— no cruzan ventanas y se
   limpian).
4. Al cerrarla, emite `chat-popout-closed`; la principal reabsorbe el estado.

El **draft no se puede popear** (no tiene fila persistente que reenganchar).

---

## Dev sessions

Tabs con icono `GitBranch`: chats ligados a una sesión de desarrollo (branch/worktree). Tienen
`dev_session_id` y se crean vía `get_or_create_dev_session_chat` (persistencia **eager**, no
lazy — están atados a una rama). Arrastran canvas tab + terminal + approvals, que se limpian en
`closeChatTab` / `switchSession`. El GC nunca las toca (`dev_session_id IS NULL` en el filtro).

---

## Características de entrada (`ChatInput`)

- **Attachments** — diálogo de archivo, drag-drop y paste de imágenes; viajan como base64.
- **Context modules** — módulos de código adjuntados como contexto (`@`), por path.
- **AI-connections (✨)** — knowledge nodes / hexágonos / módulos fijados desde sus paneles;
  resueltos server-side desde el registro de conexiones.
- **Skills** — paleta con `/` (`listSkills`): `/commit`, `/fix`, `/test`, `/review`, `/explain`.
- **Knowledge feature** — en tabs de tipo knowledge, el `featureId` activo se envía con el mensaje.

---

## Interacción del agente

| Mecanismo | Evento → estado | UI |
|-----------|-----------------|----|
| Confirmar tool | `chat-tool-confirm` → `pendingConfirm` | overlay; `approveToolCall(approved, allowSession)` |
| Preguntar al usuario | `chat-ask-user` → `pendingAskUser` | overlay; `respondToAgent` |
| Aprobar plan | `chat-plan-ready` → `pendingPlan` | overlay; `approvePlan` |
| Tareas | `chat-task-update` → `tasks` | `ChatTaskList` |
| Sub-agentes | `chat-sub-agent` → `subAgents` | `ChatSubAgent` |
| Snapshots / revert | `chat-snapshot` → `commitHash` | `revertToSnapshot(devSessionId, hash, msgId)` |

`approveToolCall` resuelve la clave de aprobación con prioridad `dev_session_id → chat_session_id`
para que el "always-allow" persista también en modo Knowledge (sin dev session).

---

## Persistencia (SQLite)

`ChatRepository` (`crates/venore-core/src/chat/repository.rs`):

- **`chat_sessions`** — `id, name, project_id, dev_session_id, created_at, updated_at`.
- **`chat_messages`** — `id, session_id, role, content, provider, model, prompt_tokens,
  completion_tokens, created_at, attachments_json`. FK `session_id → chat_sessions(id)
  ON DELETE CASCADE`.
- Tablas auxiliares para snapshots y tool calls (revert, `get_session_activity`).

> La FK obliga a que la fila de sesión exista **antes** de guardar mensajes. La materialización
> lazy lo respeta: el envío hace `await createSession()` (INSERT) antes de `sendMessage`.

### GC de sesiones vacías

`delete_empty_sessions()` borra sesiones plain sin mensajes. Guardas:

- `dev_session_id IS NULL` — nunca toca dev-sessions.
- `datetime(created_at) < datetime('now','-1 minute')` — no borra recién creadas en vuelo.
  Se usa `datetime(...)` en ambos lados porque `created_at` es RFC3339 y la comparación de string
  plana sería incorrecta (separador `T` vs espacio).
- `id NOT IN (SELECT DISTINCT session_id FROM chat_messages)` — solo sin mensajes.

Best-effort al inicio de `list_sessions`: limpia las vacías legacy en cada carga sin un comando
ni scheduler aparte.

---

## Comandos Tauri (resumen)

| Comando | Para qué |
|---------|----------|
| `send_chat_message` | inicia el turno; emite los eventos `chat-*` |
| `stop_chat_stream` | aborta el stream activo |
| `create_chat_session` / `list_chat_sessions` / `delete_chat_session` / `rename_chat_session` | CRUD de sesiones |
| `get_or_create_dev_session_chat` | chat ligado a una dev session |
| `get_chat_messages` / `get_chat_snapshots` / `get_session_activity` | carga histórica |
| `approve_tool_call` / `respond_to_agent` / `approve_plan` | interacción del agente |
| `get_session_stream_status` | reconectar stream tras pop-out |
| `revert_to_snapshot` | volver a un commit de snapshot |
| `generate_chat_title` | título LLM para auto-naming |
| `get_chat_context_options` | módulos con `.context.md` para el selector |
| `open_chat_window` / `clear_session_approvals` | pop-out / limpieza de approvals |

---

## Invariantes y puntos frágiles

1. **`openChatTabs ⊆ sessions`** — si se rompe, el tab no se pinta. Sostenida por el merge de
   `loadSessions` y la promoción del draft en `createSession`.
2. **Filtrado por `activeSessionId`** en cada listener — fuente de bugs de "mensaje en el tab
   equivocado" si se omite.
3. **El draft nunca llega a la DB** — todo path de persistencia (`createSession`,
   `getOrCreateSendableSession`) lo materializa primero; el resto lo trata como local.
4. **`_switchEpoch`** descarta loads obsoletos en switches rápidos.
5. **El embedding/stream de sesiones no-activas** desvía a `sessionCache`, no a la UI.
