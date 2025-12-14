use std::ffi::{CStr, CString};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use anyhow::{Context, Result};
use log::{debug, warn};
use walkdir::WalkDir;
use libc::{c_int, c_ulong, c_char};
use crate::defs::HYMO_PROTOCOL_VERSION;

const DEV_PATH: &str = "/dev/hymo_ctl";
const HYMO_IOC_MAGIC: u8 = 0xE0;

const _IOC_NRBITS: u32 = 8;
const _IOC_TYPEBITS: u32 = 8;
const _IOC_SIZEBITS: u32 = 14;
const _IOC_DIRBITS: u32 = 2;

const _IOC_NRSHIFT: u32 = 0;
const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_NONE: u32 = 0;
const _IOC_WRITE: u32 = 1;
const _IOC_READ: u32 = 2;
const _IOC_READ_WRITE: u32 = 3;

const fn _ioc(dir: u32, type_: u8, nr: u8, size: usize) -> c_ulong {
    ((dir << _IOC_DIRSHIFT) |
     ((type_ as u32) << _IOC_TYPESHIFT) |
     ((nr as u32) << _IOC_NRSHIFT) |
     ((size as u32) << _IOC_SIZESHIFT)) as c_ulong
}

const fn _io(type_: u8, nr: u8) -> c_ulong {
    _ioc(_IOC_NONE, type_, nr, 0)
}

const fn _ior<T>(type_: u8, nr: u8) -> c_ulong {
    _ioc(_IOC_READ, type_, nr, std::mem::size_of::<T>())
}

const fn _iow<T>(type_: u8, nr: u8) -> c_ulong {
    _ioc(_IOC_WRITE, type_, nr, std::mem::size_of::<T>())
}

const fn _iowr<T>(type_: u8, nr: u8) -> c_ulong {
    _ioc(_IOC_READ_WRITE, type_, nr, std::mem::size_of::<T>())
}

const HYMO_IOC_ADD_RULE: c_ulong    = _iow::<HymoIoctlArg>(HYMO_IOC_MAGIC, 1);
const HYMO_IOC_DEL_RULE: c_ulong    = _iow::<HymoIoctlArg>(HYMO_IOC_MAGIC, 2);
const HYMO_IOC_HIDE_RULE: c_ulong   = _iow::<HymoIoctlArg>(HYMO_IOC_MAGIC, 3);
const HYMO_IOC_CLEAR_ALL: c_ulong   = _io(HYMO_IOC_MAGIC, 5);
const HYMO_IOC_GET_VERSION: c_ulong = _ior::<c_int>(HYMO_IOC_MAGIC, 6);
const HYMO_IOC_LIST_RULES: c_ulong  = _iowr::<HymoIoctlListArg>(HYMO_IOC_MAGIC, 7);
const HYMO_IOC_SET_DEBUG: c_ulong   = _iow::<c_int>(HYMO_IOC_MAGIC, 8);

#[repr(C)]
struct HymoIoctlArg {
    src: *const c_char,
    target: *const c_char,
    r#type: c_int,
}

#[repr(C)]
struct HymoIoctlListArg {
    buf: *mut c_char,
    size: usize,
}

#[derive(Debug, PartialEq)]
pub enum HymoFsStatus {
    Available,
    NotPresent,
    ProtocolMismatch,
}

pub struct HymoFs;

impl HymoFs {
    fn open_dev() -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEV_PATH)
            .with_context(|| format!("Failed to open {}", DEV_PATH))
    }

    pub fn check_status() -> HymoFsStatus {
        if !Path::new(DEV_PATH).exists() {
            return HymoFsStatus::NotPresent;
        }
        
        if let Some(ver) = Self::get_version() {
            if ver == HYMO_PROTOCOL_VERSION {
                HymoFsStatus::Available
            } else {
                debug!("HymoFS protocol mismatch: kernel={}, user={}", ver, HYMO_PROTOCOL_VERSION);
                HymoFsStatus::ProtocolMismatch
            }
        } else {
            HymoFsStatus::NotPresent
        }
    }

    pub fn is_available() -> bool {
        Self::check_status() == HymoFsStatus::Available
    }

    pub fn get_version() -> Option<i32> {
        let file = Self::open_dev().ok()?;
        let mut ver: c_int = 0;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_GET_VERSION as c_int, &mut ver)
        };
        if ret < 0 {
            None
        } else {
            Some(ver as i32)
        }
    }

    pub fn clear() -> Result<()> {
        debug!("HymoFS: Clearing all rules");
        let file = Self::open_dev()?;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_CLEAR_ALL as c_int)
        };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS clear failed: {}", err);
        }
        Ok(())
    }

    pub fn set_debug(enable: bool) -> Result<()> {
        let file = Self::open_dev()?;
        let val: c_int = if enable { 1 } else { 0 };
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_SET_DEBUG as c_int, &val)
        };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS set_debug failed: {}", err);
        }
        Ok(())
    }

    pub fn add_rule(src: &str, target: &str, type_val: i32) -> Result<()> {
        debug!("HymoFS: ADD_RULE src='{}' target='{}' type={}", src, target, type_val);
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;
        let c_target = CString::new(target)?;
        
        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: c_target.as_ptr(),
            r#type: type_val as c_int,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_ADD_RULE as c_int, &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS add_rule failed: {}", err);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_rule(src: &str) -> Result<()> {
        debug!("HymoFS: DEL_RULE src='{}'", src);
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;
        
        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: std::ptr::null(),
            r#type: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_DEL_RULE as c_int, &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS delete_rule failed: {}", err);
        }
        Ok(())
    }

    pub fn hide_path(path: &str) -> Result<()> {
        debug!("HymoFS: HIDE_RULE path='{}'", path);
        let file = Self::open_dev()?;
        let c_path = CString::new(path)?;
        
        let arg = HymoIoctlArg {
            src: c_path.as_ptr(),
            target: std::ptr::null(),
            r#type: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_HIDE_RULE as c_int, &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS hide_path failed: {}", err);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn list_active_rules() -> Result<String> {
        let file = Self::open_dev()?;
        let capacity = 128 * 1024;
        let mut buffer = vec![0u8; capacity];
        let mut arg = HymoIoctlListArg {
            buf: buffer.as_mut_ptr() as *mut c_char,
            size: capacity,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_LIST_RULES as c_int, &mut arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS list_rules failed: {}", err);
        }

        let c_str = unsafe { CStr::from_ptr(buffer.as_ptr() as *const c_char) };
        Ok(c_str.to_string_lossy().into_owned())
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::add_rule(
                    &target_path.to_string_lossy(),
                    &current_path.to_string_lossy(),
                    0 
                ) {
                    warn!("Failed to add rule for {}: {}", target_path.display(), e);
                }
            } else if file_type.is_char_device() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.rdev() == 0 {
                        if let Err(e) = Self::hide_path(&target_path.to_string_lossy()) {
                            warn!("Failed to hide path {}: {}", target_path.display(), e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_directory_rules(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::delete_rule(&target_path.to_string_lossy()) {
                    warn!("Failed to delete rule for {}: {}", target_path.display(), e);
                }
            } else if file_type.is_char_device() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.rdev() == 0 {
                        if let Err(e) = Self::delete_rule(&target_path.to_string_lossy()) {
                            warn!("Failed to delete hidden rule for {}: {}", target_path.display(), e);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
