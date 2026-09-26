//! Safe collection of SecondEgo-owned temporary execution directories.
//!
//! The collector only considers names in SecondEgo's private temp namespace,
//! never follows symlinks, skips a directory whose ownership lock is held by
//! a live run, and removes only entries older than the retention window.

use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const PREFIXES: [&str; 2] = ["secondego-remote-", "secondego-attempt-"];
const MARKER: &str = ".secondego-owned";
const LOCK: &str = ".secondego-lock";

#[derive(Debug, Clone)]
pub struct GcConfig {
    pub temp_root: PathBuf,
    pub retention: Duration,
    pub max_scan_entries: usize,
    pub max_size_bytes: u64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            temp_root: std::env::temp_dir(),
            retention: Duration::from_secs(24 * 60 * 60),
            max_scan_entries: 256,
            max_size_bytes: 2 * 1_000_000_000,
        }
    }
}

impl GcConfig {
    pub fn from_environment() -> Self {
        let mut config = Self::default();
        if let Some(value) = env_u64("SECONDEGO_GC_RETENTION_SECONDS") {
            config.retention = Duration::from_secs(value.min(30 * 24 * 60 * 60));
        }
        if let Some(value) = env_u64("SECONDEGO_GC_MAX_ENTRIES") {
            config.max_scan_entries = value.clamp(1, 10_000) as usize;
        }
        if let Some(value) = env_u64("SECONDEGO_GC_MAX_BYTES") {
            config.max_size_bytes = value.clamp(1_000_000, 100 * 1_000_000_000);
        }
        config
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GcReport {
    pub scanned: u64,
    pub deleted: u64,
    pub skipped_recent: u64,
    pub skipped_active: u64,
    pub skipped_unowned: u64,
    pub errors: u64,
    pub bytes_reclaimed: u64,
}

/// A lock and ownership marker for a temporary directory.
///
/// Dropping the lease removes the directory after the caller has finished its
/// work. If the process crashes, the lock is released by the OS and the next
/// collector pass can reclaim the stale directory.
#[derive(Debug)]
pub struct OwnedTempLease {
    root: PathBuf,
    lock: File,
}

impl OwnedTempLease {
    pub fn create(root: &Path, kind: &str) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "temporary root is not a directory",
            ));
        }
        let marker = root.join(MARKER);
        let mut marker_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(marker)?;
        writeln!(marker_file, "kind={kind}")?;
        writeln!(marker_file, "pid={}", std::process::id())?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(root.join(LOCK))?;
        lock.lock_exclusive()?;
        Ok(Self {
            root: root.to_path_buf(),
            lock,
        })
    }
}

impl Drop for OwnedTempLease {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn collect_garbage() -> GcReport {
    collect_garbage_with(&GcConfig::from_environment())
}

pub fn collect_garbage_with(config: &GcConfig) -> GcReport {
    let mut report = GcReport::default();
    let Ok(entries) = fs::read_dir(&config.temp_root) else {
        return report;
    };
    let mut candidates = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
                return None;
            }
            let metadata = fs::symlink_metadata(&path).ok()?;
            if !metadata.file_type().is_dir() {
                return None;
            }
            Some((path, metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(_, modified)| *modified);
    for (path, modified) in candidates.into_iter().take(config.max_scan_entries) {
        report.scanned += 1;
        if SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
            < config.retention
        {
            report.skipped_recent += 1;
            continue;
        }
        if !path.join(MARKER).is_file() {
            report.skipped_unowned += 1;
            continue;
        }
        let lock_path = path.join(LOCK);
        let Ok(lock) = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)
        else {
            report.errors += 1;
            continue;
        };
        if lock.try_lock_exclusive().is_err() {
            report.skipped_active += 1;
            continue;
        }
        let size = directory_size(&path, config.max_size_bytes).unwrap_or(0);
        if fs::remove_dir_all(&path).is_ok() {
            report.deleted += 1;
            report.bytes_reclaimed = report.bytes_reclaimed.saturating_add(size);
        } else {
            report.errors += 1;
        }
        let _ = lock.unlock();
    }
    report
}

fn directory_size(root: &Path, limit: u64) -> io::Result<u64> {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                total = total.saturating_add(entry.metadata()?.len());
                if total >= limit {
                    return Ok(total);
                }
            }
        }
    }
    Ok(total)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}
