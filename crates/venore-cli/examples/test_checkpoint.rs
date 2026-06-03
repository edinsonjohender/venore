use std::path::Path;
use venore_core::checkpoint::CheckpointManager;

fn main() {
    let project_path = Path::new(".");
    let checkpoint_mgr = CheckpointManager::new(project_path);

    println!("✓ CheckpointManager created");

    if checkpoint_mgr.exists() {
        println!("✓ Checkpoint exists at .venore/context-checkpoint.json");

        match checkpoint_mgr.load() {
            Ok(Some(checkpoint)) => {
                println!("✓ Checkpoint loaded successfully\n");
                println!("Checkpoint Details:");
                println!("  Version: {}", checkpoint.version);
                println!("  Project: {}", checkpoint.wizard_config.project_name);
                println!("  Description: {}", checkpoint.wizard_config.project_description);
                println!("  Total modules: {}", checkpoint.total_modules);
                println!("  Completed: {} modules", checkpoint.completed_module_ids.len());

                let info = checkpoint_mgr.get_info();
                println!("\nCheckpoint Info:");
                println!("  Completed: {}/{} modules", info.completed_count, info.total_count);
                println!("  Progress: {}%", info.progress_percent);

                let completed = checkpoint_mgr.get_completed_ids();
                println!("\nCompleted Module IDs:");
                for id in &completed {
                    println!("  ✓ {}", id);
                }

                println!("\nSelected Modules (from wizard_config):");
                for (i, name) in checkpoint.wizard_config.selected_module_names.iter().enumerate() {
                    let status = if completed.contains(name) {
                        "✓ DONE"
                    } else {
                        "⏳ PENDING"
                    };
                    println!("  {}. {} - {}", i + 1, name, status);
                }

                println!("\n🎉 All checkpoint operations working correctly!");
                println!("\n📊 Resume would generate {} remaining modules:",
                         checkpoint.wizard_config.selected_module_names.len() - completed.len());
                for name in &checkpoint.wizard_config.selected_module_names {
                    if !completed.contains(name) {
                        println!("  → {}", name);
                    }
                }
            }
            Ok(None) => println!("✗ Checkpoint file exists but returned None (corrupt?)"),
            Err(e) => println!("✗ Error loading checkpoint: {}", e),
        }
    } else {
        println!("✗ Checkpoint does not exist at .venore/context-checkpoint.json");
        println!("   Run the wizard and interrupt it to create a checkpoint");
    }
}
