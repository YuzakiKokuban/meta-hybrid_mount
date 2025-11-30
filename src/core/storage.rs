// src/core/storage.rs
use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};
use rustix::mount::{unmount, UnmountFlags};
use crate::{defs, utils, core::state};

/// Represents an active storage backend
pub struct StorageHandle {
    pub mount_point: PathBuf,
    pub mode: String, // "tmpfs" or "ext4"
}

/// Sets up the storage backend (Tmpfs or Ext4 Image)
pub fn setup(mnt_dir: &Path, image_path: &Path, force_ext4: bool) -> Result<StorageHandle> {
    log::info!("Setting up storage at {}", mnt_dir.display());

    // Clean up previous mounts if necessary to ensure a clean state
    if mnt_dir.exists() { 
        // Best effort unmount, ignore errors if not mounted
        let _ = unmount(mnt_dir, UnmountFlags::DETACH); 
    }
    utils::ensure_dir_exists(mnt_dir)?;

    let mode = if !force_ext4 && try_setup_tmpfs(mnt_dir)? {
        "tmpfs".to_string()
    } else {
        setup_ext4_image(mnt_dir, image_path)?
    };

    Ok(StorageHandle {
        mount_point: mnt_dir.to_path_buf(),
        mode,
    })
}

fn try_setup_tmpfs(target: &Path) -> Result<bool> {
    log::info!("Attempting Tmpfs mode...");
    if let Err(e) = utils::mount_tmpfs(target) {
        log::warn!("Tmpfs mount failed: {}. Falling back to Image.", e);
        return Ok(false);
    }

    if utils::is_xattr_supported(target) {
        log::info!("Tmpfs mode active (XATTR supported).");
        Ok(true)
    } else {
        log::warn!("Tmpfs does NOT support XATTR. Unmounting...");
        let _ = unmount(target, UnmountFlags::DETACH);
        Ok(false)
    }
}

fn setup_ext4_image(target: &Path, image_path: &Path) -> Result<String> {
    log::info!("Falling back to Ext4 Image mode...");
    if !image_path.exists() {
        bail!("modules.img not found at {}", image_path.display());
    }
    
    utils::mount_image(image_path, target)
        .context("Failed to mount modules.img")?;

    // CRITICAL FIX: Repair root permissions immediately after mount.
    // OverlayFS requires the lowerdir root to be accessible and have correct context.
    log::info!("Repairing storage root permissions...");
    
    // 1. Chmod 0755
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(target)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(target, perms)?;

    // 2. Chown 0:0 (Root:Root)
    // Using rustix for direct chown
    use rustix::fs::{chown, Uid, Gid};
    unsafe {
        chown(target, Some(Uid::from_raw(0)), Some(Gid::from_raw(0)))
            .context("Failed to chown storage root")?;
    }

    // 3. Restore SELinux Context (u:object_r:system_file:s0 is standard for system overlays)
    utils::lsetfilecon(target, "u:object_r:system_file:s0")
        .context("Failed to set SELinux context on storage root")?;
        
    log::info!("Image mode active and secured.");
    Ok("ext4".to_string())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB { format!("{:.1}G", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.0}M", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.0}K", bytes as f64 / KB as f64) }
    else { format!("{}B", bytes) }
}

pub fn print_status() -> Result<()> {
    let state = state::RuntimeState::load().unwrap_or_default();
    
    let path = if state.mount_point.as_os_str().is_empty() {
        PathBuf::from(defs::FALLBACK_CONTENT_DIR)
    } else {
        state.mount_point
    };
    
    if !path.exists() {
        println!("{{ \"error\": \"Not mounted\" }}");
        return Ok(());
    }

    let fs_type = if state.storage_mode.is_empty() {
        "unknown".to_string()
    } else {
        state.storage_mode
    };

    let stats = rustix::fs::statvfs(&path).context("statvfs failed")?;
    let block_size = stats.f_frsize as u64;
    let total_bytes = stats.f_blocks as u64 * block_size;
    let free_bytes = stats.f_bfree as u64 * block_size;
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let percent = if total_bytes > 0 { (used_bytes as f64 / total_bytes as f64) * 100.0 } else { 0.0 };
    
    println!(
        "{{ \"size\": \"{}\", \"used\": \"{}\", \"percent\": \"{:.0}%\", \"type\": \"{}\" }}",
        format_size(total_bytes),
        format_size(used_bytes),
        percent,
        fs_type
    );
    Ok(())
}
