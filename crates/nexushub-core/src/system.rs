use crate::{codex::CodexPaths, config::Config};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub host_label: String,
    pub hostname: Option<String>,
    pub public_endpoint: Option<String>,
    pub codex_home: String,
    pub state_db: String,
    pub panel_db: String,
    pub app_server_socket: Option<String>,
    pub app_server_service: ServiceStatus,
    pub state_db_integrity: Option<String>,
    pub hidden_thread_count: usize,
    pub thread_source_counts: HashMap<String, usize>,
    pub app_server_source_counts: HashMap<String, usize>,
    pub app_server_hidden_thread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub active: bool,
    pub active_state: Option<String>,
    pub sub_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub panel_current: String,
    pub panel_latest: Option<String>,
    pub panel_update_available: Option<bool>,
    pub codex_current: Option<String>,
    pub codex_latest: Option<String>,
    pub codex_update_available: Option<bool>,
    pub codex_user: Option<String>,
    pub codex_root: Option<String>,
    pub codex_raw: Option<String>,
}

pub async fn system_status(config: &Config) -> Result<SystemStatus> {
    let service = service_status(&config.codex.app_server_service)
        .await
        .unwrap_or(ServiceStatus {
            active: false,
            active_state: None,
            sub_state: None,
        });
    let paths = CodexPaths::new(&config.codex.home);
    let state_db_integrity = crate::codex::db_integrity(&paths).ok();
    let hidden_thread_count = crate::codex::hidden_thread_ids(&paths)
        .map(|ids| ids.len())
        .unwrap_or(0);
    let thread_source_counts = crate::codex::thread_source_counts(&paths).unwrap_or_default();
    Ok(SystemStatus {
        host_label: config.codex.host_label.clone(),
        hostname: command_stdout("hostname", &[]).await.ok(),
        public_endpoint: config.server.public_base_url.clone(),
        codex_home: config.codex.home.display().to_string(),
        state_db: paths.state_db().display().to_string(),
        panel_db: config.paths.db_path.display().to_string(),
        app_server_socket: config
            .codex
            .app_server_socket
            .as_ref()
            .map(|path| path.display().to_string()),
        app_server_service: service,
        state_db_integrity,
        hidden_thread_count,
        thread_source_counts,
        app_server_source_counts: HashMap::new(),
        app_server_hidden_thread_count: 0,
    })
}

pub async fn version_info() -> Result<VersionInfo> {
    let latest = github_latest_release("lich13", "nexushub").await.ok();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let update_available = latest
        .as_ref()
        .map(|latest| latest.trim_start_matches('v') != current);
    let codex_raw = command_stdout_timeout(
        "/usr/local/bin/codex-raw",
        &["--version"],
        Duration::from_secs(3),
    )
    .await
    .ok();
    let codex_root = command_stdout_timeout(
        "sudo",
        &["-n", "codex", "--version"],
        Duration::from_secs(3),
    )
    .await
    .ok();
    let codex_user = command_stdout_timeout("codex", &["--version"], Duration::from_secs(3))
        .await
        .ok();
    let codex_current = current_codex_version(
        codex_raw.as_deref(),
        codex_root.as_deref(),
        codex_user.as_deref(),
    );
    let codex_latest = npm_latest_version("@openai/codex").await.ok();
    let codex_update_available =
        codex_update_available(codex_current.as_deref(), codex_latest.as_deref());
    Ok(VersionInfo {
        panel_current: current,
        panel_latest: latest,
        panel_update_available: update_available,
        codex_current,
        codex_latest,
        codex_update_available,
        codex_user,
        codex_root,
        codex_raw,
    })
}

async fn github_latest_release(owner: &str, repo: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
    }
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let release: Release = reqwest::Client::new()
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(release.tag_name)
}

async fn npm_latest_version(package: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct DistTags {
        latest: String,
    }
    #[derive(Deserialize)]
    struct PackageInfo {
        #[serde(rename = "dist-tags")]
        dist_tags: DistTags,
    }
    let encoded = package.replace('/', "%2F");
    let url = format!("https://registry.npmjs.org/{encoded}");
    let package: PackageInfo = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(package.dist_tags.latest)
}

fn current_codex_version(
    raw: Option<&str>,
    root: Option<&str>,
    user: Option<&str>,
) -> Option<String> {
    [raw, root, user]
        .into_iter()
        .flatten()
        .find_map(extract_semver)
}

fn codex_update_available(current: Option<&str>, latest: Option<&str>) -> Option<bool> {
    Some(
        compare_semver(
            extract_semver(latest?)?.as_str(),
            extract_semver(current?)?.as_str(),
        )?
        .is_gt(),
    )
}

fn extract_semver(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    for start in 0..bytes.len() {
        let first = bytes[start] as char;
        if !first.is_ascii_digit() && first != 'v' {
            continue;
        }
        let mut index = if first == 'v' { start + 1 } else { start };
        if index >= bytes.len() || !(bytes[index] as char).is_ascii_digit() {
            continue;
        }
        let mut parts = Vec::new();
        for _ in 0..3 {
            let part_start = index;
            while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                index += 1;
            }
            if part_start == index {
                parts.clear();
                break;
            }
            parts.push(&value[part_start..index]);
            if parts.len() < 3 {
                if index >= bytes.len() || bytes[index] != b'.' {
                    parts.clear();
                    break;
                }
                index += 1;
            }
        }
        if parts.len() != 3 {
            continue;
        }
        let mut end = index;
        if end < bytes.len() && bytes[end] == b'-' {
            end += 1;
            let pre_start = end;
            while end < bytes.len() {
                let ch = bytes[end] as char;
                if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                    end += 1;
                } else {
                    break;
                }
            }
            if pre_start == end {
                end = index;
            }
        }
        return Some(value[start..end].trim_start_matches('v').to_string());
    }
    None
}

fn compare_semver(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = ParsedVersion::parse(left)?;
    let right = ParsedVersion::parse(right)?;
    Some(left.cmp(&right))
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedVersion {
    core: [u64; 3],
    pre: Vec<VersionIdentifier>,
}

impl ParsedVersion {
    fn parse(value: &str) -> Option<Self> {
        let (core_text, pre_text) = value
            .trim()
            .trim_start_matches('v')
            .split_once('-')
            .unwrap_or((value.trim().trim_start_matches('v'), ""));
        let mut core = [0_u64; 3];
        let parts = core_text.split('.').collect::<Vec<_>>();
        if parts.len() != 3 {
            return None;
        }
        for (index, part) in parts.iter().enumerate() {
            core[index] = part.parse().ok()?;
        }
        let pre = if pre_text.is_empty() {
            Vec::new()
        } else {
            pre_text
                .split('.')
                .map(VersionIdentifier::parse)
                .collect::<Option<Vec<_>>>()?
        };
        Some(Self { core, pre })
    }
}

impl Ord for ParsedVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.core.cmp(&other.core) {
            std::cmp::Ordering::Equal => compare_pre(&self.pre, &other.pre),
            ordering => ordering,
        }
    }
}

impl PartialOrd for ParsedVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Eq, PartialEq)]
enum VersionIdentifier {
    Numeric(u64),
    Text(String),
}

impl VersionIdentifier {
    fn parse(value: &str) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        if value.chars().all(|ch| ch.is_ascii_digit()) {
            Some(Self::Numeric(value.parse().ok()?))
        } else if value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            Some(Self::Text(value.to_string()))
        } else {
            None
        }
    }
}

fn compare_pre(left: &[VersionIdentifier], right: &[VersionIdentifier]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if left.is_empty() && right.is_empty() {
        return Ordering::Equal;
    }
    if left.is_empty() {
        return Ordering::Greater;
    }
    if right.is_empty() {
        return Ordering::Less;
    }
    for (left, right) in left.iter().zip(right) {
        let ordering = match (left, right) {
            (VersionIdentifier::Numeric(left), VersionIdentifier::Numeric(right)) => {
                left.cmp(right)
            }
            (VersionIdentifier::Numeric(_), VersionIdentifier::Text(_)) => Ordering::Less,
            (VersionIdentifier::Text(_), VersionIdentifier::Numeric(_)) => Ordering::Greater,
            (VersionIdentifier::Text(left), VersionIdentifier::Text(right)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

async fn service_status(service: &str) -> Result<ServiceStatus> {
    let output = Command::new("systemctl")
        .args([
            "show",
            service,
            "-p",
            "ActiveState",
            "-p",
            "SubState",
            "--no-pager",
        ])
        .output()
        .await
        .context("systemctl show")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let active_state = parse_systemctl_property(&text, "ActiveState");
    let sub_state = parse_systemctl_property(&text, "SubState");
    Ok(ServiceStatus {
        active: active_state.as_deref() == Some("active"),
        active_state,
        sub_state,
    })
}

async fn command_stdout(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output().await?;
    if !output.status.success() {
        anyhow::bail!("{program} exited with {}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn command_stdout_timeout(
    program: &str,
    args: &[&str],
    duration: Duration,
) -> Result<String> {
    tokio::time::timeout(duration, command_stdout(program, args))
        .await
        .with_context(|| format!("{program} timed out"))?
}

fn parse_systemctl_property(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::{
        codex_update_available, compare_semver, current_codex_version, extract_semver,
        parse_systemctl_property,
    };

    #[test]
    fn parses_property() {
        assert_eq!(
            parse_systemctl_property("ActiveState=active\n", "ActiveState").as_deref(),
            Some("active")
        );
    }

    #[test]
    fn extracts_codex_cli_semver() {
        assert_eq!(
            extract_semver("codex-cli 0.137.0").as_deref(),
            Some("0.137.0")
        );
        assert_eq!(
            extract_semver("v0.137.0-beta.1").as_deref(),
            Some("0.137.0-beta.1")
        );
        assert_eq!(extract_semver("unknown"), None);
    }

    #[test]
    fn current_codex_version_prefers_raw_root_user_order() {
        assert_eq!(
            current_codex_version(
                Some("codex-cli 0.137.0"),
                Some("codex-cli 0.136.0"),
                Some("codex-cli 0.135.0")
            )
            .as_deref(),
            Some("0.137.0")
        );
        assert_eq!(
            current_codex_version(None, Some("codex-cli 0.136.0"), None).as_deref(),
            Some("0.136.0")
        );
    }

    #[test]
    fn compares_semver_with_prerelease_rules() {
        assert!(compare_semver("0.138.0", "0.137.0").is_some_and(|ordering| ordering.is_gt()));
        assert!(compare_semver("0.137.0", "0.137.0").is_some_and(|ordering| ordering.is_eq()));
        assert!(compare_semver("0.137.0-beta.2", "0.137.0-beta.11")
            .is_some_and(|ordering| ordering.is_lt()));
        assert!(
            compare_semver("0.137.0-beta.1", "0.137.0").is_some_and(|ordering| ordering.is_lt())
        );
        assert_eq!(compare_semver("unknown", "0.137.0"), None);
    }

    #[test]
    fn codex_update_available_is_three_state() {
        assert_eq!(
            codex_update_available(Some("0.137.0"), Some("0.138.0")),
            Some(true)
        );
        assert_eq!(
            codex_update_available(Some("0.137.0"), Some("0.137.0")),
            Some(false)
        );
        assert_eq!(
            codex_update_available(Some("0.138.0"), Some("0.137.0")),
            Some(false)
        );
        assert_eq!(
            codex_update_available(Some("unknown"), Some("0.137.0")),
            None
        );
        assert_eq!(codex_update_available(Some("0.137.0"), None), None);
    }
}
