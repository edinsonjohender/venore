# Venore CLI - Context Generation Wizard

Interactive CLI for generating project contexts, based on the V1 flow.

## 🎯 Purpose

Testing and development tool for the context generator. Lets you run the full project-analysis pipeline through an interactive 4-step wizard, similar to V1, with automatic generation of `.context.md` files using an LLM.

## 🚀 Usage

### Wizard command (interactive)

```bash
cargo run -p venore-cli -- wizard
```

This command starts an interactive wizard that walks you through 4 steps.

### Requirements

- API key configured in `.env`:
  - `GEMINI_API_KEY` (recommended — free)
  - `ANTHROPIC_API_KEY`
  - `OPENAI_API_KEY`

## 📊 Sample output

```
Step 1 of 4: Project Context
Step 2 of 4: Analysis Rules
Step 3 of 4: Analyzing project...

✓ Files: 560
✓ Modules: 86

Step 4 of 4: Generate Contexts

Select modules to generate contexts for:
[Use Space to select, Enter to confirm]

🤖 Using gemini with model gemini-2.5-flash
🔍 Testing connection... ✓ Connected (920ms)

📊 Building full analysis...
✓ Analysis built

[========================================] 86/86 Done!

✓ Generated: 86
```

## 🔄 Current status (2026-01-23)

### ✅ Implemented (Steps 1-4)
- **Step 1**: Project Context (name, description, team, goals)
- **Step 2**: Analysis Rules (depth level, layers, exclusions)
- **Step 3**: Analysis (scan + parse + module detection)
- **Step 4**: Context Generation (LLM-powered .context.md generation)

### ⏳ Pending (Steps 5-8)
- Steps 5-8: Review, refinement, batch operations (future)

## 🎯 Features

### Rate-limit handling
- **Smart per-module delay**:
  - Gemini: 4s (15 req/min → safe)
  - Anthropic: 2s (30 req/min)
  - OpenAI: 2s (30 req/min)
- **Auto rate-limit detection**: bumps delay to 30s when a 429 is detected
- **Automatic retry**: 3 attempts with exponential backoff

### Multi-provider support
- Gemini 2.0 Flash Exp (recommended — free during preview)
- Anthropic Claude (Haiku/Sonnet)
- OpenAI GPT-4o Mini

### Depth levels
- **Minimal**: ~500-800 tokens, no code snippets
- **Normal**: ~1.5-2K tokens, 1 snippet
- **Detailed**: ~3-4K tokens, 3 snippets
- **Expert**: ~5-8K tokens, 5 snippets

## 📈 Testing results

Test project: Excalidraw (560 files, 86 modules)

| Metric | Without delay | With delay |
|---------|---------------|------------|
| Modules selected | 85 | 86 |
| Generated successfully | 67 (79%) | **86 (100%)** ✅ |
| Failures | 18 (21%) | **0 (0%)** ✅ |
| Total tokens | 134K | 138K |
| Estimated time | ~5 min | ~11 min |

**Conclusion**: a 4s delay completely eliminates rate-limit failures.

## ⚠️ Known limitations

### Export detection
The AST parser currently **does NOT detect**:
- ❌ `export * from "./file"` — full re-exports
- ❌ `export { foo } from "./file"` — selective re-exports

**Impact**: modules that rely on re-exports (such as `@excalidraw/math`) may show up with no detected exports in the analysis.

**Current workaround**: the LLM prompt includes instructions to infer exports from code snippets when the analysis fails.

**It only detects**:
- ✅ `export function foo()`
- ✅ `export class Foo`
- ✅ `export const foo = ...`

**Affected file**: `crates/venore-core/src/analysis/ast_parser.rs:467-517`

### Generated context quality

Based on analysis of 7 contexts from the Excalidraw project:

| Quality | Count | %  |
|---------|-------|---|
| ⭐ 9-9.5/10 | 3 | 43% |
| ⭐ 7.5-8.5/10 | 3 | 43% |
| ⚠️ 4-7/10 | 1 | 14% |

**Average**: 8/10

**Common issues**:
1. React hooks (`useX`) occasionally categorized as constants instead of functions.
2. Modules with no detected exports produce generic contexts (mitigated with an improved prompt).

## 🗂️ Output structure

Contexts are generated under `.context/` folders:

```
my-project/
├── packages/
│   └── my-module/
│       ├── .context/
│       │   └── my-module.context.md  ← Generated
│       ├── index.ts
│       └── ...
```

Each `.context.md` includes:
- YAML metadata (tokens, model, hash, timestamp)
- Quick Summary
- Purpose
- API Reference
- Architecture
- Usage Examples
- Notes

---

**Last updated**: 2026-01-23
