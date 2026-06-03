//! Mesh Discovery — register, unregister, and discover peer instances
//!
//! Each open project in a Venore process registers itself in
//! `~/.venore/mesh/{project_id}.json`. A single process can host N peer
//! registrations (one per open project), so identity is keyed by
//! `project_id`, not by PID. Liveness is detected via a TTL on the
//! `last_seen` timestamp — the background loop in `mesh::lifecycle`
//! refreshes local peers periodically; anything older than [`STALE_TTL`]
//! is treated as a dead peer and its file is cleaned up.

use crate::analysis::AnalysisOutput;
use crate::error::{Result, VenoreError};
use crate::mesh::types::{PeerInfo, PeerRegistration, ProjectProfile};
use chrono::Utc;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Peers whose `last_seen` is older than this are considered dead and
/// their registration file is removed during `discover_peers`. The
/// background loop touches local registrations every 60s, so 5 minutes
/// gives plenty of margin for slow disks / paused processes while still
/// cleaning up fast after a crash.
const STALE_TTL: chrono::Duration = chrono::Duration::minutes(5);

/// Manages mesh peer discovery via filesystem-based registration.
///
/// Multiple peers per process are supported — one entry per `project_id`
/// in the `registrations` map, mirrored as one file in `mesh_dir`.
pub struct MeshDiscovery {
    /// Locally-registered peers, keyed by `project_id`.
    registrations: HashMap<String, PeerRegistration>,
    /// Directory for mesh registration files (~/.venore/mesh/)
    mesh_dir: PathBuf,
}

impl MeshDiscovery {
    /// Creates a new MeshDiscovery (private — use global())
    fn new() -> Self {
        let mesh_dir = if cfg!(debug_assertions) {
            std::env::temp_dir().join("venore-dev").join("mesh")
        } else {
            dirs::home_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(".venore")
                .join("mesh")
        };

        Self {
            registrations: HashMap::new(),
            mesh_dir,
        }
    }

    /// Gets the global MeshDiscovery instance
    pub fn global() -> Arc<Mutex<Self>> {
        static INSTANCE: Lazy<Arc<Mutex<MeshDiscovery>>> =
            Lazy::new(|| Arc::new(Mutex::new(MeshDiscovery::new())));
        INSTANCE.clone()
    }

    /// Register a project as a mesh peer in this process.
    ///
    /// If the same `project_id` is already registered locally, this is a
    /// no-op — covers the "same project opened in two windows" case
    /// without duplicating the peer.
    pub fn register(
        &mut self,
        project_id: &str,
        project_name: &str,
        project_path: &str,
    ) -> Result<()> {
        if self.registrations.contains_key(project_id) {
            return Ok(());
        }

        let profile = Self::build_profile(Path::new(project_path));

        let now = Utc::now();
        let registration = PeerRegistration {
            project_id: project_id.to_string(),
            project_name: project_name.to_string(),
            project_path: project_path.to_string(),
            pid: std::process::id(),
            port: 0,
            registered_at: now,
            last_seen: now,
            profile,
        };

        self.write_registration(&registration)?;
        self.registrations
            .insert(project_id.to_string(), registration);

        tracing::info!(
            project_id = project_id,
            pid = std::process::id(),
            "Registered in mesh"
        );

        Ok(())
    }

    /// Unregister a single project peer. Called when its last window closes
    /// or via the mesh disconnect command.
    pub fn unregister(&mut self, project_id: &str) -> Result<()> {
        if self.registrations.remove(project_id).is_some() {
            self.remove_registration_file(project_id)?;
            tracing::info!(project_id = %project_id, "Unregistered from mesh");
        }
        Ok(())
    }

    /// Unregister every locally-registered peer. Called on app exit.
    pub fn unregister_all(&mut self) -> Result<()> {
        let ids: Vec<String> = self.registrations.keys().cloned().collect();
        for id in ids {
            let _ = self.unregister(&id);
        }
        Ok(())
    }

    /// Bump `last_seen` on all local peers and rewrite their files. The
    /// background loop calls this every 60s so other processes' TTL-based
    /// liveness check sees us as alive.
    pub fn touch_local(&mut self) -> Result<()> {
        let now = Utc::now();
        for reg in self.registrations.values_mut() {
            reg.last_seen = now;
        }
        // Collect clones to drop the &mut borrow before writing.
        let snapshots: Vec<PeerRegistration> = self.registrations.values().cloned().collect();
        for reg in snapshots {
            self.write_registration(&reg)?;
        }
        Ok(())
    }

    /// Discover all live peers across all processes — including this
    /// process's other local peers. Cleans up registration files whose
    /// `last_seen` is older than [`STALE_TTL`].
    ///
    /// The result intentionally includes the caller's own registrations:
    /// in single-process multi-window mode, project A's window needs to
    /// see project B's peer (also local). Each consumer filters out its
    /// own `project_id` itself.
    pub fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        if !self.mesh_dir.exists() {
            return Ok(vec![]);
        }

        let entries = std::fs::read_dir(&self.mesh_dir).map_err(|e| {
            VenoreError::MeshError(format!("Failed to read mesh directory: {}", e))
        })?;

        let now = Utc::now();
        let mut peers = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let reg: PeerRegistration = match serde_json::from_str(&content) {
                Ok(r) => r,
                Err(_) => {
                    tracing::warn!(path = %path.display(), "Removing corrupt mesh registration");
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            };

            // Liveness — locally-registered peers are always alive (we own
            // them); remote peers age out by TTL on `last_seen`.
            if !self.registrations.contains_key(&reg.project_id) {
                let age = now.signed_duration_since(reg.last_seen);
                if age > STALE_TTL {
                    tracing::info!(
                        project_id = %reg.project_id,
                        pid = reg.pid,
                        age_secs = age.num_seconds(),
                        "Cleaning up stale mesh registration"
                    );
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            }

            peers.push(PeerInfo {
                project_id: reg.project_id,
                project_name: reg.project_name,
                project_path: reg.project_path,
                port: reg.port,
                is_alive: true,
                profile: reg.profile,
            });
        }

        tracing::debug!(count = peers.len(), "Discovered mesh peers");
        Ok(peers)
    }

    /// Update the port for a registered peer after its transport binds.
    pub fn update_port(&mut self, project_id: &str, port: u16) -> Result<()> {
        let reg = self.registrations.get_mut(project_id).ok_or_else(|| {
            VenoreError::MeshError(format!("Not registered: {}", project_id))
        })?;
        reg.port = port;
        reg.last_seen = Utc::now();
        let snapshot = reg.clone();
        self.write_registration(&snapshot)?;
        tracing::info!(project_id = %project_id, port = port, "Updated mesh registration port");
        Ok(())
    }

    /// Set the same port on every local registration. The transport server
    /// is per-process, so all peers in this process share one port.
    pub fn update_port_all(&mut self, port: u16) -> Result<()> {
        let now = Utc::now();
        for reg in self.registrations.values_mut() {
            reg.port = port;
            reg.last_seen = now;
        }
        let snapshots: Vec<PeerRegistration> = self.registrations.values().cloned().collect();
        for reg in snapshots {
            self.write_registration(&reg)?;
        }
        if !self.registrations.is_empty() {
            tracing::info!(port = port, peer_count = self.registrations.len(), "Updated mesh port on all local registrations");
        }
        Ok(())
    }

    /// Read a peer's registration file by project_id.
    pub fn get_peer_registration(&self, project_id: &str) -> Result<PeerRegistration> {
        let path = self.mesh_dir.join(format!("{}.json", project_id));
        if !path.exists() {
            return Err(VenoreError::MeshPeerNotFound(project_id.to_string()));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VenoreError::MeshError(format!("Failed to read peer registration: {}", e))
        })?;
        let reg: PeerRegistration = serde_json::from_str(&content)?;
        Ok(reg)
    }

    /// True iff this process holds a registration for `project_id`.
    pub fn is_registered(&self, project_id: &str) -> bool {
        self.registrations.contains_key(project_id)
    }

    /// True iff this process holds at least one registration.
    pub fn has_any_registration(&self) -> bool {
        !self.registrations.is_empty()
    }

    /// Get a snapshot of one local registration.
    pub fn get_local_registration(&self, project_id: &str) -> Option<&PeerRegistration> {
        self.registrations.get(project_id)
    }

    /// Iterate over all local registrations.
    pub fn iter_local_registrations(&self) -> impl Iterator<Item = &PeerRegistration> {
        self.registrations.values()
    }

    /// Get the mesh directory path (used by MeshTransport for test isolation)
    pub fn mesh_dir(&self) -> &std::path::Path {
        &self.mesh_dir
    }

    // =========================================================================
    // Profile building
    // =========================================================================

    /// Build a ProjectProfile from on-disk analysis data.
    /// Returns None if analysis hasn't run yet — no computation, just reads JSON.
    fn build_profile(project_path: &Path) -> Option<ProjectProfile> {
        let output = match AnalysisOutput::load_from_disk(project_path) {
            Ok(Some(o)) => o,
            _ => return None,
        };

        let language = output
            .repository
            .language
            .as_ref()
            .map(|l| format!("{:?}", l));
        let technologies = output.repository.technologies.clone();
        let module_names: Vec<String> = output.modules.iter().map(|m| m.name.clone()).collect();
        let total_files = output.repository.total_files;
        let total_modules = output.repository.total_modules;
        let description = Self::description_from_project_memory(project_path);

        Some(ProjectProfile {
            language,
            technologies,
            module_names,
            total_files,
            total_modules,
            description,
        })
    }

    /// Short blurb shown in peer cards. Reads the curated `description`
    /// from the portable project memory at
    /// `<project>/.venore/project-memory.json`. Returns `None` when the
    /// file is missing, unparseable, or the description was left blank
    /// — the UI then renders the peer card without a tooltip.
    ///
    /// Truncates to ~200 characters at a word boundary so the tooltip
    /// stays small.
    fn description_from_project_memory(project_path: &Path) -> Option<String> {
        let memory = crate::memory::file_storage::load(project_path).ok().flatten()?;
        let desc = memory.description.trim();
        if desc.is_empty() {
            return None;
        }
        if desc.len() <= 200 {
            return Some(desc.to_string());
        }
        let boundary = desc[..200].rfind(' ').unwrap_or(200);
        Some(format!("{}...", &desc[..boundary]))
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Write registration to disk using atomic write (temp + rename)
    fn write_registration(&self, reg: &PeerRegistration) -> Result<()> {
        std::fs::create_dir_all(&self.mesh_dir).map_err(|e| {
            VenoreError::MeshError(format!("Failed to create mesh directory: {}", e))
        })?;

        let target = self.mesh_dir.join(format!("{}.json", reg.project_id));
        let temp = self
            .mesh_dir
            .join(format!("{}.json.tmp", reg.project_id));

        let json = serde_json::to_string_pretty(reg)?;
        std::fs::write(&temp, &json).map_err(|e| {
            VenoreError::MeshError(format!("Failed to write temp registration: {}", e))
        })?;
        std::fs::rename(&temp, &target).map_err(|e| {
            VenoreError::MeshError(format!("Failed to rename registration file: {}", e))
        })?;

        Ok(())
    }

    /// Remove a registration file by project_id
    fn remove_registration_file(&self, project_id: &str) -> Result<()> {
        let path = self.mesh_dir.join(format!("{}.json", project_id));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                VenoreError::MeshError(format!("Failed to remove registration: {}", e))
            })?;
        }
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a MeshDiscovery with a custom temp directory for testing
    fn test_discovery(dir: &std::path::Path) -> MeshDiscovery {
        MeshDiscovery {
            registrations: HashMap::new(),
            mesh_dir: dir.to_path_buf(),
        }
    }

    #[test]
    fn test_register_creates_file() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery
            .register("proj-1", "My Project", "/path/to/project")
            .unwrap();

        let file = mesh_dir.join("proj-1.json");
        assert!(file.exists(), "Registration file should exist");

        let content = std::fs::read_to_string(&file).unwrap();
        let reg: PeerRegistration = serde_json::from_str(&content).unwrap();
        assert_eq!(reg.project_id, "proj-1");
        assert_eq!(reg.project_name, "My Project");
        assert_eq!(reg.project_path, "/path/to/project");
        assert_eq!(reg.pid, std::process::id());
        assert_eq!(reg.port, 0);
    }

    #[test]
    fn test_unregister_removes_file() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery
            .register("proj-2", "Test Project", "/tmp/test")
            .unwrap();
        let file = mesh_dir.join("proj-2.json");
        assert!(file.exists());

        discovery.unregister("proj-2").unwrap();
        assert!(!file.exists(), "Registration file should be removed");
        assert!(!discovery.is_registered("proj-2"));
    }

    #[test]
    fn test_register_multiple_projects_same_process() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery.register("proj-a", "A", "/a").unwrap();
        discovery.register("proj-b", "B", "/b").unwrap();

        assert!(discovery.is_registered("proj-a"));
        assert!(discovery.is_registered("proj-b"));
        assert!(mesh_dir.join("proj-a.json").exists());
        assert!(mesh_dir.join("proj-b.json").exists());

        discovery.unregister("proj-a").unwrap();
        assert!(!discovery.is_registered("proj-a"));
        assert!(discovery.is_registered("proj-b"));
        assert!(mesh_dir.join("proj-b.json").exists());
    }

    #[test]
    fn test_unregister_all_drains_everything() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery.register("proj-a", "A", "/a").unwrap();
        discovery.register("proj-b", "B", "/b").unwrap();
        discovery.unregister_all().unwrap();

        assert!(!discovery.has_any_registration());
        assert!(!mesh_dir.join("proj-a.json").exists());
        assert!(!mesh_dir.join("proj-b.json").exists());
    }

    #[test]
    fn test_discover_finds_fresh_peers() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        std::fs::create_dir_all(&mesh_dir).unwrap();

        let discovery = test_discovery(&mesh_dir);

        let fresh = PeerRegistration {
            project_id: "peer-1".to_string(),
            project_name: "Peer Project".to_string(),
            project_path: "/other/project".to_string(),
            pid: 4_000_000,
            port: 0,
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            profile: None,
        };
        std::fs::write(
            mesh_dir.join("peer-1.json"),
            serde_json::to_string_pretty(&fresh).unwrap(),
        )
        .unwrap();

        let peers = discovery.discover_peers().unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].project_id, "peer-1");
    }

    #[test]
    fn test_discover_cleans_stale_peers_by_ttl() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        std::fs::create_dir_all(&mesh_dir).unwrap();

        let discovery = test_discovery(&mesh_dir);

        let stale = PeerRegistration {
            project_id: "stale-1".to_string(),
            project_name: "Stale".to_string(),
            project_path: "/gone".to_string(),
            pid: 9_999,
            port: 0,
            registered_at: Utc::now() - chrono::Duration::hours(1),
            last_seen: Utc::now() - chrono::Duration::hours(1),
            profile: None,
        };
        let file = mesh_dir.join("stale-1.json");
        std::fs::write(&file, serde_json::to_string_pretty(&stale).unwrap()).unwrap();
        assert!(file.exists());

        let peers = discovery.discover_peers().unwrap();
        assert!(peers.is_empty(), "Stale peer should not appear");
        assert!(!file.exists(), "Stale file should be cleaned up");
    }

    #[test]
    fn test_register_idempotent_keeps_first_registration() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery.register("proj-3", "Project V1", "/path/v1").unwrap();
        discovery.register("proj-3", "Project V2", "/path/v2").unwrap();

        let reg = discovery.get_local_registration("proj-3").unwrap();
        assert_eq!(reg.project_name, "Project V1", "Same id is no-op");
        assert_eq!(reg.project_path, "/path/v1");
        assert!(discovery.is_registered("proj-3"));
    }

    #[test]
    fn test_discover_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh-nonexistent");
        let discovery = test_discovery(&mesh_dir);

        let peers = discovery.discover_peers().unwrap();
        assert!(peers.is_empty());
    }

    #[test]
    fn test_discover_includes_local_peers() {
        // Multi-window/single-process: each local project must show up in
        // the discovery list so other windows can see it as a peer.
        // Consumers filter their own project_id at the call site.
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery
            .register("self-proj", "Self Project", "/self/path")
            .unwrap();

        let peers = discovery.discover_peers().unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].project_id, "self-proj");
    }

    #[test]
    fn test_discover_does_not_ttl_evict_local_peers() {
        // Local peers are always alive (we own them) — TTL should only
        // apply to peers owned by other processes. Without this guard, a
        // process that didn't touch its registration for >TTL would
        // delete its own files.
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery.register("local-old", "Old", "/old").unwrap();
        // Manually rewrite to make last_seen ancient.
        let stale_path = mesh_dir.join("local-old.json");
        let mut reg: PeerRegistration =
            serde_json::from_str(&std::fs::read_to_string(&stale_path).unwrap()).unwrap();
        reg.last_seen = Utc::now() - chrono::Duration::hours(1);
        std::fs::write(&stale_path, serde_json::to_string_pretty(&reg).unwrap()).unwrap();

        let peers = discovery.discover_peers().unwrap();
        assert_eq!(peers.len(), 1, "Local peer must survive TTL sweep");
        assert!(stale_path.exists(), "Local registration file must not be deleted");
    }

    #[test]
    fn test_update_port() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery
            .register("port-proj", "Port Project", "/path/port")
            .unwrap();
        discovery.update_port("port-proj", 12345).unwrap();

        let content =
            std::fs::read_to_string(mesh_dir.join("port-proj.json")).unwrap();
        let reg: PeerRegistration = serde_json::from_str(&content).unwrap();
        assert_eq!(reg.port, 12345);
    }

    #[test]
    fn test_touch_local_refreshes_last_seen() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        let mut discovery = test_discovery(&mesh_dir);

        discovery.register("touch-proj", "T", "/t").unwrap();
        let before = discovery
            .get_local_registration("touch-proj")
            .unwrap()
            .last_seen;

        std::thread::sleep(std::time::Duration::from_millis(10));
        discovery.touch_local().unwrap();

        let after = discovery
            .get_local_registration("touch-proj")
            .unwrap()
            .last_seen;
        assert!(after > before);

        // Disk also reflects the touch.
        let on_disk: PeerRegistration = serde_json::from_str(
            &std::fs::read_to_string(mesh_dir.join("touch-proj.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk.last_seen, after);
    }

    #[test]
    fn test_get_peer_registration() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        std::fs::create_dir_all(&mesh_dir).unwrap();

        let discovery = test_discovery(&mesh_dir);

        let fake_peer = PeerRegistration {
            project_id: "get-peer".to_string(),
            project_name: "Get Peer".to_string(),
            project_path: "/get/peer".to_string(),
            pid: 9999,
            port: 8080,
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            profile: None,
        };
        let content = serde_json::to_string_pretty(&fake_peer).unwrap();
        std::fs::write(mesh_dir.join("get-peer.json"), &content).unwrap();

        let reg = discovery.get_peer_registration("get-peer").unwrap();
        assert_eq!(reg.project_id, "get-peer");
        assert_eq!(reg.port, 8080);
    }

    #[test]
    fn test_get_peer_registration_not_found() {
        let tmp = TempDir::new().unwrap();
        let mesh_dir = tmp.path().join("mesh");
        std::fs::create_dir_all(&mesh_dir).unwrap();

        let discovery = test_discovery(&mesh_dir);
        let result = discovery.get_peer_registration("nonexistent");
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    // description_from_project_memory — peer-card blurb sourcing
    // ------------------------------------------------------------------------

    #[test]
    fn description_from_project_memory_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let desc = MeshDiscovery::description_from_project_memory(tmp.path());
        assert!(desc.is_none());
    }

    #[test]
    fn description_from_project_memory_returns_short_description_as_is() {
        let tmp = TempDir::new().unwrap();
        crate::memory::file_storage::save(
            tmp.path(),
            &crate::memory::ProjectMemory {
                id: "id".into(),
                project_id: "proj".into(),
                name: "demo".into(),
                description: "Short blurb.".into(),
                state: "active".into(),
                team_size: "solo".into(),
                goals: vec![],
                architecture: String::new(),
                tech_debt: String::new(),
                response_language: "en".into(),
                conventions: vec![],
                project_summary: String::new(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        let desc = MeshDiscovery::description_from_project_memory(tmp.path());
        assert_eq!(desc.as_deref(), Some("Short blurb."));
    }

    #[test]
    fn description_from_project_memory_truncates_long_descriptions() {
        let tmp = TempDir::new().unwrap();
        let long = "lorem ipsum ".repeat(40);
        crate::memory::file_storage::save(
            tmp.path(),
            &crate::memory::ProjectMemory {
                id: "id".into(),
                project_id: "proj".into(),
                name: "demo".into(),
                description: long.clone(),
                state: "active".into(),
                team_size: "solo".into(),
                goals: vec![],
                architecture: String::new(),
                tech_debt: String::new(),
                response_language: "en".into(),
                conventions: vec![],
                project_summary: String::new(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        let desc = MeshDiscovery::description_from_project_memory(tmp.path()).unwrap();
        assert!(desc.len() <= 203, "{} too long ({})", desc, desc.len());
        assert!(desc.ends_with("..."));
    }

    #[test]
    fn description_from_project_memory_returns_none_when_blank() {
        let tmp = TempDir::new().unwrap();
        crate::memory::file_storage::save(
            tmp.path(),
            &crate::memory::ProjectMemory {
                id: "id".into(),
                project_id: "proj".into(),
                name: "demo".into(),
                description: "   ".into(),
                state: "active".into(),
                team_size: "solo".into(),
                goals: vec![],
                architecture: String::new(),
                tech_debt: String::new(),
                response_language: "en".into(),
                conventions: vec![],
                project_summary: String::new(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        let desc = MeshDiscovery::description_from_project_memory(tmp.path());
        assert!(desc.is_none());
    }
}
