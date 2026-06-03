# Analysis Module

Code-analysis pipeline that produces structured context.

## Modules

### 1. File Scanner (`file_scanner.rs`)

**What it is**: scans and lists files in the project.

**What it's for**: filter files by extension, ignore patterns, get basic metadata.

**When**: first pipeline step. Runs before parsing.

**How**:
```rust
use venore_core::analysis::file_scanner::{ScanConfig, scan_directory};

let config = ScanConfig {
    project_path: PathBuf::from("./my-project"),
    target_extensions: vec!["ts".into(), "tsx".into()],
    ignore_patterns: vec!["node_modules".into(), "dist".into()],
    max_file_size_kb: 500,
};

let result = scan_directory(config)?;
// result.files holds the list of files found
```

**Output**: `ScanResult` with file list, total size, duration.

---

### 2. AST Parser (`ast_parser.rs`)

**What it is**: code parser using Tree-sitter to extract symbols.

**What it's for**: extract functions, classes, interfaces, enums, types, imports, and exports.

**When**: second step. Runs after the scan, file by file.

**How**:
```rust
use venore_core::analysis::ast_parser::{ParseConfig, Language, parse_file};

let config = ParseConfig {
    file_path: PathBuf::from("./src/auth.ts"),
    language: Language::TypeScript,
    content: std::fs::read_to_string("./src/auth.ts")?,
};

let result = parse_file(config)?;
// result.symbols holds functions, classes, etc.
// result.imports holds detected imports
// result.exports holds detected exports
```

**Output**: `ParseResult` with symbols, imports, exports.

**Supports**: TypeScript, TSX, JavaScript, JSX.

---

### 3. Module Detector (`module_detector.rs`)

**What it is**: detects modules and their dependencies.

**What it's for**: group files into logical modules and compute inter-module dependencies.

**When**: third step. Runs after parsing, using the import statements.

**How**:
```rust
use venore_core::analysis::module_detector::{DetectorConfig, detect_modules};

let config = DetectorConfig {
    files: scan_result.files,
    parse_results,
    project_root: PathBuf::from("./my-project"),
};

let result = detect_modules(config)?;
// result.modules holds detected modules
// result.orphan_files holds files that don't belong to any module
```

**Output**: `DetectionResult` with modules, dependencies, and orphan files.

**Module criteria**:
- Folder with `index.ts/tsx/js/jsx`
- Folder with 3+ related files
- Files with significant exports

---

### 4. Analysis Output (`analysis_output.rs`)

**What it is**: consolidated structure to feed the LLM.

**What it's for**: produce a structured payload with everything needed to generate context.

**When**: final pipeline step. Consolidates all the information.

**How**:
```rust
use venore_core::analysis::{AnalysisOutput, AnalysisConfig, DepthLevel};

let config = AnalysisConfig {
    project_root: PathBuf::from("./my-project"),
    scan_result,
    parse_results,
    modules,
    depth_level: DepthLevel::Normal,
};

let output = AnalysisOutput::build(config)?;
```

**Output**: `AnalysisOutput` with:
- Repository info (name, language, technologies)
- Modules with:
  - Architecture (dependencies, dependents, external_deps)
  - Symbols (exports, all)
  - Imports (module, items, file)
  - Code snippets (per depth level)
  - Files

**Depth levels**:
- `Minimal`: no code snippets
- `Normal`: 1 snippet (~100 chars)
- `Detailed`: 3 snippets (~300 chars)
- `Expert`: 5 snippets (~500 chars)

---

## Full pipeline

```
1. File Scanner
   ↓ (file list)
2. AST Parser (per file)
   ↓ (symbols, imports, exports)
3. Module Detector
   ↓ (modules, dependencies)
4. Analysis Output
   ↓ (structure for the LLM)
5. LLM Integration (PENDING — TASK-004)
   ↓ (generated context)
6. Context Writer (PENDING — TASK-005)
```

---

## Testing

See `crates/venore-cli` for an LLM-free testing tool:

```bash
# Full pipeline
venore-cli modules ./my-project

# Analysis output only
venore-cli analysis-output ./my-project --depth expert --format json
```
