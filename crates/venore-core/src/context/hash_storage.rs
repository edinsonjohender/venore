//! Portable per-module code-hash snapshots at `<project_path>/.venore/code-hashes.json`.
//!
//! Each analyzed module gets a SHA-256 fingerprint stored on disk. When
//! another dev clones the repo, comparing the stored fingerprint against a
//! freshly computed one tells us which modules' code has drifted from the
//! persisted `project_memory` / `module_layers` snapshots — i.e. which
//! modules are "stale" relative to the committed analysis.
//!
//! Owner of `.venore/code-hashes.json` — no other module reads or writes
//! that path directly.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::atomic_json;
use crate::{Result, VenoreError};

use super::hash::{calculate_module_fingerprint, calculate_module_hash};

const VENORE_DIR: &str = ".venore";
const HASHES_FILE: &str = "code-hashes.json";

/// v2 adds the cheap `total_size` / `max_mtime` fingerprint fields alongside the
/// existing `file_count`, enabling a stat-only short-circuit before the SHA-256.
/// v1 files (missing those fields) still load — serde defaults them to 0, which
/// never matches a real subtree, so the first check falls through to the hash
/// (correct, just not yet optimized) until the entry is re-snapshotted.
const CURRENT_SCHEMA_VERSION: u32 = 2;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// One stored module fingerprint.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModuleHashEntry {
    pub module_name: String,
    pub module_path: String,
    pub code_hash: String,
    pub file_count: u32,
    /// Cheap filesystem fingerprint (with `file_count`): sum of file sizes in
    /// bytes. Lets a re-scan skip the SHA-256 when nothing moved. Defaults to 0
    /// for v1 snapshots, which forces one content hash until re-snapshotted.
    #[serde(default)]
    pub total_size: u64,
    /// Newest mtime across the module subtree, unix seconds. Part of the
    /// fingerprint. Defaults to 0 for v1 snapshots.
    #[serde(default)]
    pub max_mtime: i64,
    /// RFC3339 timestamp of when this hash was computed.
    pub computed_at: String,
}

/// On-disk envelope. `schema_version` at the top, entries sorted by
/// `module_name` for deterministic git diffs.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CodeHashesFile {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    generated_at: String,
    modules: Vec<ModuleHashEntry>,
}

/// Diff between a stored hash and the current code on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleModule {
    pub module_name: String,
    pub module_path: String,
    /// What the stored snapshot claims.
    pub stored_hash: String,
    /// What the current source tree hashes to. `"sha256-MISSING"` when the
    /// module directory no longer exists on disk.
    pub current_hash: String,
    /// True when `current_hash == "sha256-MISSING"`. Lets the UI render a
    /// distinct "module deleted" badge vs a generic "modified" one.
    pub missing_on_disk: bool,
}

pub fn path_for(project_path: &Path) -> PathBuf {
    project_path.join(VENORE_DIR).join(HASHES_FILE)
}

pub fn exists(project_path: &Path) -> bool {
    path_for(project_path).exists()
}

/// Load stored hashes. `Ok(None)` if file missing or corrupted (the corrupt
/// file is renamed to `code-hashes.json.corrupt` so a follow-up save can
/// recreate a clean one).
pub fn load(project_path: &Path) -> Result<Option<Vec<ModuleHashEntry>>> {
    let path = path_for(project_path);
    let envelope: Option<CodeHashesFile> = atomic_json::read_or_backup_corrupt(&path)?;
    Ok(envelope.map(|env| env.modules))
}

/// Save hashes atomically. Sorts by `module_name` before writing so the
/// JSON diff stays minimal when only one module changes.
pub fn save(project_path: &Path, entries: &[ModuleHashEntry]) -> Result<()> {
    let mut sorted: Vec<ModuleHashEntry> = entries.to_vec();
    sorted.sort_by(|a, b| a.module_name.cmp(&b.module_name));

    let envelope = CodeHashesFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        modules: sorted,
    };
    atomic_json::write_atomic(&path_for(project_path), &envelope)
}

pub fn delete(project_path: &Path) -> Result<()> {
    let path = path_for(project_path);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|e| {
        VenoreError::FileWriteError(format!("Failed to delete {}: {}", path.display(), e))
    })
}

/// Compute fresh hashes for every module the snapshot covers and compare
/// against the stored fingerprints. Returns the modules whose hash drifted.
///
/// Modules whose directory has been deleted are reported with
/// `missing_on_disk = true`. Modules added since the snapshot are NOT
/// reported — they have no stored hash, so by definition they're not stale.
/// The wizard (or a future "re-snapshot" command) is the right place to
/// extend coverage.
///
/// **Deepest-only filtering**: a module's hash by design walks its entire
/// source subtree, so touching one file in `src/shared/ui/Alert/Alert.tsx`
/// invalidates the hash of `Alert`, `ui`, and `src` at once. The raw set
/// would surface 3 stale modules for what is conceptually one drift. We
/// drop entries whose `module_path` is an ancestor of another stale
/// entry's path, leaving only the most specific module(s) responsible.
/// Hashes on disk are unchanged — a re-snapshot still refreshes every
/// affected entry; this is purely a presentation filter.
/// Cheap pre-check: does the module's current stat-only fingerprint still match
/// the stored one? `true` = definitely unchanged (skip the SHA-256). `false`
/// when the fingerprint differs OR the dir is missing — in both cases the caller
/// must fall through to the content hash (which reports `MISSING` for a deleted
/// module, so we must NOT short-circuit a missing dir as "fresh").
fn fingerprint_fresh(project_path: &Path, entry: &ModuleHashEntry) -> Result<bool> {
    match calculate_module_fingerprint(project_path, &entry.module_path)? {
        Some(fp) => Ok(fp.file_count == entry.file_count
            && fp.total_size == entry.total_size
            && fp.max_mtime == entry.max_mtime),
        None => Ok(false),
    }
}

pub fn detect_stale_modules(project_path: &Path) -> Result<Vec<StaleModule>> {
    let stored = match load(project_path)? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let mut stale = Vec::new();
    for entry in &stored {
        // Stat-only short-circuit: if the cheap fingerprint still matches, the
        // subtree didn't change → skip the expensive content hash.
        if fingerprint_fresh(project_path, entry)? {
            continue;
        }
        let (current, _file_count) = calculate_module_hash(project_path, &entry.module_path)?;
        if current != entry.code_hash {
            stale.push(StaleModule {
                module_name: entry.module_name.clone(),
                module_path: entry.module_path.clone(),
                stored_hash: entry.code_hash.clone(),
                current_hash: current.clone(),
                missing_on_disk: current == "sha256-MISSING",
            });
        }
    }
    Ok(keep_deepest(stale))
}

/// Check a SINGLE module against its stored fingerprint. Returns `Some` when it
/// drifted, `None` when it's fresh or not covered by the snapshot.
///
/// This is the per-node counterpart of [`detect_stale_modules`]: the Staleness
/// Current sweeps the ocean one module-node at a time and the desktop bridge
/// calls this for each, so the expensive content hashing is spread across
/// background ticks instead of blocking project open. `keep_deepest` filtering
/// (which needs the full set) is applied by the bridge once the sweep ends, via
/// [`filter_deepest_stale`].
pub fn check_module_stale(project_path: &Path, module_path: &str) -> Result<Option<StaleModule>> {
    let stored = match load(project_path)? {
        Some(s) => s,
        None => return Ok(None),
    };
    // A module not present in the snapshot has no baseline to drift from.
    let entry = match stored.iter().find(|e| e.module_path == module_path) {
        Some(e) => e,
        None => return Ok(None),
    };
    // Stat-only short-circuit: matching fingerprint → fresh, skip the SHA-256.
    if fingerprint_fresh(project_path, entry)? {
        return Ok(None);
    }
    let (current, _file_count) = calculate_module_hash(project_path, &entry.module_path)?;
    if current != entry.code_hash {
        Ok(Some(StaleModule {
            module_name: entry.module_name.clone(),
            module_path: entry.module_path.clone(),
            stored_hash: entry.code_hash.clone(),
            current_hash: current.clone(),
            missing_on_disk: current == "sha256-MISSING",
        }))
    } else {
        Ok(None)
    }
}

/// Public wrapper over `keep_deepest`: drop stale entries whose path is an
/// ancestor of another stale entry's path. The Staleness Current accumulates
/// per-module results during a sweep with no global view; the bridge calls this
/// once the sweep completes to collapse "a leaf changed → leaf + ui + src all
/// stale" down to just the responsible leaf, matching `detect_stale_modules`.
pub fn filter_deepest_stale(stale: Vec<StaleModule>) -> Vec<StaleModule> {
    keep_deepest(stale)
}

/// Drop stale entries whose path is an ancestor of another stale entry's
/// path. See `detect_stale_modules` for the rationale.
///
/// Comparison uses forward-slash paths (already normalized at hash-write
/// time) and an explicit `prefix + '/'` check so sibling names that share
/// a prefix (`auth` vs `auth-utils`) aren't accidentally collapsed.
fn keep_deepest(stale: Vec<StaleModule>) -> Vec<StaleModule> {
    if stale.len() <= 1 {
        return stale;
    }
    let paths: Vec<String> = stale.iter().map(|s| s.module_path.clone()).collect();
    stale
        .into_iter()
        .filter(|s| {
            let prefix = format!("{}/", s.module_path.trim_end_matches('/'));
            !paths.iter().any(|other| other != &s.module_path && other.starts_with(&prefix))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn entry(name: &str, path: &str, hash: &str) -> ModuleHashEntry {
        ModuleHashEntry {
            module_name: name.into(),
            module_path: path.into(),
            code_hash: hash.into(),
            file_count: 1,
            // 0/0 fingerprint never matches a real subtree, so these helper
            // entries always fall through to the content hash (old behavior).
            total_size: 0,
            max_mtime: 0,
            computed_at: "2026-05-12T00:00:00Z".into(),
        }
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = TempDir::new().unwrap();
        let records = vec![
            entry("Card", "src/Card", "sha256-bbb"),
            entry("Button", "src/Button", "sha256-aaa"),
        ];
        save(dir.path(), &records).unwrap();
        let loaded = load(dir.path()).unwrap().unwrap();
        // Sorted on disk → loaded order is alphabetical.
        assert_eq!(loaded[0].module_name, "Button");
        assert_eq!(loaded[1].module_name, "Card");
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(load(dir.path()).unwrap().is_none());
    }

    #[test]
    fn load_corrupt_file_backs_up() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(VENORE_DIR)).unwrap();
        fs::write(path_for(dir.path()), "{garbage").unwrap();
        assert!(load(dir.path()).unwrap().is_none());
        assert!(path_for(dir.path()).with_extension("json.corrupt").exists());
    }

    #[test]
    fn detect_stale_returns_empty_when_no_snapshot() {
        let dir = TempDir::new().unwrap();
        assert!(detect_stale_modules(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn detect_stale_finds_modified_modules() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let (hash_v1, _) = calculate_module_hash(dir.path(), "m").unwrap();
        save(dir.path(), &[entry("M", "m", &hash_v1)]).unwrap();
        // Nothing changed → no stale modules.
        assert!(detect_stale_modules(dir.path()).unwrap().is_empty());

        // Touch a file → module is stale.
        write(&dir.path().join("m/a.ts"), "v2");
        let stale = detect_stale_modules(dir.path()).unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].module_name, "M");
        assert_eq!(stale[0].stored_hash, hash_v1);
        assert!(!stale[0].missing_on_disk);
    }

    #[test]
    fn detect_stale_marks_deleted_module() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let (hash, _) = calculate_module_hash(dir.path(), "m").unwrap();
        save(dir.path(), &[entry("Gone", "m", &hash)]).unwrap();
        // Delete the module dir.
        fs::remove_dir_all(dir.path().join("m")).unwrap();
        let stale = detect_stale_modules(dir.path()).unwrap();
        assert_eq!(stale.len(), 1);
        assert!(stale[0].missing_on_disk);
        assert_eq!(stale[0].current_hash, "sha256-MISSING");
    }

    #[test]
    fn check_module_stale_none_without_snapshot() {
        let dir = TempDir::new().unwrap();
        assert!(check_module_stale(dir.path(), "m").unwrap().is_none());
    }

    #[test]
    fn check_module_stale_none_for_untracked_module() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let (hash, _) = calculate_module_hash(dir.path(), "m").unwrap();
        save(dir.path(), &[entry("M", "m", &hash)]).unwrap();
        // "other" isn't in the snapshot → no baseline → not stale.
        assert!(check_module_stale(dir.path(), "other").unwrap().is_none());
    }

    #[test]
    fn check_module_stale_detects_drift_and_freshness() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let (hash_v1, _) = calculate_module_hash(dir.path(), "m").unwrap();
        save(dir.path(), &[entry("M", "m", &hash_v1)]).unwrap();
        // Unchanged → None.
        assert!(check_module_stale(dir.path(), "m").unwrap().is_none());

        // Edit → Some.
        write(&dir.path().join("m/a.ts"), "v2");
        let stale = check_module_stale(dir.path(), "m").unwrap().unwrap();
        assert_eq!(stale.module_name, "M");
        assert!(!stale.missing_on_disk);
    }

    #[test]
    fn check_module_stale_short_circuits_on_matching_fingerprint() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let fp = crate::context::hash::calculate_module_fingerprint(dir.path(), "m")
            .unwrap()
            .unwrap();
        // Store a DELIBERATELY WRONG hash but the RIGHT fingerprint. If the
        // check hashed the content it would report stale; the matching
        // fingerprint must make it short-circuit to fresh (skip the hash).
        let e = ModuleHashEntry {
            module_name: "M".into(),
            module_path: "m".into(),
            code_hash: "sha256-deadbeef".into(),
            file_count: fp.file_count,
            total_size: fp.total_size,
            max_mtime: fp.max_mtime,
            computed_at: "x".into(),
        };
        save(dir.path(), &[e]).unwrap();
        assert!(
            check_module_stale(dir.path(), "m").unwrap().is_none(),
            "matching fingerprint should short-circuit to fresh, skipping the content hash",
        );
    }

    #[test]
    fn check_module_stale_marks_missing() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("m/a.ts"), "v1");
        let (hash, _) = calculate_module_hash(dir.path(), "m").unwrap();
        save(dir.path(), &[entry("Gone", "m", &hash)]).unwrap();
        fs::remove_dir_all(dir.path().join("m")).unwrap();
        let stale = check_module_stale(dir.path(), "m").unwrap().unwrap();
        assert!(stale.missing_on_disk);
    }

    #[test]
    fn filter_deepest_stale_matches_keep_deepest() {
        // Public wrapper collapses ancestors just like the internal filter.
        let result = filter_deepest_stale(vec![
            stale("Alert", "src/shared/ui/Alert"),
            stale("ui", "src/shared/ui"),
            stale("src", "src"),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].module_name, "Alert");
    }

    #[test]
    fn save_sorts_for_deterministic_diff() {
        let dir = TempDir::new().unwrap();
        save(
            dir.path(),
            &[
                entry("Zeta", "z", "sha256-z"),
                entry("Alpha", "a", "sha256-a"),
                entry("Mu", "m", "sha256-m"),
            ],
        )
        .unwrap();
        let raw = fs::read_to_string(path_for(dir.path())).unwrap();
        let alpha = raw.find("Alpha").unwrap();
        let mu = raw.find("Mu").unwrap();
        let zeta = raw.find("Zeta").unwrap();
        assert!(alpha < mu && mu < zeta);
    }

    #[test]
    fn delete_is_idempotent() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &[entry("X", "x", "sha256-x")]).unwrap();
        delete(dir.path()).unwrap();
        assert!(!exists(dir.path()));
        delete(dir.path()).unwrap();
    }

    #[test]
    fn schema_version_and_generated_at_present() {
        let dir = TempDir::new().unwrap();
        save(dir.path(), &[entry("X", "x", "sha256-x")]).unwrap();
        let raw = fs::read_to_string(path_for(dir.path())).unwrap();
        assert!(raw.contains("\"schema_version\""));
        assert!(raw.contains("\"generated_at\""));
        assert!(raw.contains("\"modules\""));
    }

    // ------------------------------------------------------------------------
    // keep_deepest filtering
    // ------------------------------------------------------------------------

    fn stale(name: &str, path: &str) -> StaleModule {
        StaleModule {
            module_name: name.into(),
            module_path: path.into(),
            stored_hash: "sha256-old".into(),
            current_hash: "sha256-new".into(),
            missing_on_disk: false,
        }
    }

    #[test]
    fn keep_deepest_empty_input() {
        assert!(keep_deepest(vec![]).is_empty());
    }

    #[test]
    fn keep_deepest_single_input_kept() {
        let result = keep_deepest(vec![stale("M", "src/m")]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn keep_deepest_siblings_both_kept() {
        let result = keep_deepest(vec![
            stale("Auth", "src/auth"),
            stale("Api", "src/api"),
        ]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn keep_deepest_drops_ancestors_of_descendants() {
        let result = keep_deepest(vec![
            stale("Alert", "src/shared/ui/Alert"),
            stale("ui", "src/shared/ui"),
            stale("src", "src"),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].module_name, "Alert");
    }

    #[test]
    fn keep_deepest_keeps_siblings_with_shared_prefix() {
        // `src/auth` is NOT an ancestor of `src/auth-utils` — both kept.
        let result = keep_deepest(vec![
            stale("auth", "src/auth"),
            stale("auth-utils", "src/auth-utils"),
        ]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn keep_deepest_drops_only_ancestor_when_one_branch_stale() {
        // Two-branch tree: only one branch has a stale leaf, the other
        // branch's root module isn't stale at all. The single common
        // ancestor (`src`) is dropped because of the stale leaf.
        let result = keep_deepest(vec![
            stale("Alert", "src/shared/ui/Alert"),
            stale("src", "src"),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].module_name, "Alert");
    }

    #[test]
    fn detect_stale_filters_to_deepest_e2e() {
        // Real disk: two nested module dirs that both end up stale because
        // a leaf source file changed. After filtering, only the leaf is
        // reported.
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("src/leaf/a.ts"), "v1");
        let (h_src, _) = calculate_module_hash(dir.path(), "src").unwrap();
        let (h_leaf, _) = calculate_module_hash(dir.path(), "src/leaf").unwrap();
        save(
            dir.path(),
            &[entry("src", "src", &h_src), entry("leaf", "src/leaf", &h_leaf)],
        )
        .unwrap();
        // Both fresh.
        assert!(detect_stale_modules(dir.path()).unwrap().is_empty());

        // Bump the leaf file — both hashes change, but only `leaf` should
        // be reported.
        write(&dir.path().join("src/leaf/a.ts"), "v2");
        let stale_list = detect_stale_modules(dir.path()).unwrap();
        assert_eq!(stale_list.len(), 1);
        assert_eq!(stale_list[0].module_name, "leaf");
    }
}
