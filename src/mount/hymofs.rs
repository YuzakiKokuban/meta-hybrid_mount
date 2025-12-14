use std::ffi::CString;
use std::fs::File;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::fd::AsRawFd;
use std::path::Path;
use anyhow::{Context, Result};
use walkdir::WalkDir;
use crate::defs::HYMO_PROTOCOL_VERSION;

const HYMO_DEV: &str = "/dev/hymo_ctl";
const HYMO_IOC_MAGIC: u8 = 0xE0;

nix::ioctl_write_ptr!(hymo_add_rule, HYMO_IOC_MAGIC, 1, HymoIoctlArg);
nix::ioctl_write_ptr!(hymo_del_rule, HYMO_IOC_MAGIC, 2, HymoIoctlArg);
nix::ioctl_write_ptr!(hymo_hide_rule, HYMO_IOC_MAGIC, 3, HymoIoctlArg);
nix::ioctl_none!(hymo_clear_all, HYMO_IOC_MAGIC, 5);
nix::ioctl_read!(hymo_get_version, HYMO_IOC_MAGIC, 6, i32);
nix::ioctl_write_ptr!(hymo_set_debug, HYMO_IOC_MAGIC, 8, i32);

#[repr(C)]
struct HymoIoctlArg {
    src: *const i8,
    target: *const i8,
    type_: i32,
}

#[derive(Debug, PartialEq)]
pub enum HymoFsStatus {
    Available,
    NotPresent,
    ProtocolMismatch,
    KernelTooOld,
    ModuleTooOld,
}

pub struct HymoFs;

impl HymoFs {
    pub fn is_available() -> bool {
        Path::new(HYMO_DEV).exists()
    }

    pub fn check_status() -> HymoFsStatus {
        if !Self::is_available() {
            return HymoFsStatus::NotPresent;
        }

        match Self::get_version() {
            Some(v) => {
                if v == HYMO_PROTOCOL_VERSION {
                    HymoFsStatus::Available
                } else if v < HYMO_PROTOCOL_VERSION {
                    HymoFsStatus::KernelTooOld
                } else {
                    HymoFsStatus::ModuleTooOld
                }
            },
            None => HymoFsStatus::ProtocolMismatch
        }
    }

    pub fn get_version() -> Option<i32> {
        let file = File::open(HYMO_DEV).ok()?;
        let mut version: i32 = 0;
        unsafe {
            hymo_get_version(file.as_raw_fd(), &mut version).ok()?;
        }
        Some(version)
    }

    pub fn set_debug(enable: bool) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let val: i32 = if enable { 1 } else { 0 };
        unsafe {
            hymo_set_debug(file.as_raw_fd(), &val)?;
        }
        Ok(())
    }

    pub fn clear() -> Result<()> {
        let file = File::open(HYMO_DEV).context("Failed to open HymoFS control device")?;
        unsafe {
            hymo_clear_all(file.as_raw_fd())?;
        }
        Ok(())
    }

    pub fn add_rule(src: &str, target: &str, type_: i32) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let c_src = CString::new(src)?;
        let c_target = CString::new(target)?;

        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: c_target.as_ptr(),
            type_,
        };

        unsafe {
            hymo_add_rule(file.as_raw_fd(), &arg)?;
        }
        Ok(())
    }

    pub fn hide_path(target: &str) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let c_target = CString::new(target)?;

        let arg = HymoIoctlArg {
            src: c_target.as_ptr(),
            target: std::ptr::null(),
            type_: 0,
        };

        unsafe {
            hymo_hide_rule(file.as_raw_fd(), &arg)?;
        }
        Ok(())
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = entry?;
            let current_path = entry.path();
            
            let rel_path = current_path.strip_prefix(module_dir)?;
            
            let target_path = target_base.join(rel_path);
            let target_str = target_path.to_string_lossy();
            let src_str = current_path.to_string_lossy();

            let metadata = entry.metadata()?;
            let file_type = metadata.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::add_rule(&src_str, &target_str, 0) {
                    log::warn!("Failed to add rule for {}: {}", target_str, e);
                }
            } else if file_type.is_char_device() {
                if metadata.rdev() == 0 {
                    if let Err(e) = Self::hide_path(&target_str) {
                        log::warn!("Failed to hide path {}: {}", target_str, e);
                    }
                }
            }
        }
        Ok(())
    }
}
