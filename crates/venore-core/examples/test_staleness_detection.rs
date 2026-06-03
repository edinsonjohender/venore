//! Example: Staleness Detection
//!
//! Demonstrates how to detect when .context.md files are outdated.
//!
//! Usage:
//! ```bash
//! cargo run --example test_staleness_detection
//! ```

use venore_core::context::{calculate_code_hash, is_context_stale, HashCache};
use std::fs;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n================================================================================");
    println!("STALENESS DETECTION TEST");
    println!("================================================================================\n");

    // Create temporary directory
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    let code_path = project_root.join("example.ts");
    let context_path = project_root.join(".context").join("example.context.md");

    // Create .context directory
    fs::create_dir_all(context_path.parent().unwrap())?;

    println!("Step 1: Create source code file");
    let original_code = r#"
export function add(a: number, b: number): number {
    return a + b;
}
"#;
    fs::write(&code_path, original_code)?;
    let original_hash = calculate_code_hash(original_code);
    println!("  Code hash: {}", original_hash);

    println!("\nStep 2: Create .context.md with frontmatter");
    let context_content = format!(
        r#"---
name: "example"
type: component

analyzed:
  at: "2026-01-22T00:00:00Z"
  agent: "test-agent"
  model: "test-model"
  provider: "test"
  codeHash: "{}"
  stale: false
---

# Example Module

This is a test context file.
"#,
        original_hash
    );
    fs::write(&context_path, context_content)?;
    println!("  Created: {}", context_path.display());

    println!("\nStep 3: Check staleness (should be NOT stale)");
    let is_stale = is_context_stale(&code_path, &context_path, original_code)?;
    println!("  Is stale? {}", if is_stale { "YES ⚠️" } else { "NO ✅" });
    assert!(!is_stale, "Context should NOT be stale");

    println!("\nStep 4: Modify source code");
    let modified_code = r#"
export function add(a: number, b: number): number {
    return a + b;
}

export function subtract(a: number, b: number): number {
    return a - b;
}
"#;
    fs::write(&code_path, modified_code)?;
    let modified_hash = calculate_code_hash(modified_code);
    println!("  New hash: {}", modified_hash);
    println!("  Hash changed: {}", original_hash != modified_hash);

    println!("\nStep 5: Check staleness (should be STALE now)");
    let is_stale = is_context_stale(&code_path, &context_path, modified_code)?;
    println!("  Is stale? {}", if is_stale { "YES ⚠️" } else { "NO ✅" });
    assert!(is_stale, "Context SHOULD be stale after code change");

    println!("\n================================================================================");
    println!("HASH CACHE TEST");
    println!("================================================================================\n");

    println!("Step 6: Create and populate cache");
    let mut cache = HashCache::new();
    cache.update(&code_path, original_hash.clone(), &context_path);
    println!("  Cache entries: {}", cache.len());

    println!("\nStep 7: Save cache to disk");
    cache.save(project_root)?;
    let cache_path = project_root.join(".venore").join("hash-cache.json");
    println!("  Saved to: {}", cache_path.display());
    println!("  File exists: {}", cache_path.exists());

    println!("\nStep 8: Load cache from disk");
    let loaded_cache = HashCache::load(project_root)?;
    println!("  Loaded entries: {}", loaded_cache.len());

    println!("\nStep 9: Check staleness using cache");
    let is_cache_stale = loaded_cache.is_stale(&code_path, &modified_hash);
    println!("  Is stale (cache)? {}", if is_cache_stale { "YES ⚠️" } else { "NO ✅" });
    assert!(is_cache_stale, "Cache should detect stale context");

    println!("\nStep 10: Update cache with new hash");
    let mut updated_cache = loaded_cache;
    updated_cache.update(&code_path, modified_hash.clone(), &context_path);
    let is_cache_stale_after = updated_cache.is_stale(&code_path, &modified_hash);
    println!("  Is stale after update? {}", if is_cache_stale_after { "YES ⚠️" } else { "NO ✅" });
    assert!(!is_cache_stale_after, "Cache should NOT detect stale after update");

    println!("\n================================================================================");
    println!("SUCCESS - All staleness detection tests passed! ✅");
    println!("================================================================================\n");

    Ok(())
}
