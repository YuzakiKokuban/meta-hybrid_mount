mod config;
mod defs;
mod utils;

#[path = "magic_mount/mod.rs"]
mod magic_mount;
mod overlay_mount;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufRead, BufReader};
use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use config::{Config, CONFIG_FILE_DEFAULT};
use rustix::mount::{unmount, UnmountFlags};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "meta-hybrid", version, about = "Hybrid Mount Metamodule")]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,
    #[arg(short = 'm', long = "moduledir")]
    moduledir: Option<PathBuf>,
    #[arg(short = 't', long = "tempdir")]
    tempdir: Option<PathBuf>,
    #[arg(short = 's', long = "mountsource")]
    mountsource: Option<String>,
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    #[arg(short = 'p', long = "partitions", value_delimiter = ',')]
    partitions: Vec<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    GenConfig {
        #[arg(short = 'o', long = "output", default_value = CONFIG_FILE_DEFAULT)]
        output: PathBuf,
    },
    ShowConfig,
    /// Output storage usage in JSON format
    Storage,
    /// List modules in JSON format
    Modules,
}

#[derive(Serialize)]
struct ModuleInfo {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    // Calculated based on config
    mode: String,
}

const BUILTIN_PARTITIONS: &[&str] = &["system", "vendor", "product", "system_ext", "odm", "oem"];

fn load_config(cli: &Cli) -> Result<Config> {
    if let Some(config_path) = &cli.config {
        return Config::from_file(config_path);
    }
    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            if Path::new(CONFIG_FILE_DEFAULT).exists() {
                eprintln!("Error loading config: {:#}", e);
            }
            Ok(Config::default())
        }
    }
}

// Helper to read props like "name=Foo" from a file
fn read_prop(path: &Path, key: &str) -> Option<String> {
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if line.starts_with(key) && line.chars().nth(key.len()) == Some('=') {
                return Some(line[key.len() + 1..].to_string());
            }
        }
    }
    None
}

// --- Smart Storage Logic ---

fn setup_storage(mnt_dir: &Path, image_path: &Path) -> Result<String> {
    log::info!("Setting up storage at {}", mnt_dir.display());

    // 1. Try Tmpfs first (Performance & Stealth)
    log::info!("Attempting Tmpfs mode...");
    if let Err(e) = utils::mount_tmpfs(mnt_dir) {
        log::warn!("Tmpfs mount failed: {}. Falling back to Image.", e);
    } else {
        // Check for XATTR support (Crucial for SELinux)
        if utils::is_xattr_supported(mnt_dir) {
            log::info!("Tmpfs mode active (XATTR supported).");
            return Ok("tmpfs".to_string());
        } else {
            log::warn!("Tmpfs does NOT support XATTR (CONFIG_TMPFS_XATTR missing?). Unmounting...");
            let _ = unmount(mnt_dir, UnmountFlags::DETACH);
        }
    }

    // 2. Fallback to Ext4 Image
    log::info!("Falling back to Ext4 Image mode...");
    if !image_path.exists() {
        anyhow::bail!("modules.img not found at {}", image_path.display());
    }
    
    utils::mount_image(image_path, mnt_dir)
        .context("Failed to mount modules.img")?;
        
    log::info!("Image mode active.");
    Ok("ext4".to_string())
}

fn sync_active_modules(source_dir: &Path, target_base: &Path) -> Result<()> {
    log::info!("Syncing modules from {} to {}", source_dir.display(), target_base.display());
    
    let ids = scan_enabled_module_ids(source_dir)?;
    if ids.is_empty() {
        log::info!("No enabled modules to sync.");
        return Ok(());
    }

    for id in ids {
        let src = source_dir.join(&id);
        let dst = target_base.join(&id);
        
        // Only sync if source has system/vendor/etc content
        let has_content = BUILTIN_PARTITIONS.iter().any(|p| src.join(p).exists());
        
        if has_content {
            log::debug!("Syncing module: {}", id);
            if let Err(e) = utils::sync_dir(&src, &dst) {
                log::error!("Failed to sync module {}: {}", id, e);
            }
        }
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn check_storage() -> Result<()> {
    let path = Path::new(defs::MODULE_CONTENT_DIR);
    if !path.exists() {
        println!("{{ \"error\": \"Not mounted\" }}");
        return Ok(());
    }

    let stats = rustix::fs::statvfs(path).context("statvfs failed")?;
    
    let block_size = stats.f_frsize as u64;
    let total_bytes = stats.f_blocks as u64 * block_size;
    let free_bytes = stats.f_bfree as u64 * block_size;
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    
    let percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "{{ \"size\": \"{}\", \"used\": \"{}\", \"percent\": \"{:.0}%\" }}",
        format_size(total_bytes),
        format_size(used_bytes),
        percent
    );
    Ok(())
}

fn list_modules(cli: &Cli) -> Result<()> {
    // 1. Load config to get module dir and modes
    let config = load_config(cli)?;
    let module_modes = config::load_module_modes();
    let modules_dir = config.moduledir;
    
    let mut modules = Vec::new();

    if modules_dir.exists() {
        for entry in fs::read_dir(&modules_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if !path.is_dir() { continue; }
            
            let id = entry.file_name().to_string_lossy().to_string();
            
            // Filters
            if id == "meta-hybrid" || id == "lost+found" { continue; }
            if path.join(defs::DISABLE_FILE_NAME).exists() || 
               path.join(defs::REMOVE_FILE_NAME).exists() || 
               path.join(defs::SKIP_MOUNT_FILE_NAME).exists() {
                continue;
            }

            // Check content (system/vendor/etc...)
            // We also check mnt dir in case it's only in image (legacy support)
            let mnt_path = Path::new(defs::MODULE_CONTENT_DIR).join(&id);
            let has_content = BUILTIN_PARTITIONS.iter().any(|p| {
                path.join(p).exists() || mnt_path.join(p).exists()
            });

            if has_content {
                let prop_path = path.join("module.prop");
                let name = read_prop(&prop_path, "name").unwrap_or_else(|| id.clone());
                let version = read_prop(&prop_path, "version").unwrap_or_default();
                let author = read_prop(&prop_path, "author").unwrap_or_default();
                let description = read_prop(&prop_path, "description").unwrap_or_default();
                
                let mode = module_modes.get(&id).cloned().unwrap_or_else(|| "auto".to_string());

                modules.push(ModuleInfo {
                    id,
                    name,
                    version,
                    author,
                    description,
                    mode,
                });
            }
        }
    }

    // Sort by name
    modules.sort_by(|a, b| a.name.cmp(&b.name));

    let json = serde_json::to_string(&modules)?;
    println!("{}", json);
    Ok(())
}

// --- Main Logic (Wrapped) ---

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle Subcommands
    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => {
                Config::default().save_to_file(output)?;
                return Ok(());
            },
            Commands::ShowConfig => {
                println!("{:#?}", load_config(&cli)?);
                return Ok(());
            },
            Commands::Storage => {
                check_storage()?;
                return Ok(());
            },
            Commands::Modules => {
                list_modules(&cli)?;
                return Ok(());
            }
        }
    }

    let mut config = load_config(&cli)?;
    config.merge_with_cli(cli.moduledir, cli.tempdir, cli.mountsource, cli.verbose, cli.partitions);

    utils::init_logger(config.verbose, Path::new(defs::DAEMON_LOG_FILE))?;
    log::info!("Hybrid Mount Starting...");

    // 1. Prepare Storage (The Smart Fallback)
    let mnt_base = Path::new(defs::MODULE_CONTENT_DIR); // /data/adb/meta-hybrid/mnt/
    let img_path = Path::new(defs::MODULE_CONTENT_DIR).parent().unwrap().join("modules.img");
    
    // Ensure clean state
    if mnt_base.exists() {
        let _ = unmount(mnt_base, UnmountFlags::DETACH);
    }

    let storage_mode = setup_storage(mnt_base, &img_path)?;
    
    // 2. Populate Storage (Sync from /data/adb/modules)
    if let Err(e) = sync_active_modules(&config.moduledir, mnt_base) {
        log::error!("Critical: Failed to sync modules: {:#}", e);
    }

    // 3. Scan & Group (Proceeds with existing logic using 'mnt' as source)
    let module_modes = config::load_module_modes();
    let mut active_modules: HashMap<String, PathBuf> = HashMap::new();
    
    // Scan the NOW POPULATED mnt directory
    if let Ok(entries) = fs::read_dir(mnt_base) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let id = entry.file_name().to_string_lossy().to_string();
                active_modules.insert(id, entry.path());
            }
        }
    }
    log::info!("Loaded {} modules from storage ({})", active_modules.len(), storage_mode);

    // 4. Partition Grouping
    let mut partition_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut magic_force_map: HashMap<String, bool> = HashMap::new();
    
    let mut all_partitions = BUILTIN_PARTITIONS.to_vec();
    let extra_parts: Vec<&str> = config.partitions.iter().map(|s| s.as_str()).collect();
    all_partitions.extend(extra_parts);

    for (module_id, content_path) in active_modules {
        let mode = module_modes.get(&module_id).map(|s| s.as_str()).unwrap_or("auto");
        let is_magic = mode == "magic";

        for &part in &all_partitions {
            let part_dir = content_path.join(part);
            if part_dir.is_dir() {
                partition_map.entry(part.to_string())
                    .or_default()
                    .push(content_path.clone()); 
                
                if is_magic {
                    magic_force_map.insert(part.to_string(), true);
                    log::info!("Partition /{} forced to Magic Mount by module '{}'", part, module_id);
                }
            }
        }
    }

    // 5. Execute Mounts
    // Use robust select_temp_dir
    let tempdir = if let Some(t) = &config.tempdir { t.clone() } else { utils::select_temp_dir()? };
    let mut magic_modules: HashSet<PathBuf> = HashSet::new();

    // First pass: OverlayFS
    for (part, modules) in &partition_map {
        let use_magic = *magic_force_map.get(part).unwrap_or(&false);
        if !use_magic {
            let target_path = format!("/{}", part);
            let overlay_paths: Vec<String> = modules.iter()
                .map(|m| m.join(part).display().to_string())
                .collect();
            
            log::info!("Mounting {} [OVERLAY] ({} layers)", target_path, overlay_paths.len());
            if let Err(e) = overlay_mount::mount_overlay(&target_path, &overlay_paths, None, None) {
                log::error!("OverlayFS mount failed for {}: {:#}, falling back to Magic Mount", target_path, e);
                magic_force_map.insert(part.to_string(), true);
            }
        }
    }

    // Second pass: Magic Mount
    for (part, _) in &partition_map {
        if *magic_force_map.get(part).unwrap_or(&false) {
            if let Some(mods) = partition_map.get(part) {
                for m in mods {
                    magic_modules.insert(m.clone());
                }
            }
        }
    }

    if !magic_modules.is_empty() {
        log::info!("Starting Magic Mount Engine...");
        utils::ensure_temp_dir(&tempdir).context(format!("Failed to create temp dir at {}", tempdir.display()))?;
        
        let module_list: Vec<PathBuf> = magic_modules.into_iter().collect();
        
        if let Err(e) = magic_mount::mount_partitions(
            &tempdir, 
            &module_list, 
            &config.mountsource, 
            &config.partitions
        ) {
            log::error!("Magic Mount failed: {:#}", e);
        }
        
        utils::cleanup_temp_dir(&tempdir);
    }

    log::info!("Hybrid Mount Completed");
    Ok(())
}

fn scan_enabled_module_ids(metadata_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if !metadata_dir.exists() { return Ok(ids); }

    for entry in fs::read_dir(metadata_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let id = entry.file_name().to_string_lossy().to_string();
            // Ignore meta-hybrid self-directory and standard ignore files
            if id == "meta-hybrid" || id == "lost+found" { continue; }
            if path.join(defs::DISABLE_FILE_NAME).exists() || 
               path.join(defs::REMOVE_FILE_NAME).exists() || 
               path.join(defs::SKIP_MOUNT_FILE_NAME).exists() {
                continue;
            }
            ids.push(id);
        }
    }
    Ok(ids)
}

fn main() {
    if let Err(e) = run() {
        log::error!("Fatal Error: {:#}", e);
        eprintln!("Fatal Error: {:#}", e);
        std::process::exit(1);
    }
}
