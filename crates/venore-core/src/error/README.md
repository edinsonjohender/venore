# Error System

Centralized error system for Venore.

## What it is

The `VenoreError` enum, covering every possible system error using `thiserror`.

## What it's for

- Consistent errors across the codebase
- Clear error messages
- Easy propagation with the `?` operator
- Automatic conversion between error types

## When to use it

Whenever you need to return an error in venore-core or venore-cli.

## How

```rust
use venore_core::error::{VenoreError, Result};

pub fn my_function() -> Result<String> {
    let path = PathBuf::from("./missing");

    // Automatic propagation
    let content = std::fs::read_to_string(&path)?;

    Ok(content)
}
```

## Error types

- **IO**: file system errors
- **Parse**: AST parsing errors
- **Config**: configuration errors
- **NotFound**: resource not found
- **InvalidInput**: validation failed

## Re-export

Import from the crate root:
```rust
use venore_core::{VenoreError, Result};
```
