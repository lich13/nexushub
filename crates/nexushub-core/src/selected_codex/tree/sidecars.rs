//! Exact per-thread byproducts. Never interpret them as rollout records.
use super::*;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    directory: bool,
    bytes: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64, u32),
}

impl Stamp {
    fn from_metadata(meta: &fs::Metadata) -> Result<Self> {
        ensure!(
            !meta.file_type().is_symlink() && (meta.is_file() || meta.is_dir()),
            "旁路内容包含符号链接或非普通文件"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(meta.is_dir() || meta.nlink() == 1, "旁路文件存在硬链接");
        }
        Ok(Self {
            directory: meta.is_dir(),
            bytes: if meta.is_file() { meta.len() } else { 0 },
            modified: meta.modified()?,
            #[cfg(unix)]
            identity: {
                use std::os::unix::fs::MetadataExt;
                (
                    meta.dev(),
                    meta.ino(),
                    meta.ctime(),
                    meta.ctime_nsec(),
                    meta.mode(),
                )
            },
        })
    }

    fn matches_for_unlink(&self, current: &Self) -> bool {
        if !self.directory {
            return self == current;
        }
        // Removing our own children changes directory timestamps. Its complete
        // stamp is checked before cleanup; unlink still pins device/inode/type.
        #[cfg(unix)]
        {
            current.directory
                && (self.identity.0, self.identity.1, self.identity.4)
                    == (current.identity.0, current.identity.1, current.identity.4)
        }
        #[cfg(not(unix))]
        {
            self == current
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Inventory {
    entries: BTreeMap<PathBuf, Stamp>,
}

fn metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn directory(paths: &CodexPaths, path: &Path) -> Result<bool> {
    let Some(meta) = metadata(path)? else {
        return Ok(false);
    };
    ensure_safe_path(&paths.home, path)?;
    ensure!(meta.is_dir(), "旁路目录类型无法核验");
    Ok(true)
}

fn count_entry(count: &mut usize, depth: usize) -> Result<()> {
    *count += 1;
    ensure!(
        *count <= MAX_FILES && depth < 34,
        "旁路扫描超出上限，无法完整核验"
    );
    Ok(())
}

impl Inventory {
    fn add(&mut self, paths: &CodexPaths, path: &Path, is_directory: bool) -> Result<()> {
        ensure_safe_path(&paths.home, path)?;
        let stamp = Stamp::from_metadata(&fs::symlink_metadata(path)?)?;
        ensure!(stamp.directory == is_directory, "旁路内容类型已变化");
        self.entries.insert(path.into(), stamp);
        Ok(())
    }

    pub(super) fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.entries.keys()
    }

    pub(super) fn bind(&self, digest: &mut Sha256) -> Result<u64> {
        for (path, stamp) in &self.entries {
            digest.update(serde_json::to_vec(&(
                path,
                stamp.directory,
                stamp.bytes,
                stamp.modified,
            ))?);
            #[cfg(unix)]
            digest.update(serde_json::to_vec(&stamp.identity)?);
        }
        Ok(self.entries.values().map(|stamp| stamp.bytes).sum())
    }

    fn cleanup(&self, paths: &CodexPaths) -> Vec<String> {
        let mut failed = Vec::<PathBuf>::new();
        // Freeze eligibility before removals change parent directory metadata.
        for (path, expected) in &self.entries {
            let checked = (|| -> Result<()> {
                if let Some(meta) = metadata(path)? {
                    ensure_safe_path(&paths.home, path)?;
                    ensure!(Stamp::from_metadata(&meta)? == *expected, "旁路内容已变化");
                }
                Ok(())
            })();
            if checked.is_err() {
                failed.push(path.clone());
            }
        }
        let mut ordered: Vec<_> = self.entries.iter().collect();
        ordered.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
        for (path, expected) in ordered {
            if failed.iter().any(|blocked| path.starts_with(blocked)) {
                continue;
            }
            if unlink(paths, path, expected).is_err() {
                failed.push(path.clone());
            }
        }
        failed
            .into_iter()
            .map(|path| path.display().to_string())
            .collect()
    }
}

pub(super) fn scan(paths: &CodexPaths, ids: &[String]) -> Result<Inventory> {
    ensure!(
        ids.iter().all(|id| uuid::Uuid::parse_str(id).is_ok()),
        "旁路线程身份无法核验"
    );
    let mut inventory = Inventory::default();
    let mut count = 0;
    let locks = paths.home.join("thread-writer-locks");
    if directory(paths, &locks)? {
        for id in ids {
            count_entry(&mut count, 1)?;
            let path = locks.join(format!("{id}.lock"));
            if metadata(&path)?.is_some() {
                inventory.add(paths, &path, false)?;
            }
        }
    }
    let snapshots = paths.home.join("shell_snapshots");
    if directory(paths, &snapshots)? {
        for entry in fs::read_dir(&snapshots)? {
            count_entry(&mut count, 1)?;
            let entry = entry?;
            if entry.file_name().to_str().is_some_and(|name| {
                ids.iter().any(|id| {
                    name.strip_prefix(id.as_str())
                        .is_some_and(|suffix| suffix.starts_with('.'))
                })
            }) {
                inventory.add(paths, &entry.path(), false)?;
            }
        }
    }
    let visualizations = paths.home.join("visualizations");
    if directory(paths, &visualizations)? {
        let mut owned_roots = Vec::<PathBuf>::new();
        for entry in walkdir::WalkDir::new(&visualizations)
            .follow_links(false)
            .max_depth(34)
        {
            let entry = entry?;
            count_entry(&mut count, entry.depth())?;
            let matches = entry
                .file_name()
                .to_str()
                .is_some_and(|name| ids.iter().any(|id| id == name));
            if matches {
                ensure!(
                    entry.file_type().is_dir(),
                    "可视化专属目录包含链接或类型不明"
                );
                owned_roots.push(entry.path().into());
            }
            if owned_roots
                .iter()
                .any(|root| entry.path().starts_with(root))
            {
                inventory.add(paths, entry.path(), entry.file_type().is_dir())?;
            }
        }
    }
    Ok(inventory)
}

// Resolve each parent from an open directory handle. An ancestor swapped for a
// symlink cannot redirect unlink outside codex_home; the final unlink never
// follows a link either. Only the previewed empty directories are removed.
#[cfg(unix)]
fn unlink(paths: &CodexPaths, path: &Path, expected: &Stamp) -> Result<()> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
    };
    let relative = path.strip_prefix(&paths.home)?;
    let mut components: Vec<_> = relative
        .components()
        .map(|part| match part {
            std::path::Component::Normal(name) => Ok(name),
            _ => anyhow::bail!("旁路路径包含不安全分量"),
        })
        .collect::<Result<_>>()?;
    let name = CString::new(components.pop().context("旁路路径为空")?.as_bytes())?;
    let mut parent = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&paths.home)?;
    for component in components {
        let name = CString::new(component.as_bytes())?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            return if error.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error.into())
            };
        }
        parent = unsafe { fs::File::from_raw_fd(fd) };
    }
    let flags = libc::O_RDONLY
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | libc::O_CLOEXEC
        | if expected.directory {
            libc::O_DIRECTORY
        } else {
            0
        };
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        return if error.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(error.into())
        };
    }
    let file = unsafe { fs::File::from_raw_fd(fd) };
    ensure!(
        expected.matches_for_unlink(&Stamp::from_metadata(&file.metadata()?)?),
        "旁路内容身份已变化"
    );
    let result = unsafe {
        libc::unlinkat(
            parent.as_raw_fd(),
            name.as_ptr(),
            if expected.directory {
                libc::AT_REMOVEDIR
            } else {
                0
            },
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn unlink(_: &CodexPaths, _: &Path, _: &Stamp) -> Result<()> {
    anyhow::bail!("当前平台无法安全清理旁路内容")
}

pub(super) fn finish(
    paths: &CodexPaths,
    preview: &TreePreview,
    cleanup: bool,
    outcome: &mut DeleteOutcome,
) {
    if cleanup {
        match lock_writers(paths, &preview.ids) {
            Ok(_guards) => outcome.residuals.extend(preview.sidecars.cleanup(paths)),
            Err(_) => {
                outcome.error = Some(
                    "主数据已删除，但旁路仍有活跃写入者或归属无法核验，请关闭占用后检查残留".into(),
                )
            }
        }
    }
    // Include paths created after the official request, but never delete them.
    match scan(paths, &preview.ids) {
        Ok(inventory) => outcome
            .residuals
            .extend(inventory.paths().map(|path| path.display().to_string())),
        Err(_) => {
            outcome
                .residuals
                .push("线程旁路：无法完整核验 writer 锁、shell 快照和可视化目录".into());
            for path in preview.sidecars.paths() {
                if !matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
                {
                    outcome.residuals.push(path.display().to_string());
                }
            }
        }
    }
    outcome.residuals.sort();
    outcome.residuals.dedup();
}
