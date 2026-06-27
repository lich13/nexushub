use std::path::Path;

const NEXUSHUB_WEBD_RESOURCE_NAME: &str = "nexushub-webd";
const NEXUSHUB_WEBD_HELPER_PLACEHOLDER: &[u8] = b"NEXUSHUB_HELPER_PLACEHOLDER";
const WEBUI_RESOURCE_NAME: &str = "webui";

pub(crate) fn sync_nexushub_webd_helper_from_resource(resource_dir: &Path) -> Result<(), String> {
    let source = resource_dir.join(NEXUSHUB_WEBD_RESOURCE_NAME);
    if !source.is_file() {
        return Ok(());
    }
    if is_nexushub_webd_helper_placeholder(&source).map_err(|err| err.to_string())? {
        return Ok(());
    }
    let platform = nexushub_core::platform::PlatformPaths::desktop_current();
    let target = platform.daemon_binary();
    sync_nexushub_webd_helper_file(&source, &target).map_err(|err| err.to_string())
}

pub(crate) fn prepare_desktop_webui_assets_from_resource(
    resource_dir: &Path,
) -> Result<(), String> {
    let source = resource_dir.join(WEBUI_RESOURCE_NAME);
    if !source.join("index.html").is_file() {
        return Ok(());
    }

    let platform = nexushub_core::platform::PlatformPaths::desktop_current();
    sync_directory(&source, &platform.webui_dir).map_err(|err| err.to_string())?;
    remove_legacy_webui_dir(&platform).map_err(|err| err.to_string())?;
    migrate_desktop_webui_dir_config(&platform).map_err(|err| err.to_string())
}

fn remove_legacy_webui_dir(
    platform: &nexushub_core::platform::PlatformPaths,
) -> std::io::Result<()> {
    let legacy = platform.data_dir.join("webui");
    if legacy != platform.webui_dir && legacy.is_dir() {
        std::fs::remove_dir_all(legacy)?;
    }
    Ok(())
}

fn migrate_desktop_webui_dir_config(
    platform: &nexushub_core::platform::PlatformPaths,
) -> anyhow::Result<()> {
    let config_path = &platform.config_file;
    if !config_path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(config_path)?;
    let mut value = text.parse::<toml::Value>()?;
    let Some(paths) = value.get_mut("paths").and_then(toml::Value::as_table_mut) else {
        return Ok(());
    };
    let data_dir = paths
        .get("data_dir")
        .and_then(toml::Value::as_str)
        .map(Path::new);
    if data_dir != Some(platform.data_dir.as_path()) {
        return Ok(());
    }
    let webui_dir = paths
        .get("webui_dir")
        .and_then(toml::Value::as_str)
        .map(Path::new);
    if webui_dir == Some(platform.webui_dir.as_path()) {
        return Ok(());
    }
    paths.insert(
        "webui_dir".to_string(),
        toml::Value::String(platform.webui_dir.display().to_string()),
    );
    std::fs::write(config_path, toml::to_string_pretty(&value)?)?;
    Ok(())
}

fn is_nexushub_webd_helper_placeholder(path: &Path) -> std::io::Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(bytes.starts_with(NEXUSHUB_WEBD_HELPER_PLACEHOLDER))
}

fn sync_nexushub_webd_helper_file(source: &Path, target: &Path) -> std::io::Result<()> {
    let should_copy = match (std::fs::metadata(source), std::fs::metadata(target)) {
        (Ok(source_meta), Ok(target_meta)) => {
            source_meta.len() != target_meta.len()
                || source_meta.modified().ok() != target_meta.modified().ok()
        }
        (Ok(_), Err(_)) => true,
        (Err(err), _) => return Err(err),
    };
    if !should_copy {
        ensure_executable(target)?;
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, target)?;
    ensure_executable(target)
}

fn sync_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    if target.exists() {
        std::fs::remove_dir_all(target)?;
    }
    copy_directory_recursive(source, target)
}

fn copy_directory_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_directory_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_nexushub_webd_helper_file_copies_and_marks_executable() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("nexushub-webd");
        let target = temp
            .path()
            .join("Application Support/NexusHub/bin/nexushub-webd");
        std::fs::write(&source, b"#!/bin/sh\nexit 0\n").unwrap();

        sync_nexushub_webd_helper_file(&source, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"#!/bin/sh\nexit 0\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "helper must be executable");
        }
    }

    #[test]
    fn helper_placeholder_detection_prevents_dev_resource_sync() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("nexushub-webd");
        std::fs::write(&source, b"NEXUSHUB_HELPER_PLACEHOLDER\nnot a binary\n").unwrap();

        assert!(is_nexushub_webd_helper_placeholder(&source).unwrap());
    }

    #[test]
    fn sync_directory_replaces_stale_webui_assets() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("resource-webui");
        let target = temp.path().join("desktop-assets");
        std::fs::create_dir_all(source.join("assets")).unwrap();
        std::fs::create_dir_all(target.join("assets")).unwrap();
        std::fs::write(
            source.join("index.html"),
            "<script src=\"/assets/new.js\"></script>",
        )
        .unwrap();
        std::fs::write(source.join("assets/new.js"), "new").unwrap();
        std::fs::write(target.join("assets/old.js"), "old").unwrap();

        sync_directory(&source, &target).unwrap();

        assert_eq!(
            std::fs::read_to_string(target.join("index.html")).unwrap(),
            "<script src=\"/assets/new.js\"></script>"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("assets/new.js")).unwrap(),
            "new"
        );
        assert!(!target.join("assets/old.js").exists());
    }

    #[test]
    fn migrate_desktop_webui_dir_config_moves_legacy_webui_path() {
        let temp = tempfile::tempdir().unwrap();
        let platform = nexushub_core::platform::PlatformPaths::for_kind_with_home(
            nexushub_core::platform::PlatformKind::Macos,
            temp.path(),
        );
        std::fs::create_dir_all(&platform.data_dir).unwrap();
        let legacy_webui = platform.data_dir.join("webui");
        let config = format!(
            r#"
[paths]
data_dir = "{}"
db_path = "{}"
webui_dir = "{}"
log_dir = "{}"
"#,
            platform.data_dir.display(),
            platform.data_dir.join("nexushub.sqlite").display(),
            legacy_webui.display(),
            platform.log_dir.display()
        );
        std::fs::write(&platform.config_file, config).unwrap();

        migrate_desktop_webui_dir_config(&platform).unwrap();

        let migrated = std::fs::read_to_string(&platform.config_file).unwrap();
        assert!(migrated.contains(&format!("webui_dir = \"{}\"", platform.webui_dir.display())));
        assert!(!migrated.contains(&format!("webui_dir = \"{}\"", legacy_webui.display())));
    }
}
