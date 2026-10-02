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
    ensure!(
        !metadata.permissions().readonly(),
        "Claude 会话只读，请核对权限和 systemd 会话目录写入白名单"
    );
    Ok(())
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
fn ensure_inactive(parsed: &Parsed, path: &Path) -> Result<()> {
    ensure!(
        parsed.issues.is_empty() && parsed.warnings.is_empty(),
        "Claude 会话格式或身份不明，管理操作已禁用"
    );
    ensure!(
        !parsed.turn_open
            && activity::Snapshot::capture().ownership(parsed, path)
                == activity::Ownership::Inactive,
        "Claude 会话仍被原生进程占用或归属无法确认，请关闭后重试"
    );
    Ok(())
}
pub fn preview_claude_delete(paths: &ClaudePaths, key: &str) -> Result<ClaudeDeletePreview> {
    let (path, parsed) = resolve(paths, key)?;
    checked_path(paths, &path)?;
    ensure_inactive(&parsed, &path)?;
    Ok(ClaudeDeletePreview {
        session_key: key.into(),
        id: parsed.id.clone(),
        title: parsed.title.clone(),
        bytes: fs::metadata(&path)?.len(),
        fingerprint: fingerprint(&path)?,
        path,
        file_count: 1,
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
    checked_path(paths, &path)?;
    ensure_inactive(&parsed, &path)?;
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
    ensure_inactive(&current, &path)?;
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
    checked_path(paths, &preview.path)?;
    ensure_inactive(&parsed, &preview.path)?;
    let quarantine = preview
        .path
        .with_file_name(format!(".nexushub-delete-{}", uuid::Uuid::new_v4()));
    fs::rename(&preview.path, &quarantine)
        .context("Claude 删除无法写入，请核对会话目录权限和 systemd 写入白名单")?;
    let result = (|| -> Result<()> {
        ensure!(
            fingerprint(&quarantine)? == preview.fingerprint,
            "Claude 文件在隔离时发生变化"
        );
        ensure_inactive(&parsed, &preview.path)?;
        ensure!(!preview.path.exists(), "Claude 会话已被原生进程重新创建");
        fs::remove_file(&quarantine)?;
        Ok(())
    })();
    if let Err(error) = result {
        if !preview.path.exists() {
            let _ = fs::rename(&quarantine, &preview.path);
        }
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
