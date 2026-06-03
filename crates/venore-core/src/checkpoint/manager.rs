use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Mutex;
use anyhow::{Context, Result};
use super::types::{Checkpoint, CheckpointConfig, CheckpointInfo, WizardConfig};

const CHECKPOINT_FILE: &str = ".venore/context-checkpoint.json";

pub struct CheckpointManager {
    checkpoint_path: PathBuf,
    checkpoint: Mutex<Option<Checkpoint>>,
}

impl CheckpointManager {
    pub fn new(project_path: &Path) -> Self {
        Self {
            checkpoint_path: project_path.join(CHECKPOINT_FILE),
            checkpoint: Mutex::new(None),
        }
    }

    /// Get the project path from checkpoint location
    /// The project path is always the parent of .venore/
    pub fn get_project_path(&self) -> PathBuf {
        self.checkpoint_path
            .parent()
            .and_then(|p| p.parent()) // .venore/ -> parent
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    // === I/O Operations ===

    pub fn exists(&self) -> bool {
        self.checkpoint_path.exists()
    }

    pub fn load(&self) -> Result<Option<Checkpoint>> {
        let parsed: Option<Checkpoint> =
            crate::utils::atomic_json::read_or_backup_corrupt(&self.checkpoint_path)
                .map_err(|e| anyhow::anyhow!("Failed to read checkpoint file: {}", e))?;

        if let Some(ref cp) = parsed {
            *self.checkpoint.lock().unwrap() = Some(cp.clone());
        }
        Ok(parsed)
    }

    pub fn save(&self) -> Result<()> {
        let checkpoint = self.checkpoint.lock().unwrap();
        let checkpoint = checkpoint.as_ref()
            .context("No checkpoint to save")?;

        crate::utils::atomic_json::write_atomic(&self.checkpoint_path, checkpoint)
            .map_err(|e| anyhow::anyhow!("Failed to save checkpoint: {}", e))?;

        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        if self.exists() {
            fs::remove_file(&self.checkpoint_path)?;
        }
        *self.checkpoint.lock().unwrap() = None;
        Ok(())
    }

    // === Lifecycle ===

    /// Initialize checkpoint with full wizard configuration
    pub fn initialize_with_wizard_config(
        &self,
        _project_path: PathBuf,
        wizard_config: WizardConfig,
        total_modules: usize,
    ) -> Result<()> {
        let checkpoint = Checkpoint {
            version: "2.0".to_string(),
            project_path: None,
            started_at: chrono::Utc::now(),
            last_updated_at: chrono::Utc::now(),
            wizard_config,
            total_modules,
            completed_module_ids: Vec::new(),
        };

        *self.checkpoint.lock().unwrap() = Some(checkpoint);
        self.save()?;
        Ok(())
    }

    /// Legacy initialize method for backward compatibility
    #[deprecated(note = "Use initialize_with_wizard_config instead")]
    pub fn initialize(
        &self,
        _project_path: PathBuf,
        config: CheckpointConfig,
        total_modules: usize,
    ) -> Result<()> {
        // Convert legacy config to new WizardConfig format
        use std::collections::HashMap;

        let wizard_config = WizardConfig {
            project_name: "Unknown".to_string(),
            project_description: "".to_string(),
            project_state: "Unknown".to_string(),
            team_size: "Unknown".to_string(),
            goals: vec![],
            depth_level: "Normal".to_string(),
            layers_to_generate: vec!["Basic Context".to_string()],
            exclusions: vec![],
            project_type: config.project_type,
            project_type_confidence: 1.0,
            project_metadata: HashMap::new(),
            total_files_scanned: 0,
            total_modules_detected: total_modules,
            module_names: vec![],
            selected_module_names: vec![],
            llm_provider: config.llm_provider,
            llm_model: config.model,
            analysis_depth: config.analysis_depth,
        };

        let checkpoint = Checkpoint {
            version: "2.0".to_string(),
            project_path: None,
            started_at: chrono::Utc::now(),
            last_updated_at: chrono::Utc::now(),
            wizard_config,
            total_modules,
            completed_module_ids: Vec::new(),
        };

        *self.checkpoint.lock().unwrap() = Some(checkpoint);
        self.save()?;
        Ok(())
    }

    pub fn mark_completed(&self, module_id: String) -> Result<()> {
        {
            let mut checkpoint = self.checkpoint.lock().unwrap();
            let checkpoint = checkpoint.as_mut()
                .context("No checkpoint initialized")?;

            if !checkpoint.completed_module_ids.contains(&module_id) {
                checkpoint.completed_module_ids.push(module_id);
                checkpoint.last_updated_at = chrono::Utc::now();
            }
        } // Lock released here

        self.save()
    }

    // === Queries ===

    pub fn get_completed_ids(&self) -> Vec<String> {
        self.checkpoint
            .lock()
            .unwrap()
            .as_ref()
            .map(|cp| cp.completed_module_ids.clone())
            .unwrap_or_default()
    }

    pub fn get_info(&self) -> CheckpointInfo {
        let checkpoint = self.checkpoint.lock().unwrap();

        match checkpoint.as_ref() {
            Some(cp) => CheckpointInfo {
                exists: true,
                completed_count: cp.completed_module_ids.len(),
                total_count: cp.total_modules,
                progress_percent: ((cp.completed_module_ids.len() * 100) / cp.total_modules.max(1)) as u8,
            },
            None => CheckpointInfo {
                exists: false,
                completed_count: 0,
                total_count: 0,
                progress_percent: 0,
            },
        }
    }
}
