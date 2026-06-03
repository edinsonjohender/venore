//! Island Detector - Groups modules into logical sub-islands
//!
//! Detects sub-islands based on:
//! - Path-based clustering (directory structure)
//! - Dependency cohesion (internal vs external deps)
//! - Criticality scores (incoming dependencies)

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::analysis_output::ModuleAnalysis;

// =============================================================================
// TYPES
// =============================================================================

/// Configuration for island detection
#[derive(Debug, Clone)]
pub struct IslandDetectorConfig {
    /// Modules to analyze (from cached AnalysisOutput)
    pub modules: Vec<ModuleAnalysis>,

    /// Detection parameters
    pub params: IslandParams,
}

/// Adjustable detection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandParams {
    /// Minimum modules to form an island (default: 2)
    pub min_modules: usize,

    /// Maximum path depth for grouping (default: 2)
    /// e.g., 2 = "src/components", 3 = "src/components/auth"
    pub max_depth: usize,

    /// Minimum cohesion ratio 0.0-1.0 (default: 0.3)
    /// Cohesion = internal_deps / total_deps
    pub cohesion_threshold: f32,

    /// Minimum sub-features to extract as separate island (default: 3)
    /// Used for weight-based filtering
    pub weight_threshold: usize,

    /// Minimum incoming dependencies for criticality (default: 3)
    /// Modules with >= N dependents are marked as critical
    pub dependency_score: usize,
}

impl Default for IslandParams {
    fn default() -> Self {
        Self {
            min_modules: 2,
            max_depth: 2,
            cohesion_threshold: 0.3,
            weight_threshold: 3,
            dependency_score: 3,
        }
    }
}

/// A detected sub-island (logical grouping of modules)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Island {
    /// Unique identifier
    pub id: String,

    /// Human-readable name (e.g., "Auth System", "API Gateway")
    pub name: String,

    /// Description of what this island does
    pub description: String,

    /// Module names belonging to this island
    pub modules: Vec<String>,

    /// Metrics
    pub cohesion: f32,      // 0.0-1.0 internal dependency ratio
    pub weight: usize,      // Number of modules in island
    pub criticality: usize, // Average incoming dependencies

    /// Hierarchy (for nested islands, future feature)
    pub level: usize, // 0=root, 1=sub-island, etc.
    pub parent_id: Option<String>,
}

/// Aggregated metrics from detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandMetrics {
    /// Total modules analyzed
    pub total_modules: usize,

    /// Number of islands detected
    pub islands_detected: usize,

    /// Average cohesion across all islands
    pub avg_cohesion: f32,

    /// Critical modules (high incoming dependencies)
    pub critical_modules: Vec<String>,
}

/// Result of island detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandDetectionResult {
    /// Detected islands
    pub islands: Vec<Island>,

    /// Aggregated metrics
    pub metrics: IslandMetrics,
}

// =============================================================================
// MAIN FUNCTION
// =============================================================================

/// Detect sub-islands from analyzed modules
///
/// This function groups modules into logical islands based on:
/// - Path-based clustering (directory structure)
/// - Dependency cohesion (internal vs external deps)
/// - Criticality scores (incoming dependencies)
///
/// # Arguments
///
/// * `config` - Configuration with modules and detection parameters
///
/// # Returns
///
/// Returns `IslandDetectionResult` with islands and metrics
///
/// # Examples
///
/// ```no_run
/// use venore_core::analysis::island_detector::*;
///
/// let config = IslandDetectorConfig {
///     modules: vec![],  // In real usage, from a cached AnalysisOutput
///     params: IslandParams::default(),
/// };
///
/// let result = detect_islands(config).unwrap();
/// println!("Found {} islands", result.islands.len());
/// ```
pub fn detect_islands(config: IslandDetectorConfig) -> Result<IslandDetectionResult> {
    // 1. Build dependency maps (outgoing + incoming)
    let (outgoing_deps, _incoming_deps) = build_dependency_maps(&config.modules);

    // 2. Calculate criticality scores
    let criticality_scores = calculate_criticality_scores(&config.modules);

    // 3. Identify critical modules (>= dependency_score threshold)
    let critical_modules =
        identify_critical_modules(&criticality_scores, config.params.dependency_score);

    // 4. Cluster modules by path prefix
    let path_clusters = cluster_by_path(&config.modules, config.params.max_depth);
    tracing::info!("📦 Found {} path-based clusters", path_clusters.len());

    // 5. Filter by min_modules threshold
    let filtered_clusters: HashMap<_, _> = path_clusters
        .into_iter()
        .filter(|(path, modules)| {
            let passes = modules.len() >= config.params.min_modules;
            if !passes {
                tracing::info!("  Cluster '{}': {} modules (< min {}) - FILTERED OUT",
                    path, modules.len(), config.params.min_modules);
            }
            passes
        })
        .collect();

    tracing::info!("📦 {} clusters pass min_modules filter", filtered_clusters.len());

    // 6. Calculate cohesion and build islands
    let mut islands = Vec::new();
    for (group_path, module_names) in filtered_clusters {
        let cohesion = calculate_cohesion(&module_names, &outgoing_deps);

        // DEBUG: Log cohesion calculation
        tracing::info!("  Cluster '{}': {} modules, cohesion: {:.2}",
            group_path, module_names.len(), cohesion);

        // Skip if cohesion below threshold
        if cohesion < config.params.cohesion_threshold {
            tracing::info!("    ❌ Skipped (cohesion {:.2} < threshold {:.2})",
                cohesion, config.params.cohesion_threshold);
            continue;
        } else {
            tracing::info!("    ✅ Accepted (cohesion {:.2} >= threshold {:.2})",
                cohesion, config.params.cohesion_threshold);
        }

        // Calculate metrics before moving module_names
        let weight = module_names.len();
        let avg_criticality = module_names
            .iter()
            .filter_map(|m| criticality_scores.get(m))
            .sum::<usize>()
            / weight.max(1);

        let critical_count = module_names
            .iter()
            .filter(|m| critical_modules.contains(m))
            .count();

        islands.push(Island {
            id: uuid::Uuid::new_v4().to_string(),
            name: generate_island_name(&group_path),
            description: format!(
                "{} modules • {}% cohesion • {} critical",
                weight,
                (cohesion * 100.0) as u32,
                critical_count
            ),
            modules: module_names,
            cohesion,
            weight,
            criticality: avg_criticality,
            level: 0,
            parent_id: None,
        });
    }

    // 7. Create "Uncategorized" island for orphaned modules
    let modules_in_islands: std::collections::HashSet<String> = islands
        .iter()
        .flat_map(|island| island.modules.clone())
        .collect();

    let orphaned_modules: Vec<String> = config
        .modules
        .iter()
        .map(|m| m.name.clone())
        .filter(|name| !modules_in_islands.contains(name))
        .collect();

    if !orphaned_modules.is_empty() {
        let orphan_count = orphaned_modules.len();
        let avg_criticality = orphaned_modules
            .iter()
            .filter_map(|m| criticality_scores.get(m))
            .sum::<usize>()
            / orphan_count.max(1);

        let critical_count = orphaned_modules
            .iter()
            .filter(|m| critical_modules.contains(m))
            .count();

        tracing::info!(
            "🏝️  Creating 'Uncategorized' island for {} orphaned modules",
            orphan_count
        );

        islands.push(Island {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Uncategorized".to_string(),
            description: format!(
                "{} modules • uncategorized • {} critical",
                orphan_count, critical_count
            ),
            modules: orphaned_modules,
            cohesion: 0.0, // No cohesion between unrelated modules
            weight: orphan_count,
            criticality: avg_criticality,
            level: 0,
            parent_id: None,
        });
    }

    // 8. Sort by criticality (desc), then weight (desc)
    islands.sort_by(|a, b| {
        b.criticality
            .cmp(&a.criticality)
            .then(b.weight.cmp(&a.weight))
    });

    // 9. Calculate aggregated metrics
    let metrics = IslandMetrics {
        total_modules: config.modules.len(),
        islands_detected: islands.len(),
        avg_cohesion: if islands.is_empty() {
            0.0
        } else {
            islands.iter().map(|i| i.cohesion).sum::<f32>() / islands.len() as f32
        },
        critical_modules,
    };

    Ok(IslandDetectionResult { islands, metrics })
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Build outgoing and incoming dependency maps
///
/// Uses `ModuleAnalysis.architecture.dependencies` (already computed!)
///
/// # Returns
///
/// (outgoing_deps, incoming_deps) where:
/// - outgoing_deps[module] = Vec<String> of modules this depends on
/// - incoming_deps[module] = Vec<String> of modules depending on this
fn build_dependency_maps(
    modules: &[ModuleAnalysis],
) -> (
    HashMap<String, Vec<String>>,
    HashMap<String, Vec<String>>,
) {
    let mut outgoing_deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming_deps: HashMap<String, Vec<String>> = HashMap::new();

    for module in modules {
        // Outgoing dependencies (already computed in ModuleAnalysis)
        outgoing_deps.insert(
            module.name.clone(),
            module.architecture.dependencies.clone(),
        );

        // Build reverse map (incoming dependencies)
        for dep in &module.architecture.dependencies {
            incoming_deps
                .entry(dep.clone())
                .or_default()
                .push(module.name.clone());
        }
    }

    (outgoing_deps, incoming_deps)
}

/// Calculate cohesion ratio for a group of modules
///
/// Cohesion = internal_deps / total_deps
/// - internal_deps: dependencies within the group
/// - total_deps: all dependencies (internal + external)
///
/// # Returns
///
/// 0.0-1.0 ratio, or 0.5 if no dependencies
///
/// # Examples
///
/// Group: ["auth", "authService"]
/// Dependencies:
/// - auth → authService (internal)
/// - auth → api (external)
/// Cohesion = 1 internal / 2 total = 0.5
pub fn calculate_cohesion(
    module_names: &[String],
    dependency_map: &HashMap<String, Vec<String>>,
) -> f32 {
    let module_set: std::collections::HashSet<_> = module_names.iter().collect();
    let mut internal_deps = 0;
    let mut total_deps = 0;

    for module_name in module_names {
        if let Some(deps) = dependency_map.get(module_name) {
            for dep in deps {
                total_deps += 1;
                if module_set.contains(dep) {
                    internal_deps += 1;
                }
            }
        }
    }

    if total_deps == 0 {
        0.5 // Neutral confidence if no dependencies
    } else {
        internal_deps as f32 / total_deps as f32
    }
}

/// Cluster modules by common path prefix
///
/// Groups modules that share the same directory at `max_depth` level
///
/// # Arguments
///
/// * `modules` - Modules to cluster
/// * `max_depth` - Directory depth for grouping (2 = "src/components")
///
/// # Returns
///
/// HashMap<group_key, Vec<module_name>>
///
/// # Examples
///
/// max_depth = 2
/// - src/auth/service.ts → "src/auth"
/// - src/auth/controller.ts → "src/auth"
/// - src/api/routes.ts → "src/api"
///
/// Result: {
///   "src/auth": ["auth-service", "auth-controller"],
///   "src/api": ["api-routes"]
/// }
fn cluster_by_path(modules: &[ModuleAnalysis], max_depth: usize) -> HashMap<String, Vec<String>> {
    let mut path_groups: HashMap<String, Vec<String>> = HashMap::new();

    for module in modules {
        // Split path by separator (supports both Unix / and Windows \)
        let path_str = &module.path;
        let segments: Vec<&str> = path_str.split(&['/', '\\'][..]).collect();

        // Group by configurable depth
        let group_key = if segments.len() >= max_depth {
            // Use platform-independent separator for consistency
            segments[..max_depth].join("/")
        } else if !segments.is_empty() {
            segments[0].to_string()
        } else {
            "root".to_string()
        };

        path_groups
            .entry(group_key)
            .or_default()
            .push(module.name.clone());
    }

    path_groups
}

/// Calculate criticality scores for each module
///
/// Criticality = number of incoming dependencies (dependents)
/// Uses `ModuleAnalysis.architecture.dependents` (already computed!)
///
/// # Returns
///
/// HashMap<module_name, criticality_score>
fn calculate_criticality_scores(modules: &[ModuleAnalysis]) -> HashMap<String, usize> {
    let mut scores = HashMap::new();

    for module in modules {
        let score = module.architecture.dependents.len();
        scores.insert(module.name.clone(), score);
    }

    scores
}

/// Generate human-readable island name from path
///
/// Converts path segments to Title Case
///
/// # Examples
///
/// - "src/auth" → "Auth System"
/// - "crates/venore-core" → "Venore Core"
/// - "packages/ui-components" → "Ui Components System"
pub fn generate_island_name(path: &str) -> String {
    // Convert "src/auth" → "Auth System"
    // Convert "crates/venore-core" → "Venore Core"
    let segments: Vec<&str> = path.split('/').collect();
    let last_segment = segments.last().unwrap_or(&"Unknown");

    // Capitalize and add "System" if needed
    let capitalized = last_segment
        .split('-')
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if capitalized.to_lowercase().contains("core")
        || capitalized.to_lowercase().contains("lib")
    {
        capitalized
    } else {
        format!("{} System", capitalized)
    }
}

/// Identify modules with high incoming dependencies
///
/// # Arguments
///
/// * `criticality_scores` - Map of module → score
/// * `threshold` - Minimum score to be considered critical
///
/// # Returns
///
/// Vec of module names with score >= threshold
fn identify_critical_modules(
    criticality_scores: &HashMap<String, usize>,
    threshold: usize,
) -> Vec<String> {
    criticality_scores
        .iter()
        .filter(|(_, &score)| score >= threshold)
        .map(|(name, _)| name.clone())
        .collect()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analysis_output::{ModuleArchitecture, ModuleSymbols};

    fn create_test_module(name: &str, path: &str, deps: Vec<&str>) -> ModuleAnalysis {
        ModuleAnalysis {
            name: name.to_string(),
            path: path.to_string(),
            file_count: 1,
            entry_point: None,
            architecture: ModuleArchitecture {
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                dependents: vec![], // Will be computed
                external_deps: vec![],
            },
            symbols: ModuleSymbols {
                exports: vec![],
                all: vec![],
            },
            imports: vec![],
            code_snippets: String::new(),
            files: vec![],
        }
    }

    #[test]
    fn test_calculate_cohesion_high() {
        // Group with mostly internal dependencies
        let modules = vec!["auth".to_string(), "authService".to_string()];
        let mut deps = HashMap::new();
        deps.insert("auth".to_string(), vec!["authService".to_string()]);
        deps.insert("authService".to_string(), vec!["auth".to_string()]);

        let cohesion = calculate_cohesion(&modules, &deps);
        assert_eq!(cohesion, 1.0); // 100% internal
    }

    #[test]
    fn test_calculate_cohesion_low() {
        // Group with mostly external dependencies
        let modules = vec!["auth".to_string()];
        let mut deps = HashMap::new();
        deps.insert(
            "auth".to_string(),
            vec!["api".to_string(), "db".to_string()],
        );

        let cohesion = calculate_cohesion(&modules, &deps);
        assert_eq!(cohesion, 0.0); // 0% internal
    }

    #[test]
    fn test_calculate_cohesion_no_deps() {
        let modules = vec!["isolated".to_string()];
        let deps = HashMap::new();

        let cohesion = calculate_cohesion(&modules, &deps);
        assert_eq!(cohesion, 0.5); // Neutral
    }

    #[test]
    fn test_cluster_by_path_depth_2() {
        let modules = vec![
            create_test_module("auth", "src/auth/service", vec![]),
            create_test_module("auth-controller", "src/auth/controller", vec![]),
            create_test_module("api", "src/api/routes", vec![]),
        ];

        let clusters = cluster_by_path(&modules, 2);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters.get("src/auth").unwrap().len(), 2);
        assert_eq!(clusters.get("src/api").unwrap().len(), 1);
    }

    #[test]
    fn test_generate_island_name() {
        assert_eq!(generate_island_name("src/auth"), "Auth System");
        assert_eq!(generate_island_name("crates/venore-core"), "Venore Core");
        assert_eq!(
            generate_island_name("packages/ui-components"),
            "Ui Components System"
        );
    }

    #[test]
    fn test_identify_critical_modules() {
        let mut scores = HashMap::new();
        scores.insert("auth".to_string(), 5);
        scores.insert("api".to_string(), 2);
        scores.insert("ui".to_string(), 1);

        let critical = identify_critical_modules(&scores, 3);
        assert_eq!(critical.len(), 1);
        assert!(critical.contains(&"auth".to_string()));
    }

    #[test]
    fn test_detect_islands_basic() {
        let modules = vec![
            create_test_module("auth", "src/auth/service", vec!["authHelper"]),
            create_test_module("authHelper", "src/auth/helper", vec!["auth"]),
            create_test_module("api", "src/api/routes", vec![]),
        ];

        let config = IslandDetectorConfig {
            modules,
            params: IslandParams {
                min_modules: 2,
                max_depth: 2,
                cohesion_threshold: 0.3,
                weight_threshold: 3,
                dependency_score: 3,
            },
        };

        let result = detect_islands(config).unwrap();
        assert_eq!(result.islands.len(), 2); // src/auth group + Uncategorized (api)
        let auth_island = result.islands.iter().find(|i| i.name != "Uncategorized").unwrap();
        assert_eq!(auth_island.modules.len(), 2);
        let uncategorized = result.islands.iter().find(|i| i.name == "Uncategorized").unwrap();
        assert_eq!(uncategorized.modules.len(), 1);
    }
}
