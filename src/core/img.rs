use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use hole_punch::{SegmentType, SparseFile};
use procfs::process::Process;
use rustix::{
    mount::{MountFlags, mount},
    path::Arg,
};
use serde::{Deserialize, Serialize};

use crate::{defs::SYSTEM_RW_DIR, utils::lsetfilecon};

pub fn copy_sparse_file<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
    punch_hole: bool,
) -> Result<()> {
    let mut src_file = File::open(src.as_ref())
        .with_context(|| format!("failed to open {}", src.as_ref().display()))?;
    let mut dst_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dst.as_ref())
        .with_context(|| format!("failed to open {}", dst.as_ref().display()))?;

    dst_file.set_len(src_file.metadata()?.len())?;

    let segments = src_file.scan_chunks()?;
    for segment in segments {
        if let SegmentType::Data = segment.segment_type {
            let start = segment.start;
            let end = segment.end + 1;

            src_file.seek(SeekFrom::Start(start))?;
            dst_file.seek(SeekFrom::Start(start))?;

            let mut buffer = [0; 4096];
            let mut total_bytes_copied = 0;

            while total_bytes_copied < end - start {
                let bytes_to_read =
                    std::cmp::min(buffer.len() as u64, end - start - total_bytes_copied);
                let bytes_read = src_file.read(&mut buffer[..bytes_to_read as usize])?;

                if bytes_read == 0 {
                    break;
                }

                if punch_hole && buffer[..bytes_read].iter().all(|&x| x == 0) {
                    dst_file.seek(SeekFrom::Current(bytes_read as i64))?;
                    total_bytes_copied += bytes_read as u64;
                    continue;
                }
                dst_file.write_all(&buffer[..bytes_read])?;
                total_bytes_copied += bytes_read as u64;
            }
        }
    }

    Ok(())
}
