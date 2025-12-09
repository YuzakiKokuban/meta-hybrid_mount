use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ffi::CString;

use anyhow::{Context, Result, bail};
use rustix::fs::{Mode, MountFlags, CStringExt};
use rustix::mount::{mount, umount, UnmountFlags};
use serde::Serialize;

use crate::{defs, utils, mount::hymofs::HymoFs};

const DEFAULT_SELINUX_CONTEXT: &str = "u:object_r:system_file:s0";
const SELINUX_XATTR_KEY: &str = "security.selinux";

pub struct StorageHandle {
    pub mount_point: PathBuf,
    pub mode: String,
}

#[derive(Serialize)]
struct StorageStatus {
    mode: String,
    mount_point: String,
    usage_percent: u8,
    total_size: u64,
    used_size: u64,
    hymofs_available: bool,
}

pub fn setup(mnt_base: &Path, img_path: &Path, force_ext4: bool) -> Result<StorageHandle> {
    if utils::is_mounted(mnt_base) {
        let _ = umount(mnt_base, UnmountFlags::DETACH);
    }
    fs::create_dir_all(mnt_base)?;

    if !force_ext4 {
        if try_setup_tmpfs(mnt_base)? {
            return Ok(StorageHandle {
                mount_point: mnt_base.to_path_buf(),
                mode: "tmpfs".to_string(),
            });
        }
    }

    setup_ext4_image(mnt_base, img_path)
}

fn try_setup_tmpfs(target: &Path) -> Result<bool> {
    match mount(
        Some(unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"tmpfs\0") }),
        target,
        Some(unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"tmpfs\0") }),
        MountFlags::empty(),
        Some(unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"mode=0755\0") }),
    ) {
        Ok(_) => {},
        Err(_) => return Ok(false),
    }

    if is_xattr_supported(target) {
        Ok(true)
    } else {
        let _ = umount(target, UnmountFlags::DETACH);
        Ok(false)
    }
}

fn is_xattr_supported(base: &Path) -> bool {
    let test_file = base.join(".xattr_test");
    if fs::write(&test_file, "test").is_err() {
        return false;
    }

    let res = set_selinux_context(&test_file, DEFAULT_SELINUX_CONTEXT).is_ok();
    let _ = fs::remove_file(test_file);
    res
}

fn setup_ext4_image(target: &Path, img_path: &Path) -> Result<StorageHandle> {
    if !img_path.exists() {
        if let Some(parent) = img_path.parent() {
            fs::create_dir_all(parent)?;
        }
        create_image(img_path).context("Failed to create modules.img")?;
    }

    if let Err(_) = mount_image(img_path, target) {
        if repair_image(img_path) {
            mount_image(img_path, target).context("Failed to mount modules.img after repair")?;
        } else {
            bail!("Failed to repair modules.img");
        }
    }

    Ok(StorageHandle {
        mount_point: target.to_path_buf(),
        mode: "ext4".to_string(),
    })
}

fn mount_image(img_path: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;

    let status = Command::new("mount")
        .arg("-o").arg("loop,rw,noatime")
        .arg(img_path)
        .arg(target)
        .status()?;

    if !status.success() {
        bail!("Mount command failed");
    }
    Ok(())
}

fn repair_image(img_path: &Path) -> bool {
    match Command::new("e2fsck")
        .arg("-y")
        .arg("-f")
        .arg(img_path)
        .status() 
    {
        Ok(status) => {
            if let Some(code) = status.code() {
                if code <= 2 {
                    return true;
                }
            }
        },
        Err(e) => log::error!("Failed to execute e2fsck: {}", e),
    }
    false
}

fn create_image(path: &Path) -> Result<()> {
    let status = Command::new("truncate")
        .arg("-s").arg("2G")
        .arg(path)
        .status()?;
    if !status.success() { bail!("Failed to allocate image file"); }

    let status = Command::new("mkfs.ext4")
        .arg("-O").arg("^has_journal")
        .arg(path)
        .status()?;
    if !status.success() { bail!("Failed to format image file"); }

    Ok(())
}

pub fn finalize_storage_permissions(target: &Path) {
    if let Err(e) = rustix::fs::chmod(target, Mode::from_raw(0o755)) {
        log::warn!("Failed to chmod storage root: {}", e);
    }

    if let Err(e) = rustix::fs::chown(target, Some(rustix::fs::Uid::from_raw(0)), Some(rustix::fs::Gid::from_raw(0))) {
        log::warn!("Failed to chown storage root: {}", e);
    }

    if let Err(e) = set_selinux_context(target, DEFAULT_SELINUX_CONTEXT) {
        log::warn!("Failed to set SELinux context: {}", e);
    }
}

fn set_selinux_context(path: &Path, context: &str) -> Result<()> {
    let c_path = CString::new(path.as_os_str().as_encoded_bytes())?;
    let c_val = CString::new(context)?;
    
    unsafe {
        let ret = libc::lsetxattr(
            c_path.as_ptr(),
            SELINUX_XATTR_KEY.as_ptr() as *const libc::c_char,
            c_val.as_ptr() as *const libc::c_void,
            c_val.as_bytes().len(),
            0
        );
        if ret != 0 {
            bail!("lsetxattr failed");
        }
    }
    Ok(())
}

pub fn print_status() -> Result<()> {
    let mnt_base = Path::new(defs::FALLBACK_CONTENT_DIR);
    let mut mode = "unknown".to_string();
    let mut total = 0;
    let mut used = 0;
    let mut percent = 0;

    if utils::is_mounted(mnt_base) {
        if let Ok(stat) = rustix::fs::statvfs(mnt_base) {
            mode = "active".to_string();

            total = stat.f_blocks * stat.f_frsize;
            let free = stat.f_bfree * stat.f_frsize;
            used = total - free;
            if total > 0 {
                percent = (used * 100 / total) as u8;
            }
        }
    }

    let status = StorageStatus {
        mode,
        mount_point: mnt_base.to_string_lossy().to_string(),
        usage_percent: percent,
        total_size: total,
        used_size: used,
        hymofs_available: HymoFs::is_available(),
    };

    println!("{}", serde_json::to_string(&status)?);
    Ok(())
}
