// meta-hybrid_mount/src/core/planner.rs
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::{conf::config, defs};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileConflict {
    pub path: String,
    pub modules: Vec<String>,
}

#[derive(Debug)]
pub struct OverlayOperation {
    pub target: String,
    pub layers: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub magic_module_paths: Vec<PathBuf>,
    pub conflicts: Vec<FileConflict>,
    
    // For state tracking
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

// Recursive function to walk directories and map files to modules
fn walk_and_map(
    base_path: &Path, 
    relative: &Path, 
    module_id: &str, 
    file_map: &mut HashMap<String, Vec<String>>
) {
    let current_full = base_path.join(relative);
    
    if let Ok(entries) = fs::read_dir(&current_full) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let next_relative = relative.join(name);
            
            if path.is_dir() {
                walk_and_map(base_path, &next_relative, module_id, file_map);
            } else {
                let key = next_relative.to_string_lossy().to_string();
                file_map.entry(key).or_default().push(module_id.to_string());
            }
        }
    }
}

fn detect_conflicts(sorted_modules: &[(&String, &PathBuf)], partitions: &[String]) -> Vec<FileConflict> {
    let mut file_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_partitions = defs::BUILTIN_PARTITIONS.to_vec();
    // Convert String to &str for comparison
    let extra_parts_str: Vec<&str> = partitions.iter().map(|s| s.as_str()).collect();
    all_partitions.extend(extra_parts_str);

    for (id, base) in sorted_modules {
        for part in &all_partitions {
            let part_dir = base.join(part);
            if part_dir.is_dir() {
                walk_and_map(&base, Path::new(part), id, &mut file_map);
            }
        }
    }

    let mut conflicts = Vec::new();
    for (path, modules) in file_map {
        if modules.len() > 1 {
            conflicts.push(FileConflict {
                path,
                modules,
            });
        }
    }
    
    // Sort for consistent reporting
    conflicts.sort_by(|a, b| a.path.cmp(&b.path));
    conflicts
}

pub fn generate(config: &config::Config, mnt_base: &Path) -> Result<MountPlan> {
    let module_modes = config::load_module_modes();
    let mut active_modules: HashMap<String, PathBuf> = HashMap::new();

    // 1. Scan active modules from storage
    if let Ok(entries) = fs::read_dir(mnt_base) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Filter out system directories and self
                if name != "lost+found" && name != "meta-hybrid" {
                    active_modules.insert(name, entry.path());
                }
            }
        }
    }

    // Sort modules by ID descending (Z->A)
    let mut sorted_modules: Vec<(&String, &PathBuf)> = active_modules.iter().collect();
    sorted_modules.sort_by(|(id_a, _), (id_b, _)| id_b.cmp(id_a));

    // 2. Prepare partitions list
    let mut all_partitions = defs::BUILTIN_PARTITIONS.to_vec();
    let extra_parts: Vec<&str> = config.partitions.iter().map(|s| s.as_str()).collect();
    all_partitions.extend(extra_parts);

    // 3. Group modules
    let mut partition_overlay_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut magic_mount_modules: HashSet<PathBuf> = HashSet::new();
    let mut overlay_ids_set: HashSet<String> = HashSet::new();
    let mut magic_ids_set: HashSet<String> = HashSet::new();

    for (module_id, content_path) in &sorted_modules {
        let mode = module_modes.get(module_id.as_str()).map(|s| s.as_str()).unwrap_or("auto");
        
        if mode == "magic" {
            magic_mount_modules.insert(content_path.to_path_buf());
            magic_ids_set.insert(module_id.to_string());
            log::info!("Planner: Module '{}' assigned to Magic Mount", module_id);
        } else {
            // Auto mode: Check partitions
            let mut participates_in_overlay = false;
            for &part in &all_partitions {
                if content_path.join(part).is_dir() {
                    partition_overlay_map.entry(part.to_string()).or_default().push(content_path.to_path_buf());
                    participates_in_overlay = true;
                }
            }
            if participates_in_overlay {
                overlay_ids_set.insert(module_id.to_string());
            }
        }
    }

    // 4. Construct the Plan
    let mut plan = MountPlan::default();

    // Conflict Detection
    log::info!("Scanning for file conflicts...");
    plan.conflicts = detect_conflicts(&sorted_modules, &config.partitions);
    if !plan.conflicts.is_empty() {
        log::warn!("Detected {} file conflicts between modules:", plan.conflicts.len());
        for conflict in &plan.conflicts {
            log::warn!("  - {}: {:?}", conflict.path, conflict.modules);
        }
    }

    // Overlay Operations
    for (part, modules) in partition_overlay_map {
        plan.overlay_ops.push(OverlayOperation {
            target: format!("/{}", part),
            layers: modules,
        });
    }

    // Magic Mounts
    plan.magic_module_paths = magic_mount_modules.into_iter().collect();
    
    // Tracking IDs
    plan.overlay_module_ids = overlay_ids_set.into_iter().collect();
    plan.magic_module_ids = magic_ids_set.into_iter().collect();
    
    plan.overlay_module_ids.sort();
    plan.magic_module_ids.sort();

    Ok(plan)
}
