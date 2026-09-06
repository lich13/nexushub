use anyhow::Result;
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}

impl Stamp {
    fn read(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            size: metadata.len(),
            modified: metadata.modified().ok()?,
            #[cfg(unix)]
            identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
        })
    }
}

struct Entry<T> {
    path: PathBuf,
    stamp: Stamp,
    value: T,
    bytes: u64,
}

pub(super) struct ReadCache<T> {
    entries: Mutex<VecDeque<Entry<T>>>,
    max_bytes: u64,
}

impl<T: Clone> ReadCache<T> {
    pub(super) const fn new(max_bytes: u64) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            max_bytes,
        }
    }

    pub(super) fn read(
        &self,
        path: &Path,
        weight: impl Fn(&T, u64) -> u64,
        load: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let before = Stamp::read(path);
        if let Ok(mut entries) = self.entries.lock() {
            if let Some(index) = entries.iter().position(|entry| entry.path == path) {
                let entry = entries.remove(index).expect("known cache entry");
                if Some(&entry.stamp) == before.as_ref() {
                    let value = entry.value.clone();
                    entries.push_back(entry);
                    return Ok(value);
                }
            }
        }
        let value = load()?;
        // Never publish a snapshot while its source is being appended or replaced.
        if let Some(stamp) = before.filter(|stamp| Some(stamp) == Stamp::read(path).as_ref()) {
            let bytes = weight(&value, stamp.size);
            if bytes <= self.max_bytes {
                if let Ok(mut entries) = self.entries.lock() {
                    entries.retain(|entry| entry.path != path);
                    while entries.len() >= 64
                        || entries.iter().map(|entry| entry.bytes).sum::<u64>() + bytes
                            > self.max_bytes
                    {
                        if entries.pop_front().is_none() {
                            break;
                        }
                    }
                    entries.push_back(Entry {
                        path: path.to_path_buf(),
                        stamp,
                        value: value.clone(),
                        bytes,
                    });
                }
            }
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn rollout_read_cache_reuses_only_unchanged_files_and_is_bounded() {
        let path = std::env::temp_dir().join(format!("nexushub-cache-{}", uuid::Uuid::new_v4()));
        fs::write(&path, "one").unwrap();
        let cache = ReadCache::new(6);
        let reads = Cell::new(0);
        let read = || {
            cache
                .read(
                    &path,
                    |_, size| size,
                    || {
                        reads.set(reads.get() + 1);
                        Ok(fs::read_to_string(&path)?)
                    },
                )
                .unwrap()
        };
        assert_eq!(read(), "one");
        assert_eq!(read(), "one");
        assert_eq!(reads.get(), 1);
        fs::write(&path, "two").unwrap();
        assert_eq!(read(), "two");
        assert_eq!(reads.get(), 2);
        fs::write(&path, "too large").unwrap();
        read();
        read();
        assert_eq!(reads.get(), 4);
        fs::remove_file(&path).unwrap();
        assert!(cache
            .read(&path, |_, size| size, || Ok(fs::read_to_string(&path)?))
            .is_err());
    }
}
