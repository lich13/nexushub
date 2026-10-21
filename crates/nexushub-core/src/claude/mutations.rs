use super::*;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    sync::Mutex,
};
static MUTATIONS: Mutex<()> = Mutex::new(());

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
    fingerprint: String,
    bytes: u64,
    directories: Vec<PathBuf>,
}

fn child_identity(path: &Path, parent: &str) -> Result<()> {
    use std::io::{BufRead, BufReader};
    let mut associated = false;
    let mut child = None::<String>;
    ensure!(
        fs::metadata(path)?.len() <= 512 * 1024 * 1024,
        "子代理历史超出核验上限"
    );
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut line = String::new();
    loop {
        line.clear();
        if reader
            .by_ref()
            .take(32 * 1024 * 1024)
            .read_line(&mut line)?
            == 0
        {
            break;
        }
        ensure!(line.ends_with('\n'), "子代理历史含半行或超大记录");
        let row: Value = serde_json::from_str(&line).context("子代理历史损坏")?;
        if let Some(id) = row.get("parentSessionId").and_then(Value::as_str) {
            ensure!(id == parent, "子代理关联到其他主会话");
            associated = true;
        }
        if let Some(id) = row.get("sessionId").and_then(Value::as_str) {
            if id == parent {
                associated = true;
            } else if let Some(previous) = &child {
                ensure!(previous == id, "子代理身份冲突");
            } else {
                child = Some(id.into());
            }
        }
    }
    ensure!(associated, "子代理缺少可核验的父会话关联");
    Ok(())
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
            if let Some(first) = relative.components().next() {
                ensure!(
                    matches!(
                        first.as_os_str().to_str(),
                        Some("subagents" | "tool-results")
                    ),
                    "Claude 专属目录存在归属不明内容"
                );
            }
            if meta.is_dir() {
                ensure!(!meta.permissions().readonly(), "Claude 专属目录只读");
                directories.push(entry.path().to_path_buf());
                continue;
            }
            ensure!(meta.is_file(), "Claude 专属目录含非普通文件");
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(meta.nlink() == 1, "Claude 专属文件存在硬链接");
            }
            if relative.starts_with("subagents") {
                let name = entry.file_name().to_str().context("子代理文件名无法核验")?;
                if name.starts_with("agent-") && name.ends_with(".jsonl") {
                    child_identity(entry.path(), parent)?;
                } else if let Some(stem) = name
                    .strip_suffix(".meta.json")
                    .filter(|s| s.starts_with("agent-"))
                {
                    let transcript = entry.path().with_file_name(format!("{stem}.jsonl"));
                    child_identity(&transcript, parent)?;
                    ensure!(meta.len() <= 1024 * 1024, "子代理元数据超出上限");
                    let _: Value = serde_json::from_slice(&fs::read(entry.path())?)
                        .context("子代理元数据损坏")?;
                } else {
                    anyhow::bail!("子代理目录存在归属不明文件");
                }
            }
            paths.push(entry.path().into());
        }
    }
    let mut hash = Sha256::new();
    hash.update(parent.as_bytes());
    let mut bytes = 0;
    for path in &paths {
        hash.update(if path == main {
            b"main".as_slice()
        } else {
            path.strip_prefix(&directory)?
                .as_os_str()
                .as_encoded_bytes()
        });
        hash.update(fingerprint(path)?);
        bytes += fs::metadata(path)?.len();
    }
    for path in &directories {
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
    Ok(OwnedFiles {
        paths,
        fingerprint: hex::encode(hash.finalize()),
        bytes,
        directories,
    })
}
fn fingerprint(path: &Path) -> Result<String> {
    fingerprint_prefix(path, u64::MAX)
}
fn fingerprint_prefix(path: &Path, limit: u64) -> Result<String> {
    let before = reader::stamp(path)?;
    let file = fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= 512 * 1024 * 1024,
        "Claude 会话超出管理大小上限"
    );
    let mut hash = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        hash.update(m.dev().to_le_bytes());
        hash.update(m.ino().to_le_bytes());
    }
    let mut file = file.take(limit);
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
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
    ensure!(
        activity::Snapshot::capture(paths).ownership(parsed, path) == activity::Ownership::Inactive,
        "Claude 会话仍被原生进程占用或归属无法确认，请关闭后重试"
    );
    Ok(())
}
pub fn preview_claude_delete(paths: &ClaudePaths, key: &str) -> Result<ClaudeDeletePreview> {
    let (path, parsed) = resolve(paths, key)?;
    check_delete_path(paths, &path)?;
    ensure_inactive(paths, &parsed, &path)?;
    let owned = owned_files(&path, &parsed.id)?;
    Ok(ClaudeDeletePreview {
        session_key: key.into(),
        id: parsed.id.clone(),
        title: parsed.title.clone(),
        bytes: owned.bytes,
        fingerprint: owned.fingerprint,
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
    let staged = quarantine.join("session.jsonl");
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
        ensure!(
            owned.fingerprint == preview.fingerprint,
            "Claude 文件在隔离时发生变化"
        );
        ensure_inactive(paths, &parsed, &preview.path)?;
        let activity = activity::Snapshot::capture(paths);
        ensure!(
            owned
                .paths
                .iter()
                .all(|p| activity.ownership(&parsed, p) == activity::Ownership::Inactive),
            "隔离的 Claude 文件仍被原生进程占用或归属无法确认"
        );
        ensure!(
            !preview.path.try_exists()? && !directory.try_exists()?,
            "Claude 会话已被原生进程重新创建"
        );
        // Unlink only the exact verified inventory. Unknown additions prevent
        // directory removal and remain recoverable; never recursively erase it.
        for path in &owned.paths {
            fs::remove_file(path)?;
        }
        for path in owned.directories.iter().rev() {
            fs::remove_dir(path)?;
        }
        fs::remove_dir(&quarantine)?;
        ensure!(
            !preview.path.try_exists()? && !directory.try_exists()?,
            "Claude 会话删除后重新出现"
        );
        Ok(())
    })();
    if let Err(error) = result {
        for (source, target) in moved.iter().rev() {
            if !source.exists() && target.exists() {
                let _ = fs::rename(target, source);
            }
        }
        let _ = fs::remove_dir(&quarantine);
        let location = if quarantine.exists() {
            &quarantine
        } else {
            &preview.path
        };
        anyhow::bail!("删除失败：{error:#}；文件已保留在 {}", location.display());
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
