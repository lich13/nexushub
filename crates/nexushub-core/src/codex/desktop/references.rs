//! References selected by native Rust controls/catalogs. This is an encrypted
//! internal queue format, never a request DTO and never an arbitrary UI path.
use super::{DesktopError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileIdentity {
    device: u64,
    inode: u64,
    size: u64,
    modified: i64,
    modified_ns: i64,
    changed: i64,
    changed_ns: i64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeReference {
    File {
        name: String,
        path: PathBuf,
        identity: FileIdentity,
        image: bool,
    },
    Skill {
        name: String,
        path: PathBuf,
        identity: Option<FileIdentity>,
    },
    App {
        name: String,
        path: String,
    },
    Plugin {
        name: String,
        path: String,
    },
}

impl NativeReference {
    pub fn selected_file(path: PathBuf) -> Result<Self> {
        let path = path
            .canonicalize()
            .map_err(|_| DesktopError::InvalidInput)?;
        safe_path(&path)?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(DesktopError::InvalidInput)?
            .to_owned();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| DesktopError::InvalidInput)?;
        let identity =
            FileIdentity::read(&file.metadata().map_err(|_| DesktopError::InvalidInput)?)?;
        let mut header = [0_u8; 16];
        let read = file
            .read(&mut header)
            .map_err(|_| DesktopError::InvalidInput)?;
        let header = &header[..read];
        let image = header.starts_with(b"\x89PNG\r\n\x1a\n")
            || header.starts_with(b"\xff\xd8\xff")
            || header.starts_with(b"GIF87a")
            || header.starts_with(b"GIF89a")
            || (header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WEBP"));
        if image && identity.size > 20 * 1024 * 1024 {
            return Err(DesktopError::InvalidInput);
        }
        let reference = Self::File {
            name,
            path,
            identity,
            image,
        };
        reference.validate()?;
        Ok(reference)
    }

    pub fn skill(name: String, path: PathBuf) -> Result<Self> {
        safe_name(&name)?;
        safe_path(&path)?;
        Ok(Self::Skill {
            name,
            path,
            identity: None,
        })
    }

    pub fn app(name: String, path: String) -> Result<Self> {
        safe_name(&name)?;
        reference_uri(&path, "app://")?;
        Ok(Self::App { name, path })
    }

    pub fn plugin(name: String, marketplace: &str) -> Result<Self> {
        safe_name(&name)?;
        safe_name(marketplace)?;
        if name.contains('@') || marketplace.contains('@') {
            return Err(DesktopError::InvalidInput);
        }
        let path = format!("plugin://{name}@{marketplace}");
        reference_uri(&path, "plugin://")?;
        Ok(Self::Plugin { name, path })
    }

    /// Catalog references are stamped only after the user selects them. Already
    /// selected files keep their original identity; capture cannot bless a swap.
    pub fn capture(mut self) -> Result<Self> {
        if let Self::Skill { path, identity, .. } = &mut self {
            if identity.is_none() {
                *path = path
                    .canonicalize()
                    .map_err(|_| DesktopError::InvalidInput)?;
                *identity = Some(file_identity(path)?);
            }
        }
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::File {
                name,
                path,
                identity,
                ..
            } => {
                safe_name(name)?;
                verify_file(path, identity)
            }
            Self::Skill {
                name,
                path,
                identity,
            } => {
                safe_name(name)?;
                verify_file(path, identity.as_ref().ok_or(DesktopError::InvalidInput)?)
            }
            Self::App { name, path } => {
                safe_name(name)?;
                reference_uri(path, "app://")
            }
            Self::Plugin { name, path } => {
                safe_name(name)?;
                reference_uri(path, "plugin://")?;
                if path
                    .strip_prefix("plugin://")
                    .and_then(|path| path.split_once('@'))
                    .is_none_or(|(plugin, market)| {
                        plugin.is_empty() || market.is_empty() || market.contains('@')
                    })
                {
                    return Err(DesktopError::InvalidInput);
                }
                Ok(())
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::File { image: true, .. } => "image",
            Self::File { .. } => "file",
            Self::Skill { .. } => "skill",
            Self::App { .. } => "app",
            Self::Plugin { .. } => "plugin",
        }
    }

    pub(super) fn input(&self) -> Option<Value> {
        match self {
            Self::File {
                path, image: true, ..
            } => Some(json!({"type":"localImage","path":path})),
            Self::Skill { name, path, .. } => Some(json!({"type":"skill","name":name,"path":path})),
            Self::App { name, path } | Self::Plugin { name, path } => {
                Some(json!({"type":"mention","name":name,"path":path}))
            }
            Self::File { .. } => None,
        }
    }

    pub(super) fn attachment(&self) -> Option<Value> {
        if let Self::File {
            name, path, image, ..
        } = self
        {
            let mut value = json!({"label":name,"path":path,"fsPath":path});
            if *image {
                value["isImageAttachment"] = json!(true);
            }
            Some(value)
        } else {
            None
        }
    }

    pub(super) fn file_description(&self) -> Option<String> {
        if let Self::File {
            name, path, image, ..
        } = self
        {
            Some(format!(
                "## {name}: {}{}",
                path.to_str()?,
                if *image {
                    "\nImage attachment: true"
                } else {
                    ""
                }
            ))
        } else {
            None
        }
    }
}

impl FileIdentity {
    fn read(metadata: &std::fs::Metadata) -> Result<Self> {
        if !metadata.is_file() {
            return Err(DesktopError::InvalidInput);
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified: metadata.mtime(),
            modified_ns: metadata.mtime_nsec(),
            changed: metadata.ctime(),
            changed_ns: metadata.ctime_nsec(),
        })
    }
}

fn safe_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
        return Err(DesktopError::InvalidInput);
    }
    Ok(())
}
fn reference_uri(path: &str, prefix: &str) -> Result<()> {
    if path.strip_prefix(prefix).is_none_or(str::is_empty)
        || path.len() > 4096
        || path.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(DesktopError::InvalidInput);
    }
    Ok(())
}
fn safe_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|path| path.len() > 8192 || path.chars().any(char::is_control))
    {
        return Err(DesktopError::InvalidInput);
    }
    Ok(())
}
fn file_identity(path: &Path) -> Result<FileIdentity> {
    safe_path(path)?;
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(DesktopError::RequestExpired);
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| DesktopError::RequestExpired)?;
    FileIdentity::read(&metadata)
}
fn verify_file(path: &Path, expected: &FileIdentity) -> Result<()> {
    if &file_identity(path)? != expected {
        return Err(DesktopError::RequestExpired);
    }
    Ok(())
}

#[cfg(test)]
#[path = "references_tests.rs"]
mod tests;
