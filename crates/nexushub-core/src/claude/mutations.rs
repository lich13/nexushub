use super::*;
use std::{
    collections::{BTreeSet, HashSet},
    fs::OpenOptions,
    io::{Read, Write},
    sync::Mutex,
};
static MUTATIONS: Mutex<()> = Mutex::new(());

fn open_regular(path: &Path) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_file(), "Claude 专属内容不是普通文件");
    Ok(file)
}

fn checked_path(paths: &ClaudePaths, path: &Path) -> Result<()> {
    let root = paths.projects.canonicalize()?;
    ensure!(path.starts_with(&root), "Claude 会话路径越界");
    let mut current = root.clone();
    for part in path.strip_prefix(&root)?.components() {
        current.push(part);
        ensure!(
            !fs::symlink_metadata(&current)?.file_type().is_symlink(),
            "Claude 会话路径不能包含符号链接"
        );
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(metadata.is_file(), "Claude 会话不是普通文件");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.nlink() == 1,
            "Claude 会话存在硬链接，管理操作已禁用"
        );
    }
    Ok(())
}
pub(super) fn check_rename_path(paths: &ClaudePaths, path: &Path) -> Result<()> {
    checked_path(paths, path)?;
    ensure!(
        !fs::metadata(path)?.permissions().readonly(),
        "Claude 会话只读，无法追加名称"
    );
    Ok(())
}
pub(super) fn check_delete_path(paths: &ClaudePaths, path: &Path) -> Result<()> {
    checked_path(paths, path)?;
    ensure!(
        !fs::metadata(path.parent().context("会话父目录缺失")?)?
            .permissions()
            .readonly(),
        "Claude 会话目录只读，无法删除"
    );
    let directory = path.with_extension("");
    if let Ok(metadata) = fs::symlink_metadata(&directory) {
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Claude 专属目录身份无法确认"
        );
    }
    Ok(())
}

struct OwnedFiles {
    paths: Vec<PathBuf>,
    bytes: u64,
    directories: Vec<PathBuf>,
    child_ids: HashSet<String>,
    warnings: Vec<String>,
    stamps: HashMap<PathBuf, reader::Stamp>,
    directory_stamps: HashMap<PathBuf, fs::Metadata>,
}

#[derive(Clone, Default)]
struct ChildIdentity {
    parents: BTreeSet<String>,
    sessions: BTreeSet<String>,
}
impl ChildIdentity {
    fn record(&mut self, row: &Value) -> Result<()> {
        ensure!(row.is_object(), "子代理原生记录不是对象");
        for (fields, ids) in [
            (["parentSessionId", "parent_session_id"], &mut self.parents),
            (["sessionId", "session_id"], &mut self.sessions),
        ] {
            for field in fields {
                if let Some(value) = row.get(field) {
                    let id = value
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .context("子代理身份字段损坏")?;
                    ids.insert(id.into());
                }
            }
        }
        Ok(())
    }
    fn verify(&self, parent: &str) -> Result<HashSet<String>> {
        ensure!(
            self.parents.iter().all(|id| id == parent),
            "子代理关联到其他主会话"
        );
        let children: HashSet<_> = self
            .sessions
            .iter()
            .filter(|id| *id != parent)
            .cloned()
            .collect();
        ensure!(children.len() <= 1, "子代理身份冲突");
        Ok(children)
    }
}

fn child_identity(path: &Path) -> Result<ChildIdentity> {
    static CACHE: crate::read_cache::ReadCache<ChildIdentity> =
        crate::read_cache::ReadCache::new(1024 * 1024).with_native_metadata();
    let before = reader::stamp(path)?;
    let result = CACHE.read(
        path,
        |identity, _| {
            128 + identity
                .parents
                .iter()
                .chain(&identity.sessions)
                .map(|id| id.len() as u64)
                .sum::<u64>()
        },
        || read_child_identity(path),
    )?;
    ensure!(
        before == reader::stamp(path)?,
        "子代理历史在核验期间发生变化"
    );
    Ok(result)
}

fn read_child_identity(path: &Path) -> Result<ChildIdentity> {
    use std::io::{BufRead, BufReader};
    let mut identity = ChildIdentity::default();
    ensure!(
        fs::metadata(path)?.len() <= 512 * 1024 * 1024,
        "子代理历史超出核验上限"
    );
    let mut reader = BufReader::new(open_regular(path)?);
    let mut line = String::new();
    let mut total = 0;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(32 * 1024 * 1024)
            .read_line(&mut line)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        ensure!(total <= 512 * 1024 * 1024, "子代理历史超出核验上限");
        ensure!(line.ends_with('\n'), "子代理历史含半行或超大记录");
        let row: Value = serde_json::from_str(&line).context("子代理历史损坏")?;
        identity.record(&row)?;
    }
    Ok(identity)
}

fn owned_files(main: &Path, parent: &str) -> Result<OwnedFiles> {
    let meta = fs::symlink_metadata(main)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "Claude 主文件身份已变化"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(meta.nlink() == 1, "Claude 主文件存在硬链接");
    }
    let directory = main.with_extension("");
    let mut paths = vec![main.to_path_buf()];
    let mut directories = Vec::new();
    let mut warnings = Vec::new();
    let mut children: HashMap<PathBuf, ChildIdentity> = HashMap::new();
    let mut stamps = HashMap::from([(main.to_path_buf(), reader::stamp(main)?)]);
    let mut directory_stamps = HashMap::new();
    if fs::symlink_metadata(&directory)
        .map(|_| true)
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(e)
            }
        })?
    {
        ensure!(
            main.file_stem().and_then(|name| name.to_str()) == Some(parent),
            "Claude 专属目录与主会话 ID 不一致"
        );
        for (count, entry) in walkdir::WalkDir::new(&directory)
            .follow_links(false)
            .max_depth(18)
            .sort_by_file_name()
            .into_iter()
            .enumerate()
        {
            let entry = entry?;
            ensure!(
                count < 20_000 && entry.depth() < 18,
                "Claude 专属目录超出核验上限"
            );
            let meta = fs::symlink_metadata(entry.path())?;
            ensure!(!meta.file_type().is_symlink(), "Claude 专属目录含符号链接");
            let relative = entry.path().strip_prefix(&directory)?;
            let known_directory = relative.as_os_str().is_empty()
                || relative.starts_with("subagents")
                || relative.starts_with("tool-results");
            if !known_directory {
                warnings.push(format!("未识别的会话专属内容：{}", relative.display()));
            }
            if meta.is_dir() {
                if known_directory && relative.components().count() > 1 {
                    warnings.push(format!("未识别的会话专属内容：{}", relative.display()));
                }
                ensure!(!meta.permissions().readonly(), "Claude 专属目录只读");
                directories.push(entry.path().to_path_buf());
                directory_stamps.insert(entry.path().to_path_buf(), meta);
                continue;
            }
            stamps.insert(entry.path().to_path_buf(), reader::stamp(entry.path())?);
            ensure!(meta.is_file(), "Claude 专属目录含非普通文件");
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(meta.nlink() == 1, "Claude 专属文件存在硬链接");
            }
            if relative.starts_with("subagents") {
                let name = entry.file_name().to_str().context("子代理文件名无法核验")?;
                if let Some(stem) = name
                    .strip_suffix(".jsonl")
                    .filter(|s| s.starts_with("agent-"))
                {
                    let identity = child_identity(entry.path())?;
                    let combined = children
                        .entry(entry.path().with_file_name(stem))
                        .or_default();
                    combined.parents.extend(identity.parents);
                    combined.sessions.extend(identity.sessions);
                } else if let Some(stem) = name
                    .strip_suffix(".meta.json")
                    .filter(|s| s.starts_with("agent-"))
                {
                    ensure!(meta.len() <= 1024 * 1024, "子代理元数据超出上限");
                    let row: Value =
                        serde_json::from_reader(open_regular(entry.path())?.take(1024 * 1024 + 1))
                            .context("子代理元数据损坏")?;
                    children
                        .entry(entry.path().with_file_name(stem))
                        .or_default()
                        .record(&row)?;
                } else {
                    warnings.push(format!("未识别的会话专属内容：{}", relative.display()));
                }
            }
            paths.push(entry.path().into());
        }
    }
    let mut bytes = 0;
    for path in &paths {
        bytes += fs::metadata(path)?.len();
        ensure!(
            bytes <= 512 * 1024 * 1024,
            "Claude 专属内容超出管理大小上限"
        );
    }
    let mut child_ids = HashSet::new();
    for identity in children.values() {
        child_ids.extend(identity.verify(parent)?);
    }
    let owned = OwnedFiles {
        paths,
        bytes,
        directories,
        child_ids,
        warnings,
        stamps,
        directory_stamps,
    };
    owned.check_stamps()?;
    Ok(owned)
}

impl OwnedFiles {
    fn check_stamps(&self) -> Result<()> {
        for (path, stamp) in &self.stamps {
            ensure!(
                *stamp == reader::stamp(path)?,
                "Claude 专属文件在核验期间发生变化"
            );
        }
        for (path, before) in &self.directory_stamps {
            let after = fs::symlink_metadata(path)?;
            ensure!(
                after.is_dir()
                    && !after.file_type().is_symlink()
                    && before.len() == after.len()
                    && before.modified()? == after.modified()?,
                "Claude 专属目录在核验期间发生变化"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(
                    (
                        before.dev(),
                        before.ino(),
                        before.ctime(),
                        before.ctime_nsec()
                    ) == (after.dev(), after.ino(), after.ctime(), after.ctime_nsec()),
                    "Claude 专属目录身份已变化"
                );
            }
        }
        Ok(())
    }
    fn fingerprint(&self, main: &Path, parent: &str) -> Result<String> {
        self.check_stamps()?;
        let directory = main.with_extension("");
        let mut hash = Sha256::new();
        hash.update(parent.as_bytes());
        for path in &self.paths {
            hash.update(if path == main {
                b"main".as_slice()
            } else {
                path.strip_prefix(&directory)?
                    .as_os_str()
                    .as_encoded_bytes()
            });
            hash.update(fingerprint(path)?);
        }
        for path in &self.directories {
            hash.update(
                path.strip_prefix(&directory)?
                    .as_os_str()
                    .as_encoded_bytes(),
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let meta = fs::metadata(path)?;
                hash.update(meta.dev().to_le_bytes());
                hash.update(meta.ino().to_le_bytes());
            }
        }
        self.check_stamps()?;
        Ok(hex::encode(hash.finalize()))
    }
}
fn fingerprint(path: &Path) -> Result<String> {
    fingerprint_prefix(path, u64::MAX)
}
fn fingerprint_prefix(path: &Path, limit: u64) -> Result<String> {
    let before = reader::stamp(path)?;
    let file = open_regular(path)?;
    ensure!(
        file.metadata()?.len() <= 512 * 1024 * 1024,
        "Claude 会话超出管理大小上限"
    );
    let mut hash = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        ensure!(m.nlink() == 1, "Claude 专属文件存在硬链接");
        hash.update(m.dev().to_le_bytes());
        hash.update(m.ino().to_le_bytes());
    }
    let mut file = file.take(limit.min(512 * 1024 * 1024 + 1));
    let mut buffer = [0; 65536];
    let mut total = 0;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(total <= 512 * 1024 * 1024, "Claude 会话超出管理大小上限");
        hash.update(&buffer[..n]);
    }
    ensure!(
        before == reader::stamp(path)?,
        "Claude 会话正在变化，请重新预览"
    );
    Ok(hex::encode(hash.finalize()))
}
fn ensure_inactive(paths: &ClaudePaths, parsed: &Parsed, path: &Path) -> Result<()> {
    ensure!(
        parsed.issues.is_empty(),
        "Claude 会话格式或身份不明，管理操作已禁用"
    );
    if let Some(reason) = activity::Snapshot::capture(paths)
        .assess(parsed, path)
        .block_reason()
    {
        anyhow::bail!(reason);
    }
    Ok(())
}
pub(super) fn deletion_assessment(
    parsed: &Parsed,
    path: &Path,
    snapshot: &activity::Snapshot,
) -> Result<activity::Assessment> {
    let owned = owned_files(path, &parsed.id)?;
    Ok(snapshot.assess_owned(parsed, &owned.paths, &owned.child_ids))
}
fn verify_main(path: &Path, expected: &Parsed) -> Result<()> {
    let current = reader::read(path)?;
    ensure!(
        current.issues.is_empty() && current.id == expected.id && current.cwd == expected.cwd,
        "Claude 主会话记录或身份已变化"
    );
    Ok(())
}
pub fn preview_claude_delete(paths: &ClaudePaths, key: &str) -> Result<ClaudeDeletePreview> {
    let (path, parsed) = resolve(paths, key)?;
    check_delete_path(paths, &path)?;
    ensure_inactive(paths, &parsed, &path)?;
    let owned = owned_files(&path, &parsed.id)?;
    if let Some(reason) = activity::Snapshot::capture(paths)
        .assess_owned(&parsed, &owned.paths, &owned.child_ids)
        .block_reason()
    {
        anyhow::bail!(reason);
    }
    let fingerprint = owned.fingerprint(&path, &parsed.id)?;
    verify_main(&path, &parsed)?;
    owned.check_stamps()?;
    Ok(ClaudeDeletePreview {
        session_key: key.into(),
        id: parsed.id.clone(),
        title: parsed.title.clone(),
        bytes: owned.bytes,
        fingerprint,
        warnings: (!owned.warnings.is_empty()).then_some(owned.warnings),
        paths: owned
            .paths
            .iter()
            .chain(owned.directories.iter())
            .cloned()
            .collect(),
        path,
        file_count: owned.paths.len(),
    })
}
pub fn rename_claude_session(
    paths: &ClaudePaths,
    key: &str,
    title: &str,
) -> Result<ClaudeSessionSummary> {
    let _lock = MUTATIONS
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude 管理锁不可用"))?;
    let title = title.trim();
    ensure!(
        !title.is_empty() && title.chars().count() <= 200 && !title.chars().any(char::is_control),
        "名称应为 1 至 200 字符"
    );
    let (path, parsed) = resolve(paths, key)?;
    check_rename_path(paths, &path)?;
    ensure_inactive(paths, &parsed, &path)?;
    let before = fingerprint(&path)?;
    let original_len = fs::metadata(&path)?.len();
    let mut options = OpenOptions::new();
    options.read(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .context("Claude 改名无法写入，请核对会话目录权限和 systemd 写入白名单")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let fd = file.metadata()?;
        let current = fs::symlink_metadata(&path)?;
        ensure!(
            fd.dev() == current.dev() && fd.ino() == current.ino(),
            "Claude 文件身份已变化"
        );
    }
    ensure!(
        before == fingerprint(&path)?,
        "Claude 会话已变化，请刷新后重试"
    );
    let current = reader::read(&path)?;
    ensure!(
        current.id == parsed.id && current.cwd == parsed.cwd,
        "Claude 会话身份已变化"
    );
    ensure_inactive(paths, &current, &path)?;
    let bytes = serde_json::to_vec(
        &serde_json::json!({"type":"custom-title","customTitle":title,"sessionId":parsed.id}),
    )?;
    let mut record = bytes;
    record.push(b'\n');
    // One append, no rewrite/truncation of native history. On failure leave the
    // original plus any appended bytes for recovery; never roll back new writer data.
    file.write_all(&record)
        .with_context(|| format!("Claude 改名未完成，原文件保留在 {}", path.display()))?;
    file.sync_all()?;
    ensure!(
        fs::metadata(&path)?.len() == original_len + record.len() as u64
            && fingerprint_prefix(&path, original_len)? == before,
        "Claude 文件在改名期间发生变化，原文件保留在 {}",
        path.display()
    );
    let result = claude_session_summary(paths, key)?;
    ensure!(
        result.id == parsed.id && result.title == title && result.read_error.is_none(),
        "Claude 改名读回未通过，文件已保留在 {}",
        path.display()
    );
    Ok(result)
}
pub fn execute_claude_delete(
    paths: &ClaudePaths,
    request: ClaudeDeleteRequest,
) -> Result<ClaudeDeleteResult> {
    let _lock = MUTATIONS
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude 管理锁不可用"))?;
    ensure!(request.confirmed, "删除需要明确确认");
    let preview = preview_claude_delete(paths, &request.session_key)?;
    ensure!(
        preview.fingerprint == request.fingerprint,
        "Claude 会话已变化，请重新预览"
    );
    let (_, parsed) = resolve(paths, &request.session_key)?;
    check_delete_path(paths, &preview.path)?;
    ensure_inactive(paths, &parsed, &preview.path)?;
    let quarantine = preview
        .path
        .with_file_name(format!(".nexushub-delete-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&quarantine)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o700))?;
    }
    let staged = quarantine.join(preview.path.file_name().context("Claude 会话文件名缺失")?);
    let directory = preview.path.with_extension("");
    let mut moved = Vec::new();
    let result = (|| -> Result<()> {
        for (source, target) in [
            (&preview.path, staged.clone()),
            (&directory, staged.with_extension("")),
        ] {
            if !source.try_exists()? {
                continue;
            }
            fs::rename(source, &target)?;
            moved.push((source.clone(), target));
        }
        #[cfg(test)]
        mutation_checkpoint(&staged)?;
        let owned = owned_files(&staged, &parsed.id)?;
        verify_main(&staged, &parsed)?;
        ensure!(
            owned.fingerprint(&staged, &parsed.id)? == preview.fingerprint,
            "Claude 文件在隔离时发生变化"
        );
        ensure_inactive(paths, &parsed, &preview.path)?;
        let activity = activity::Snapshot::capture(paths);
        if let Some(reason) = activity
            .assess_owned(&parsed, &owned.paths, &owned.child_ids)
            .block_reason()
        {
            anyhow::bail!("隔离后占用核验未通过：{reason}");
        }
        ensure!(
            !preview.path.try_exists()? && !directory.try_exists()?,
            "Claude 会话已被原生进程重新创建"
        );
        // Unlink only the exact verified inventory. Unknown additions prevent
        // directory removal and remain recoverable; never recursively erase it.
        for path in &owned.paths {
            ensure!(
                owned.stamps[path] == reader::stamp(path)?,
                "Claude 专属文件在删除前发生变化"
            );
            fs::remove_file(path)?;
            #[cfg(test)]
            mutation_checkpoint(path)?;
        }
        for path in owned.directories.iter().rev() {
            fs::remove_dir(path)?;
        }
        fs::remove_dir(&quarantine)?;
        ensure!(
            !preview.path.try_exists()? && !directory.try_exists()?,
            "Claude 会话删除后重新出现"
        );
        ensure!(
            !files(paths)?.contains(&preview.path),
            "Claude 会话列表定位键仍有残留"
        );
        if let Ok(mut located) = LOCATED_SESSIONS.lock() {
            located.remove(&request.session_key);
        }
        Ok(())
    })();
    if let Err(error) = result {
        for (source, target) in moved.iter().rev() {
            if !source.exists() && target.exists() {
                let _ = fs::rename(target, source);
            }
        }
        let _ = fs::remove_dir(&quarantine);
        let residuals = [&quarantine, &preview.path, &directory]
            .into_iter()
            .filter(|path| fs::symlink_metadata(path).is_ok())
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let remaining = if residuals.is_empty() {
            "未能确认最终残留状态".into()
        } else {
            format!("尚存内容：{}", residuals.join("；"))
        };
        anyhow::bail!(
            "删除可能部分完成：{error:#}；{remaining}；已删除文件无法完整回滚，请刷新后检查残留"
        );
    }
    Ok(ClaudeDeleteResult {
        session_key: request.session_key,
        deleted: true,
        bytes: preview.bytes,
    })
}

#[cfg(test)]
type MutationHook = Box<dyn FnMut(&Path) -> Result<()>>;
#[cfg(test)]
thread_local! { pub(super) static TEST_MUTATION: std::cell::RefCell<Option<MutationHook>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
fn mutation_checkpoint(path: &Path) -> Result<()> {
    TEST_MUTATION.with(|slot| match slot.borrow_mut().as_mut() {
        Some(hook) => hook(path),
        None => Ok(()),
    })
}
