//! Explicit local settings management. Never runs as part of App installation.
use super::*;
use serde_json::json;
use std::io::Write;
use std::sync::Mutex;

static SETTINGS: Mutex<()> = Mutex::new(());
pub const HANDLER: &str = "claude-hook";
const EVENTS: [(&str, &str); 3] = [
    ("Stop", ""),
    (
        "Notification",
        "permission_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input|idle_prompt",
    ),
    ("StopFailure", ""),
];

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeHooksAction {
    Status,
    Install,
    Remove,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeHooksRequest {
    pub action: ClaudeHooksAction,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHooksStatus {
    pub installed: bool,
    pub managed_count: usize,
    pub changed: bool,
}

fn managed(value: &Value, binary: &Path) -> bool {
    value["type"] == "command"
        && value["command"].as_str() == binary.to_str()
        && value["args"].as_array().is_some_and(|args| {
            args.len() == 3 && args[0] == "--config" && args[1].is_string() && args[2] == HANDLER
        })
}
fn expected(binary: &Path, config: &Path) -> Value {
    json!({"type":"command", "command":binary, "args":["--config", config, HANDLER], "async":true})
}
fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(metadata) => {
            ensure!(
                metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.len() <= 2 * 1024 * 1024,
                "Claude 设置文件无法安全核验"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ensure!(metadata.nlink() == 1, "Claude 设置文件存在硬链接");
            }
            Ok(Some(fs::read(path)?))
        }
    }
}
fn checked(bytes: Option<&[u8]>) -> Result<Value> {
    let root: Value = bytes
        .map(serde_json::from_slice)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    ensure!(root.is_object(), "Claude 设置必须为 JSON 对象，未作修改");
    if let Some(hooks) = root.get("hooks") {
        let hooks = hooks
            .as_object()
            .context("Claude hooks 格式错误，未作修改")?;
        for groups in hooks.values() {
            for group in groups
                .as_array()
                .context("Claude hook 分组格式错误，未作修改")?
            {
                ensure!(group.is_object(), "Claude hook 分组格式错误，未作修改");
                let handlers = group["hooks"]
                    .as_array()
                    .context("Claude hook 列表格式错误，未作修改")?;
                ensure!(
                    handlers.iter().all(Value::is_object),
                    "Claude hook 条目格式错误，未作修改"
                );
            }
        }
    }
    Ok(root)
}
fn status(root: &Value, binary: &Path, config: &Path) -> ClaudeHooksStatus {
    let count = root["hooks"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
        .flat_map(|g| g["hooks"].as_array().into_iter().flatten())
        .filter(|h| managed(h, binary))
        .count();
    let installed = count == EVENTS.len()
        && EVENTS.iter().all(|(event, matcher)| {
            root["hooks"][event].as_array().is_some_and(|groups| {
                groups.iter().any(|g| {
                    g["matcher"] == *matcher
                        && g["hooks"]
                            .as_array()
                            .is_some_and(|hs| hs.contains(&expected(binary, config)))
                })
            })
        });
    ClaudeHooksStatus {
        installed,
        managed_count: count,
        changed: false,
    }
}
pub fn installed(paths: &ClaudePaths, binary: &Path, config: &Path) -> bool {
    read(&paths.config.join("settings.json"))
        .and_then(|bytes| checked(bytes.as_deref()))
        .is_ok_and(|root| status(&root, binary, config).installed)
}

/// The monitor runs the same fixed executable as the hook, including when a
/// user selected a non-default NexusHub config file.
pub fn configured(paths: &ClaudePaths, binary: &Path) -> bool {
    read(&paths.config.join("settings.json"))
        .and_then(|bytes| checked(bytes.as_deref()))
        .is_ok_and(|root| {
            let config = root["hooks"]["Stop"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|g| g["hooks"].as_array().into_iter().flatten())
                .find(|h| managed(h, binary))
                .and_then(|h| h["args"][1].as_str());
            config.is_some_and(|config| status(&root, binary, Path::new(config)).installed)
        })
}
pub fn manage(
    paths: &ClaudePaths,
    binary: &Path,
    config: &Path,
    request: ClaudeHooksRequest,
) -> Result<ClaudeHooksStatus> {
    let _lock = SETTINGS
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude 设置管理锁不可用"))?;
    let path = paths.config.join("settings.json");
    let before = read(&path)?;
    let before_stamp = before
        .as_ref()
        .map(|_| super::reader::stamp(&path))
        .transpose()?;
    let mut root = checked(before.as_deref())?;
    let prior = status(&root, binary, config);
    if request.action == ClaudeHooksAction::Status {
        return Ok(prior);
    }
    ensure!(request.confirmed, "Claude hooks 安装或移除需要明确确认");
    if (request.action == ClaudeHooksAction::Install && prior.installed)
        || (request.action == ClaudeHooksAction::Remove && prior.managed_count == 0)
    {
        return Ok(prior);
    }
    if request.action == ClaudeHooksAction::Install {
        ensure!(
            binary.is_absolute() && binary.is_file(),
            "本机通知程序不可用，请修复 App 安装"
        );
    }
    let hooks = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();
    for groups in hooks.values_mut() {
        let groups = groups.as_array_mut().unwrap();
        groups.retain_mut(|group| {
            let handlers = group["hooks"].as_array_mut().unwrap();
            let before = handlers.len();
            handlers.retain(|h| !managed(h, binary));
            // Preserve pre-existing empty groups and any user/plugin handlers.
            before == handlers.len() || !handlers.is_empty()
        });
    }
    if request.action == ClaudeHooksAction::Install {
        for (event, matcher) in EVENTS {
            hooks
                .entry(event)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap()
                .push(json!({"matcher":matcher,"hooks":[expected(binary,config)]}));
        }
    }
    let bytes = serde_json::to_vec_pretty(&root)?;
    fs::create_dir_all(&paths.config)?;
    let temporary = paths
        .config
        .join(format!(".nexushub-hooks-{}.tmp", uuid::Uuid::new_v4()));
    let write = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        #[cfg(test)]
        SETTINGS_CHECKPOINT.with(|slot| {
            if let Some(hook) = slot.borrow_mut().as_mut() {
                hook(&path);
            }
        });
        ensure!(
            read(&path)? == before
                && before_stamp
                    == if path.exists() {
                        Some(super::reader::stamp(&path)?)
                    } else {
                        None
                    },
            "Claude 设置已被其他程序修改，请刷新后重试"
        );
        fs::rename(&temporary, &path)?;
        ensure!(
            read(&path)?.as_deref() == Some(bytes.as_slice()),
            "Claude 设置读回发生变化，请刷新后检查"
        );
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    write?;
    Ok(ClaudeHooksStatus {
        changed: true,
        ..status(&root, binary, config)
    })
}

#[cfg(test)]
type SettingsHook = Box<dyn FnMut(&Path)>;
#[cfg(test)]
thread_local! { static SETTINGS_CHECKPOINT: std::cell::RefCell<Option<SettingsHook>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merge_repair_remove_preserve_user_hooks_and_reject_conflicts() {
        let directory = std::env::temp_dir().join(format!("claude-hooks-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let paths = ClaudePaths {
            config: directory.as_path().into(),
            projects: directory.as_path().join("projects"),
        };
        let binary = directory.as_path().join("nexushub-webd");
        fs::write(&binary, "fixture").unwrap();
        let config = directory.as_path().join("fixture.toml");
        let path = directory.as_path().join("settings.json");
        let original = json!({"theme":"dark","hooks":{"Stop":[{"hooks":[{"type":"command","command":"user-fixture","args":[]}]}]}});
        fs::write(&path, original.to_string()).unwrap();
        let run = |action| {
            manage(
                &paths,
                &binary,
                &config,
                ClaudeHooksRequest {
                    action,
                    confirmed: true,
                },
            )
        };
        assert!(run(ClaudeHooksAction::Install).unwrap().installed);
        assert!(!run(ClaudeHooksAction::Install).unwrap().changed);
        let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(installed["theme"], "dark");
        assert_eq!(installed["hooks"]["Stop"][0], original["hooks"]["Stop"][0]);
        assert_eq!(
            installed["hooks"]["Stop"][1]["hooks"][0]["args"][2],
            HANDLER
        );
        assert!(installed["hooks"]["Stop"][1]["hooks"][0]
            .get("timeout")
            .is_none());
        assert!(!run(ClaudeHooksAction::Remove).unwrap().installed);
        assert!(!run(ClaudeHooksAction::Remove).unwrap().changed);
        SETTINGS_CHECKPOINT.with(|s| {
            *s.borrow_mut() = Some(Box::new(|path| {
                fs::write(path, "{\"concurrent\":true}").unwrap();
            }))
        });
        assert!(run(ClaudeHooksAction::Install).is_err());
        SETTINGS_CHECKPOINT.with(|s| *s.borrow_mut() = None);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"concurrent\":true}");
        for broken in ["{broken", "[]", "{\"hooks\":{\"Stop\":false}}"] {
            fs::write(&path, broken).unwrap();
            assert!(run(ClaudeHooksAction::Install).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), broken);
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
