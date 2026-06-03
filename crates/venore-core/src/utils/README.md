# Utils

Core utilities for path manipulation, string operations, and validation.

## What it is

A collection of reusable helper functions.

## What it's for

Avoid duplicating common logic (path normalization, validation, etc.).

## Modules

### Path utils (`path.rs`)

**Purpose**: cross-platform path normalization.

**Functions**:
```rust
use venore_core::utils::path::{normalize_path, get_relative_path};

// Normalize separators (\ → / on Windows)
let normalized = normalize_path(&PathBuf::from("src\\auth\\index.ts"));

// Get a relative path
let relative = get_relative_path(&project_root, &file_path);
```

### String utils (`string.rs`)

**Purpose**: common string operations.

**Functions**:
```rust
use venore_core::utils::string::truncate;

// Truncate a string to N chars
let short = truncate("Long text here...", 10);
```

### Validation (`validation.rs`)

**Purpose**: validate inputs before processing them.

**Functions**:
```rust
use venore_core::utils::validation::{is_valid_extension, is_ignored_pattern};

// Validate extension
if is_valid_extension(".ts") { ... }

// Check whether a path should be ignored
if is_ignored_pattern(&path, &["node_modules", "dist"]) { ... }
```

## Re-export

```rust
use venore_core::utils;
```
