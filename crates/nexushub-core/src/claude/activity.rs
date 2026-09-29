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
}

impl Snapshot {
    pub(super) fn capture() -> Self {
        Self {
            processes: collect(),
        }
    }
    pub(super) fn ownership(&self, parsed: &Parsed, path: &Path) -> Ownership {
        let Some(processes) = &self.processes else {
            return Ownership::Unknown;
        };
        let mut uncertain = false;
        for p in processes {
            // No native registry is treated as authoritative. Bind a live process
            // to the exact transcript or an explicit session argument and cwd.
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
            if open_file || (cwd_matches && explicit) {
                if p.started_ms
                    .zip(parsed.last_turn_at)
                    .is_some_and(|(start, turn)| start <= turn)
                {
                    return Ownership::Open;
                }
                uncertain = true;
            } else if p.cwd.is_none() || cwd_matches || p.args.iter().any(|a| a == &parsed.id) {
                uncertain = true;
            }
        }
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
    let output = Command::new("ps")
        .args(["-axo", "pid=,stat=,lstart=,comm=,args="])
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
                processes: Some(vec![owner.clone()])
            }
            .ownership(&p, file),
            Ownership::Open
        );
        assert_eq!(
            Snapshot {
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
                processes: Some(vec![Process {
                    args: vec!["claude".into()],
                    ..owner
                }])
            }
            .ownership(&p, file),
            Ownership::Unknown
        );
        assert_eq!(
            Snapshot { processes: None }.ownership(&p, file),
            Ownership::Unknown
        );
        assert!(!native("zsh", &["claude".into()]));
    }
}
