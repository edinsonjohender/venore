# Currents + Logbook Semantic Search

> Estado: **implementado (v1)** — 2026-06-01
> Módulo core: `crates/venore-core/src/ocean/currents/`, `crates/venore-core/src/rag/logbook_repository.rs`, `crates/venore-core/src/rag/logbook.rs`
> Búsqueda: `crates/venore-core/src/rag/searcher.rs` (`search_hybrid` genérico)
> Tool del LLM: `search_logbook` en `crates/venore-core/src/tools/executor.rs`
> Bridge Tauri: `crates/venore-desktop/src/commands/currents.rs`
> UI: `crates/venore-desktop/ui/src/components/ocean/CurrentOverlay.tsx`

## TL;DR

En modo Knowledge la IA buscaba en los logbooks (secciones markdown de los nodos) con
**substring grep** — solo coincidencia literal. Ahora usa **búsqueda híbrida** (FTS5 +
embeddings + RRF), igual que la búsqueda de código. Esto cierra la única brecha objetiva del
modo Knowledge frente a Mem0/Zep/GBrain.

El indexado lo hace **Currents (Corrientes)**: un sistema nuevo de *trabajadores pasivos que
navegan el Ocean de forma visible*, cada uno con su propio cursor y tarea. La primera
corriente es la **Index Current**: al abrir el proyecto recorre nodo a nodo, detecta por
hash qué secciones cambiaron, y reindexa solo esas. El embedding (lento) corre en una tarea
aparte, coalescida por proyecto, para no bloquear el cursor.

> Nada del Ocean visual (canvas, islas, faros, conexiones) cambió. El rover de node-states
> (halos `Overflow`) sigue intacto — Currents es un sistema **hermano e independiente**.

## Arquitectura

```
┌─ Ocean (fuente de verdad del conocimiento) ──────────────────────────┐
│   OCEAN_LAYOUTS singleton → KnowledgeNodeData.sections: Vec<NodeSection>│
└───────────────────────────┬───────────────────────────────────────────┘
        ensure_currents_started(default_currents())  (en initialize_ocean_layout)
                            ▼
┌─ ocean/currents/ (core, sin dependencia de rag) ─────────────────────┐
│   runner.rs   un tokio task / proyecto, round-robin entre corrientes  │
│               trait Current { visit(ctx) -> Vec<CurrentTask> }         │
│               cada corriente: cursor + pending propios (no toca dirty) │
│   traversal.rs  nearest_pending() — selección "nodo más cercano"       │
│   index.rs    IndexCurrent → emite CurrentTask::IndexLogbookNode       │
└───────────────────────────┬───────────────────────────────────────────┘
            CurrentEvent por mpsc  { Progress | Task }
                            ▼
┌─ commands/currents.rs (desktop — aquí vive el acople ocean↔rag) ─────┐
│   Progress → emite Tauri `ocean-current-progress`                      │
│   Task(IndexLogbookNode) → lee secciones (with_service),               │
│        rag::index_logbook_node() (diff por hash),                      │
│        embed coalescido por proyecto (EMBEDDING_IN_FLIGHT)             │
└───────────────────────────┬───────────────────────────────────────────┘
                            ▼
┌─ rag/ (core) ────────────────────────────────────────────────────────┐
│   LogbookRepository  tablas logbook_chunks / _fts / _embeddings        │
│   logbook.rs  index_logbook_node, embed_logbook_chunks, remove_node    │
│   searcher.rs  search_hybrid(dyn HybridSearchable) ← code + logbook    │
└────────────────────────────────────────────────────────────────────────┘
                            ▲
        search_logbook tool (executor.rs) → search_logbook_hybrid
                            (fallback a grep si no hay índice / 0 hits)
```

## Decisiones de diseño (implementadas)

1. **Tablas separadas** `logbook_chunks` / `logbook_chunks_fts` / `logbook_embeddings` — sin
   FK a `rag_files`. Evita contaminar la búsqueda de código y evita filas sintéticas. Columna
   `content_hash = SHA256(name + "\0" + content)` para detección de cambios (un rename también
   re-embebe). El FTS5 lleva trigger **AFTER UPDATE** además de insert/delete porque las
   secciones se upsertan in-place (a diferencia de `rag_*` que borra-y-reinserta).
2. **Búsqueda híbrida genérica:** `search_hybrid(repo: &dyn HybridSearchable, …)` con el RRF
   `merge_results(0.4, 0.6)` en UN solo sitio. `RagRepository` y `LogbookRepository`
   implementan el trait; `search_code_hybrid` y `search_logbook_hybrid` son wrappers delgados.
3. **`ocean` NO importa `rag`.** La corriente emite una intención serializable
   (`CurrentTask::IndexLogbookNode`); el bridge desktop (que conoce ambos) la ejecuta.
4. **El embedding no bloquea el cursor.** La corriente solo detecta+encola; el embed corre en
   `tokio::spawn` coalescido por proyecto. Degrada con gracia sin API key (FTS-only + `warn!`).
5. **Opción Y (al lado):** el rover de node-states no se tocó. Plegarlo a Currents queda
   diferido (gate = tests verdes).
6. **Frescura ≤ 5 min:** una sección editada se reindexa en el siguiente barrido
   (`CURRENTS_REFRESH = 5min`) o al reabrir. Sin hook desde las mutaciones (diferido).

## Estructura de datos

`logbook_chunks.id = NodeSection.id` → reindexar una sección sobrescribe su chunk
(idempotencia sin tabla extra). `node_id` se guarda para borrar por nodo y para etiquetar los
resultados (`relative_path = "logbook/{node_id}"` en la proyección a `RagChunk`).

## Verificación

- **Tests unitarios (core, `sqlite::memory:`):**
  - `rag::logbook_repository` (8): upsert/search, update in-place re-sincroniza FTS, hash,
    ids por nodo + delete_node, delete_chunk, embeddings pending/has, drop de embedding stale
    al cambiar contenido, search_by_embedding, aislamiento por proyecto.
  - `rag::logbook` (5): inserta todo y luego salta sin cambios, reindexa al editar, reindexa
    en rename, borra sección eliminada, remove_node.
  - `ocean::currents::traversal` (5) + `ocean::currents::index` (4): selección nearest, filtro
    de variantes de código, emite intención solo para nodos de conocimiento con secciones.
- **Regresión del rover:** `scanner.rs` / `saturation.rs` / `states.rs` sin cambios → `Overflow`
  y `ocean-state-changed` idénticos.
- **Compila:** `venore-core` (983 lib tests verdes), `venore-desktop`, UI `tsc --noEmit`.
- **Manual:** abrir proyecto Knowledge → ver la corriente (cursor verde) barrer el Ocean →
  preguntar en el chat por significado (no keyword) y confirmar que encuentra la sección.
  Sin API key de embeddings → sigue funcionando por FTS.

## Diferido (no en v1)

- Plegar el rover de node-states a Currents.
- Hook de reindex inmediato al editar (frescura < 5 min).
- Feature de "corrientes creadas por el usuario".
- `get_current_status` como comando Tauri (hay `current_snapshot` en core listo para exponer).
