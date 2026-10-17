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

/// Encrypted queue metadata uses a resolved reference index, not an expiring
/// UI grant. Ranges remain relative to the user's text until wire construction.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceSpan {
    pub reference: usize,
    pub start: usize,
    pub end: usize,
}

pub(super) fn resolve_spans(
    handles: &[String],
    spans: &[crate::services::codex_desktop::DesktopReferenceSpan],
) -> Result<Vec<ReferenceSpan>> {
    if spans.len() > 32 {
        return Err(DesktopError::InvalidInput);
    }
    spans
        .iter()
        .map(|span| {
            Ok(ReferenceSpan {
                reference: handles
                    .iter()
                    .position(|handle| handle == &span.handle)
                    .ok_or(DesktopError::InvalidInput)?,
                start: span.start,
                end: span.end,
            })
        })
        .collect()
}

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
        Self::selected_file_detailed(path).map_err(|(error, _)| error)
    }

    pub fn selected_file_detailed(
        path: PathBuf,
    ) -> std::result::Result<Self, (DesktopError, &'static str)> {
        let path = path
            .canonicalize()
            .map_err(|_| (DesktopError::InvalidInput, "无法读取所选文件，请重新选择"))?;
        safe_path(&path).map_err(|error| (error, "文件名或路径格式无效"))?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or((DesktopError::InvalidInput, "文件名或路径格式无效"))?
            .to_owned();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| (DesktopError::InvalidInput, "无法读取所选文件，请重新选择"))?;
        let identity = FileIdentity::read(
            &file
                .metadata()
                .map_err(|_| (DesktopError::InvalidInput, "无法读取所选文件，请重新选择"))?,
        )
        .map_err(|error| (error, "仅支持普通文件，不能添加目录"))?;
        let mut header = [0_u8; 16];
        let read = file
            .read(&mut header)
            .map_err(|_| (DesktopError::InvalidInput, "无法读取所选文件，请重新选择"))?;
        let header = &header[..read];
        let image = header.starts_with(b"\x89PNG\r\n\x1a\n")
            || header.starts_with(b"\xff\xd8\xff")
            || header.starts_with(b"GIF87a")
            || header.starts_with(b"GIF89a")
            || (header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WEBP"));
        if image && identity.size > 20 * 1024 * 1024 {
            return Err((
                DesktopError::InvalidInput,
                "图片超过 20MiB，请选择较小的图片",
            ));
        }
        let reference = Self::File {
            name,
            path,
            identity,
            image,
        };
        reference
            .validate()
            .map_err(|error| (error, "所选文件已变化，请重新选择"))?;
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

    pub(super) fn placeholder(&self) -> Option<String> {
        match self {
            Self::Skill { name, .. } => Some(format!("${name}")),
            Self::App { name, .. } | Self::Plugin { name, .. } => Some(format!("@{name}")),
            Self::File { .. } => None,
        }
    }

    /// Build 20586's composer dG/uG serializes resolved mentions as Markdown
    /// links. Only catalog-authorized paths are used; no UI path is accepted.
    pub(super) fn prompt_link(&self) -> Option<String> {
        let (label, path) = match self {
            Self::Skill { name, path, .. } => (format!("${name}"), path.to_str()?),
            Self::App { name, path } => (format!("${name}"), path.as_str()),
            Self::Plugin { name, path } => (format!("@{name}"), path.as_str()),
            Self::File { .. } => return None,
        };
        // Match the native prompt-link serializer, including its label boundary
        // escape. Paths retain spaces and opening parentheses as native identity;
        // the native parser unescapes backslashes and the closing delimiter.
        let label = label
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace("](", "]\\(")
            .replace(']', "\\]");
        let destination = path.replace('\\', "\\\\").replace(')', "\\)");
        Some(format!("[{label}]({destination})"))
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
