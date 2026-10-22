//! Bounded, read-only access to the two physical representations of a rollout.
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

pub(crate) const MAX_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_LINE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_HEADER_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RolloutName {
    pub thread_id: String,
    pub rollout_id: String,
}

impl RolloutName {
    pub fn parse(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        let name = name.strip_suffix(".zst").unwrap_or(name);
        let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
        chrono::NaiveDateTime::parse_from_str(core.get(..19)?, "%Y-%m-%dT%H-%M-%S").ok()?;
        if core.get(19..20)? != "-" {
            return None;
        }
        let ids = core.get(20..)?;
        let (thread, rollout) = ids.split_once('_').unwrap_or((ids, ids));
        uuid::Uuid::parse_str(thread).ok()?;
        uuid::Uuid::parse_str(rollout).ok()?;
        Some(Self {
            thread_id: thread.into(),
            rollout_id: rollout.into(),
        })
    }
}

pub(crate) fn is_compressed(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
}

pub(crate) fn plain_path(path: &Path) -> PathBuf {
    if is_compressed(path) {
        path.with_extension("")
    } else {
        path.into()
    }
}

pub(crate) fn compressed_path(path: &Path) -> PathBuf {
    plain_path(path).with_extension("jsonl.zst")
}

pub(crate) fn open(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_file(), "原生历史不是普通文件");
    Ok(file)
}

pub(crate) struct RolloutReader {
    reader: BufReader<Box<dyn Read>>,
    remaining: u64,
}

impl RolloutReader {
    pub fn new(path: &Path, limit: u64) -> Result<Self> {
        let file = open(path)?.take(limit.max(MAX_HEADER_BYTES).saturating_add(1));
        let reader: Box<dyn Read> = if is_compressed(path) {
            let mut decoder = zstd::stream::read::Decoder::new(file)?;
            // Bound decoder allocations as well as the emitted record bytes.
            decoder.window_log_max(27)?;
            Box::new(decoder)
        } else {
            Box::new(file)
        };
        Ok(Self {
            reader: BufReader::new(reader),
            remaining: limit,
        })
    }

    pub fn next_record(&mut self, line_limit: u64) -> Result<Option<Value>> {
        let limit = line_limit.min(self.remaining);
        let mut line = Vec::new();
        let read = self
            .reader
            .by_ref()
            .take(limit + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            return Ok(None);
        }
        ensure!(read as u64 <= limit, "原生历史超出文件或单行读取上限");
        self.remaining -= read as u64;
        ensure!(line.ends_with(b"\n"), "线程存在半行，请关闭原生写入后重试");
        let value = serde_json::from_slice(&line).context("原生历史记录损坏")?;
        Ok(Some(value))
    }
}

pub(crate) fn header(path: &Path) -> Result<Value> {
    let value = RolloutReader::new(path, MAX_HEADER_BYTES)?
        .next_record(MAX_HEADER_BYTES)?
        .context("原生历史缺少文件头")?;
    ensure!(value["type"] == "session_meta", "未知原生历史格式");
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_names_distinguish_thread_and_rollout_without_merging_other_paths() {
        let thread = "11111111-1111-4111-8111-111111111111";
        let rollout = "22222222-2222-4222-8222-222222222222";
        let name = format!("rollout-2026-01-01T00-00-00-{thread}_{rollout}.jsonl.zst");
        let parsed = RolloutName::parse(Path::new(&name)).unwrap();
        assert_eq!(parsed.thread_id, thread);
        assert_eq!(parsed.rollout_id, rollout);
        assert_eq!(
            compressed_path(&plain_path(Path::new(&name))),
            Path::new(&name)
        );
        assert!(RolloutName::parse(Path::new(&name.replace("2026-01-01", "invalid"))).is_none());
    }

    #[test]
    fn decoded_records_obey_line_total_json_and_complete_frame_limits() {
        let root = std::env::temp_dir().join(format!("nexushub-rollout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("fixture.jsonl.zst");
        for (data, limit, line, valid) in [
            (b"{}\n{}\n".as_slice(), 6, 3, true),
            (b"{}\n{}\n".as_slice(), 5, 3, false),
            (b"{\"long\":1}\n".as_slice(), 20, 3, false),
            (b"invalid\n".as_slice(), 20, 20, false),
            (b"{}".as_slice(), 20, 20, false),
        ] {
            std::fs::write(&path, zstd::stream::encode_all(data, 0).unwrap()).unwrap();
            let result = (|| -> Result<()> {
                let mut reader = RolloutReader::new(&path, limit)?;
                while reader.next_record(line)?.is_some() {}
                Ok(())
            })();
            assert_eq!(result.is_ok(), valid, "{data:?}: {result:?}");
        }
        let mut compressed = zstd::stream::encode_all(b"{}\n".as_slice(), 0).unwrap();
        compressed.pop();
        std::fs::write(&path, compressed).unwrap();
        let mut reader = RolloutReader::new(&path, 20).unwrap();
        let result = (|| -> Result<()> {
            while reader.next_record(20)?.is_some() {}
            Ok(())
        })();
        assert!(result.is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
