use super::*;
use std::{collections::HashSet, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ownership {
    Inactive,
    Open,
    Unknown,
}
#[derive(Clone, Debug)]
struct Process {
    pid: u32,
    started_ms: Option<i64>,
    cwd: Option<PathBuf>,
    args: Vec<String>,
    files: HashSet<PathBuf>,
}
pub(super) struct Snapshot {
    processes: Option<Vec<Process>>,
    registrations: Vec<Registration>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Registration {
    pid: u32,
    session_id: String,
    cwd: PathBuf,
    proc_start: String,
    started_at: i64,
    pid_domain: String,
}

fn pid_domain() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        Some("darwin".into())
    }
    #[cfg(target_os = "linux")]
    {
        Some(format!(
            "linux:{}:{}",
            fs::read_to_string("/etc/machine-id").ok()?.trim(),
            fs::read_link("/proc/self/ns/pid").ok()?.to_str()?
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

fn started_ms(value: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(
        &value.split_whitespace().collect::<Vec<_>>().join(" "),
        "%a %b %e %T %Y",
    )
    .ok()
    .map(|d| d.and_utc().timestamp_millis())
}

impl Snapshot {
    pub(super) fn capture(paths: &ClaudePaths) -> Self {
        let root = paths.config.join("sessions");
        let mut registrations = Vec::new();
        let _ = (|| -> Result<()> {
            if !root.try_exists()? {
                return Ok(());
            }
            ensure!(
                !fs::symlink_metadata(&root)?.file_type().is_symlink(),
                "会话登记目录不可核验"
            );
            for (index, entry) in fs::read_dir(&root)?.enumerate() {
                ensure!(index < 20_000, "会话登记超出核验上限");
                let path = entry?.path();
                if path.extension().is_none_or(|s| s != "json") {
                    continue;
                }
                let registration = (|| -> Result<Registration> {
                    let meta = fs::symlink_metadata(&path)?;
                    ensure!(
                        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 262_144,
                        "会话登记无法核验"
                    );
                    let registration: Registration = serde_json::from_slice(&fs::read(&path)?)?;
                    ensure!(
                        registration.pid > 1 && !registration.session_id.is_empty(),
                        "会话登记身份缺失"
                    );
                    Ok(registration)
                })();
                if let Ok(registration) = registration {
                    registrations.push(registration);
                }
            }
            Ok(())
        })();
        Self {
            processes: collect(),
            registrations,
        }
    }
    pub(super) fn ownership(&self, parsed: &Parsed, path: &Path) -> Ownership {
        let Some(processes) = &self.processes else {
            return Ownership::Unknown;
        };
        let mut uncertain = false;
        let domain = pid_domain();
        for registration in &self.registrations {
            if domain.as_deref() != Some(&registration.pid_domain)
                && (registration.session_id == parsed.id
                    || same_path(&registration.cwd, Path::new(&parsed.cwd)))
            {
                uncertain = true;
            }
        }
        for p in processes {
            let registrations: Vec<_> = self
                .registrations
                .iter()
                .filter(|r| r.pid == p.pid && domain.as_deref() == Some(r.pid_domain.as_str()))
                .collect();
            let verified: Vec<_> = registrations
                .iter()
                .filter(|r| {
                    started_ms(&r.proc_start).zip(p.started_ms).is_some_and(
                        |(registered, actual)| registered == actual && r.started_at >= actual,
                    )
                })
                .collect();
            let owns = verified
                .iter()
                .any(|r| r.session_id == parsed.id && same_path(&r.cwd, Path::new(&parsed.cwd)));
            if owns {
                return Ownership::Open;
            }
            if verified.len() == 1 && !p.files.contains(path) {
                continue;
            }
            let cwd_matches = p
                .cwd
                .as_ref()
                .is_some_and(|cwd| same_path(cwd, Path::new(&parsed.cwd)));
            let explicit = p.args.windows(2).any(|pair| {
                matches!(pair[0].as_str(), "--resume" | "-r" | "--session-id")
                    && pair[1] == parsed.id
            }) || p.args.iter().any(|a| {
                a == &format!("--resume={}", parsed.id)
                    || a == &format!("--session-id={}", parsed.id)
            });
            let open_file = p.files.contains(path);
            if open_file {
                return Ownership::Open;
            }
            if cwd_matches && explicit {
                if p.started_ms
                    .zip(parsed.last_turn_at)
                    .is_some_and(|(start, turn)| start <= turn)
                {
                    return Ownership::Open;
                }
                uncertain = true;
            } else if p.cwd.is_none()
                || cwd_matches
                || p.args.iter().any(|a| a == &parsed.id)
                || registrations.iter().any(|r| r.session_id == parsed.id)
            {
                uncertain = true;
            }
        }
        // A malformed registry must not poison targets whose cwd is known to
        // have no possible native process, or a process verified to another ID.
        if uncertain {
            Ownership::Unknown
        } else {
            Ownership::Inactive
        }
    }
}
fn same_path(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}
fn native(command: &str, args: &[String]) -> bool {
    let name = Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    matches!(name, "claude" | "claude.exe")
        || (matches!(name, "node" | "bun")
            && args.iter().any(|a| {
                a.contains("/@anthropic-ai/claude-code/")
                    && (a.ends_with("cli.js") || a.ends_with("claude.exe"))
            }))
}
fn collect() -> Option<Vec<Process>> {
    // macOS truncates `comm` paths to 16 characters even with -ww. `ucomm`
    // comes from the native process name, so long CLI installation paths do
    // not hide a live Claude process or rely on a caller-controlled argv[0].
    #[cfg(target_os = "macos")]
    let format = "pid=,stat=,lstart=,ucomm=,args=";
    #[cfg(not(target_os = "macos"))]
    let format = "pid=,stat=,lstart=,comm=,args=";
    let output = Command::new("ps")
        .args(["-ww", "-axo", format])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut processes = Vec::new();
    for line in String::from_utf8(output.stdout).ok()?.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 9 || fields[1].contains('Z') {
            continue;
        }
        let args = fields[8..]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        if !native(fields[7], &args) {
            continue;
        }
        let pid = fields[0].parse().ok()?;
        let started =
            chrono::NaiveDateTime::parse_from_str(&fields[2..7].join(" "), "%a %b %e %T %Y")
                .ok()
                .map(|d| d.and_utc().timestamp_millis());
        processes.push(Process {
            pid,
            started_ms: started,
            cwd: None,
            args,
            files: HashSet::new(),
        });
    }
    if processes.is_empty() {
        return Some(processes);
    }
    #[cfg(target_os = "linux")]
    for p in &mut processes {
        let root = PathBuf::from(format!("/proc/{}", p.pid));
        p.cwd = fs::read_link(root.join("cwd")).ok();
        if let Ok(files) = fs::read_dir(root.join("fd")) {
            p.files = files
                .filter_map(|e| fs::read_link(e.ok()?.path()).ok())
                .collect();
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let pids = processes
            .iter()
            .map(|p| p.pid.to_string())
            .collect::<Vec<_>>()
            .join(",");
        if let Ok(output) = Command::new("/usr/sbin/lsof")
            .args(["-nP", "-p", &pids, "-Fpnf"])
            .output()
        {
            let mut current = None;
            let mut descriptor = "";
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some(pid) = line.strip_prefix('p').and_then(|s| s.parse::<u32>().ok()) {
                    current = processes.iter().position(|p| p.pid == pid);
                } else if let Some(fd) = line.strip_prefix('f') {
                    descriptor = fd;
                } else if let (Some(index), Some(name)) = (current, line.strip_prefix('n')) {
                    if descriptor == "cwd" {
                        processes[index].cwd = Some(PathBuf::from(name));
                    } else if name.ends_with(".jsonl") {
                        processes[index].files.insert(PathBuf::from(name));
                    }
                }
            }
        }
    }
    Some(processes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verified_registry_scopes_same_cwd_and_survives_unavailable_lsof() {
        let parsed = Parsed {
            id: "historical".into(),
            cwd: "/fixture/project".into(),
            ..Default::default()
        };
        let date = "Tue Oct 20 08:00:00 2026";
        let process = Process {
            pid: 42,
            started_ms: started_ms(date),
            cwd: None,
            args: vec!["claude".into()],
            files: HashSet::new(),
        };
        let registration = Registration {
            pid: 42,
            session_id: "current".into(),
            cwd: parsed.cwd.clone().into(),
            proc_start: date.into(),
            started_at: started_ms(date).unwrap() + 500,
            pid_domain: pid_domain().unwrap(),
        };
        let snapshot = Snapshot {
            processes: Some(vec![process.clone()]),
            registrations: vec![registration.clone()],
        };
        assert_eq!(
            snapshot.ownership(&parsed, Path::new("/fixture/history.jsonl")),
            Ownership::Inactive
        );
        assert_eq!(
            snapshot.ownership(
                &Parsed {
                    id: "current".into(),
                    ..parsed.clone()
                },
                Path::new("/fixture/current.jsonl")
            ),
            Ownership::Open
        );
        for registration in [
            Registration {
                proc_start: "Mon Oct 19 08:00:00 2026".into(),
                ..registration.clone()
            },
            Registration {
                pid_domain: "foreign-domain".into(),
                ..registration
            },
        ] {
            let snapshot = Snapshot {
                processes: Some(vec![process.clone()]),
                registrations: vec![registration],
            };
            assert_eq!(
                snapshot.ownership(&parsed, Path::new("/fixture/history.jsonl")),
                Ownership::Unknown
            );
        }
    }

    #[test]
    fn corrupt_registry_only_blocks_targets_a_live_unmapped_process_could_own() {
        let parsed = Parsed {
            id: "fixture".into(),
            cwd: "/fixture/project".into(),
            ..Default::default()
        };
        let process = Process {
            pid: 42,
            started_ms: Some(1000),
            cwd: Some("/fixture/elsewhere".into()),
            args: vec!["claude".into()],
            files: HashSet::new(),
        };
        let mut snapshot = Snapshot {
            processes: Some(vec![process]),
            registrations: vec![],
        };
        assert_eq!(
            snapshot.ownership(&parsed, Path::new("/fixture/history.jsonl")),
            Ownership::Inactive
        );
        snapshot.processes.as_mut().unwrap()[0].cwd = Some(parsed.cwd.clone().into());
        assert_eq!(
            snapshot.ownership(&parsed, Path::new("/fixture/history.jsonl")),
            Ownership::Unknown
        );
    }
    #[test]
    fn inactive_reused_idle_and_unknown_ownership_do_not_prove_running() {
        let p = Parsed {
            id: "native-id".into(),
            cwd: "/tmp/example".into(),
            last_turn_at: Some(2000),
            ..Default::default()
        };
        let file = Path::new("/tmp/example.jsonl");
        assert_eq!(
            Snapshot {
                registrations: vec![],
                processes: Some(vec![])
            }
            .ownership(&p, file),
            Ownership::Inactive
        );
        let owner = Process {
            pid: 5,
            started_ms: Some(1000),
            cwd: Some(PathBuf::from(&p.cwd)),
            args: vec!["claude".into(), "--resume".into(), p.id.clone()],
            files: HashSet::new(),
        };
        assert_eq!(
            Snapshot {
                registrations: vec![],
                processes: Some(vec![owner.clone()])
            }
            .ownership(&p, file),
            Ownership::Open
        );
        assert_eq!(
            Snapshot {
                registrations: vec![],
                processes: Some(vec![Process {
                    started_ms: Some(3000),
                    ..owner.clone()
                }])
            }
            .ownership(&p, file),
            Ownership::Unknown
        );
        assert_eq!(
            Snapshot {
                registrations: vec![],
                processes: Some(vec![Process {
                    args: vec!["claude".into()],
                    ..owner
                }])
            }
            .ownership(&p, file),
            Ownership::Unknown
        );
        assert_eq!(
            Snapshot {
                registrations: vec![],
                processes: None
            }
            .ownership(&p, file),
            Ownership::Unknown
        );
        assert!(!native("zsh", &["claude".into()]));
    }
}
