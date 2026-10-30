//! Local-only, allowlisted access to the already running Antigravity App.
use anyhow::{ensure, Context, Result};
use nexushub_core::{
    antigravity,
    native_probe::{NativeScan, NativeStreamSnapshot},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Duration,
};

const SERVICE: &str = "/exa.language_server_pb.LanguageServerService/";
const LIMIT: usize = nexushub_core::resource_limits::ANTIGRAVITY_LOCAL_RPC_BYTES;
static CACHE: tokio::sync::Mutex<
    VecDeque<(String, String, NativeStreamSnapshot, std::time::Instant)>,
> = tokio::sync::Mutex::const_new(VecDeque::new());

pub async fn evict_idle_cache(max_idle: Duration) {
    let mut cache = CACHE.lock().await;
    let now = std::time::Instant::now();
    cache.retain(|(_, _, _, touched)| now.saturating_duration_since(*touched) < max_idle);
}
struct Endpoint {
    pid: u32,
    ports: Vec<u16>,
    token: String,
}

#[cfg(target_os = "macos")]
fn discover() -> Result<Endpoint> {
    use std::process::Command;
    let output = Command::new("/bin/ps")
        .args(["-ww", "-axo", "pid=,uid=,comm="])
        .output()?;
    ensure!(output.status.success(), "Antigravity process unavailable");
    let processes = String::from_utf8(output.stdout)?;
    let uid = unsafe { libc_uid() };
    let mut owners = Vec::new();
    for line in processes.lines() {
        let mut fields = line.split_whitespace();
        let Some(pid) = fields.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Some(owner) = fields.next().and_then(|s| s.trim().parse::<u32>().ok()) else {
            continue;
        };
        let path = fields.collect::<Vec<_>>().join(" ");
        if owner == uid
            && path.ends_with("/Antigravity.app/Contents/Resources/bin/language_server")
            && std::path::Path::new(&path).is_file()
        {
            owners.push(pid);
        }
    }
    ensure!(
        owners.len() == 1,
        "Antigravity native owner unavailable or ambiguous"
    );
    let pid = owners[0];
    let output = Command::new("/bin/ps")
        .args(["-ww", "-p", &pid.to_string(), "-o", "command="])
        .output()?;
    ensure!(
        output.status.success(),
        "Antigravity native owner unavailable"
    );
    let command = String::from_utf8(output.stdout)?;
    let args = command.split_whitespace().collect::<Vec<_>>();
    let token = args
        .windows(2)
        .find(|w| w[0] == "--csrf_token")
        .map(|w| w[1])
        .context("Antigravity authentication unavailable")?;
    ensure!(
        !token.is_empty()
            && token.len() <= 256
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')),
        "Antigravity authentication unavailable"
    );
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-Pan",
            "-p",
            &pid.to_string(),
            "-iTCP",
            "-sTCP:LISTEN",
            "-Fn",
        ])
        .output()?;
    ensure!(output.status.success(), "Antigravity listener unavailable");
    let mut ports = String::from_utf8(output.stdout)?
        .lines()
        .filter_map(parse_loopback_listener_port)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    ensure!(
        !ports.is_empty() && ports.len() <= 8,
        "Antigravity loopback listener unavailable"
    );
    Ok(Endpoint {
        pid,
        ports,
        token: token.into(),
    })
}
#[cfg(target_os = "macos")]
unsafe fn libc_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}
#[cfg(not(target_os = "macos"))]
fn discover() -> Result<Endpoint> {
    anyhow::bail!("Antigravity notifications require the local Mac App")
}

#[cfg(target_os = "macos")]
fn parse_loopback_listener_port(line: &str) -> Option<u16> {
    use std::net::Ipv4Addr;
    let (host, port) = line.strip_prefix('n')?.split_once(':')?;
    (host.parse::<Ipv4Addr>().ok() == Some(Ipv4Addr::LOCALHOST))
        .then(|| port.parse::<u16>().ok())
        .flatten()
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()?)
}
async fn query(
    client: &reqwest::Client,
    port: u16,
    token: &str,
    method: &str,
    body: Value,
) -> Result<Value> {
    ensure!(
        matches!(
            method,
            "GetAllCascadeTrajectories"
                | "GetCascadeTrajectory"
                | "GetCascadeTrajectorySteps"
                | "GetCascadeTrajectoryExecutorMetadatas"
        ),
        "unsupported Antigravity read method"
    );
    let url = format!("http://127.0.0.1:{port}{SERVICE}{method}");
    let mut response = client
        .post(url)
        .header("x-codeium-csrf-token", token)
        .header("Connect-Protocol-Version", "1")
        .json(&body)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Antigravity read unavailable"))?;
    ensure!(
        response.status().is_success()
            && response.content_length().is_none_or(|n| n <= LIMIT as u64),
        "Antigravity response unavailable"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Antigravity read unavailable"))?
    {
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= LIMIT,
            "Antigravity response exceeds limit"
        );
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid Antigravity response"))
}
async fn connect() -> Result<(Endpoint, reqwest::Client, u16, Value)> {
    let endpoint = tokio::task::spawn_blocking(discover).await??;
    let client = client()?;
    for &port in &endpoint.ports {
        if let Ok(value) = query(
            &client,
            port,
            &endpoint.token,
            "GetAllCascadeTrajectories",
            json!({"excludeSubtrajectories":false}),
        )
        .await
        {
            if value["trajectorySummaries"].is_object() {
                return Ok((endpoint, client, port, value));
            }
        }
    }
    anyhow::bail!("Antigravity native API unavailable")
}

fn is_subtrajectory(id: &str, item: &Value) -> bool {
    let parent = item["parentConversationId"]
        .as_str()
        .or_else(|| item["trajectoryMetadata"]["parentConversationId"].as_str())
        .unwrap_or("");
    let depth = item["nestingDepth"]
        .as_i64()
        .or_else(|| item["trajectoryMetadata"]["nestingDepth"].as_i64())
        .unwrap_or(0);
    let root = item["rootConversationId"]
        .as_str()
        .or_else(|| item["trajectoryMetadata"]["rootConversationId"].as_str())
        .unwrap_or("");
    !parent.is_empty() || depth > 0 || (!root.is_empty() && root != id)
}

fn summary_is_active(item: &Value) -> bool {
    item["waitingSteps"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty())
        || matches!(
            item["status"].as_str(),
            Some(
                "CASCADE_RUN_STATUS_RUNNING"
                    | "CASCADE_RUN_STATUS_BUSY"
                    | "CASCADE_RUN_STATUS_WAITING"
                    | "CASCADE_RUN_STATUS_CANCELING"
            )
        )
        || item["notFullyIdle"].as_bool() == Some(true)
        || item["hasRunningSubagents"].as_bool() == Some(true)
}

fn collect_subagents_by_root(
    summaries: &serde_json::Map<String, Value>,
) -> BTreeMap<String, BTreeMap<String, antigravity::SubagentLiveness>> {
    let mut parents: BTreeMap<&str, &str> = BTreeMap::new();
    for (cid, item) in summaries {
        if let Some(pid) = item["parentConversationId"]
            .as_str()
            .or_else(|| item["trajectoryMetadata"]["parentConversationId"].as_str())
            .filter(|s| !s.is_empty())
        {
            parents.insert(cid.as_str(), pid);
        }
    }
    let mut by_root: BTreeMap<String, BTreeMap<String, antigravity::SubagentLiveness>> =
        BTreeMap::new();
    for (cid, item) in summaries {
        if !is_subtrajectory(cid, item) {
            continue;
        }
        let liveness = antigravity::SubagentLiveness {
            active: summary_is_active(item),
            last_modified_ms: nexushub_core::native_probe::timestamp(&item["lastModifiedTime"])
                .or_else(|| nexushub_core::native_probe::timestamp(&item["createdTime"])),
        };
        let mut target_roots = BTreeSet::new();
        if let Some(root) = item["rootConversationId"]
            .as_str()
            .or_else(|| item["trajectoryMetadata"]["rootConversationId"].as_str())
            .filter(|s| !s.is_empty() && *s != cid)
        {
            target_roots.insert(root.to_owned());
        }
        let mut curr = cid.as_str();
        let mut hops = 0usize;
        while let Some(&pid) = parents.get(curr) {
            target_roots.insert(pid.to_owned());
            curr = pid;
            hops += 1;
            if hops >= 16 {
                break;
            }
        }
        for root in target_roots {
            by_root
                .entry(root)
                .or_default()
                .insert(cid.clone(), liveness.clone());
        }
    }
    by_root
}

fn decode_local_file_uri(uri: &str) -> Option<std::path::PathBuf> {
    let raw = uri.trim();
    let stripped = raw.strip_prefix("file://").unwrap_or(raw);
    let bytes = stripped.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = std::path::PathBuf::from(String::from_utf8_lossy(&out).into_owned());
    path.is_absolute().then_some(path)
}

fn has_open_file_handle(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        let bin = if std::path::Path::new("/usr/sbin/lsof").is_file() {
            "/usr/sbin/lsof"
        } else {
            "lsof"
        };
        std::process::Command::new(bin)
            .args(["-Fp", "--"])
            .arg(path)
            .output()
            .is_ok_and(|out| out.status.success() && out.stdout.contains(&b'p'))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn inspect_task_log(log_uri: &str) -> Option<antigravity::TaskLiveness> {
    let path = decode_local_file_uri(log_uri)?;
    let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
    let last_activity_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    Some(antigravity::TaskLiveness {
        active: has_open_file_handle(&path),
        last_activity_ms,
    })
}

fn parse_ps_etime_ms(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (days, rest) = match trimmed.split_once('-') {
        Some((d, r)) => (d.parse::<i64>().ok()?, r),
        None => (0, trimmed),
    };
    let parts: Vec<i64> = rest
        .split(':')
        .map(|p| p.parse::<i64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (hours, mins, secs) = match parts.as_slice() {
        [m, s] => (0, *m, *s),
        [h, m, s] => (*h, *m, *s),
        _ => return None,
    };
    if days < 0 || hours < 0 || !(0..60).contains(&mins) || !(0..60).contains(&secs) {
        return None;
    }
    let total_secs = days
        .checked_mul(86_400)?
        .checked_add(hours.checked_mul(3_600)?)?
        .checked_add(mins.checked_mul(60)?)?
        .checked_add(secs)?;
    total_secs.checked_mul(1_000)
}

fn read_ps_ppid_etime() -> Option<String> {
    #[cfg(unix)]
    {
        let bin = if std::path::Path::new("/bin/ps").is_file() {
            "/bin/ps"
        } else {
            "ps"
        };
        let out = std::process::Command::new(bin)
            .args(["-axo", "ppid=,etime="])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8(out.stdout).ok())
            .flatten()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn has_active_child_process_since(parent_pid: u32, since_ms: i64, now_ms: i64) -> bool {
    if parent_pid == 0 {
        return false;
    }
    let Some(table) = read_ps_ppid_etime() else {
        return false;
    };
    const MAX_COMMAND_AGE_MS: i64 = 4 * 3_600 * 1_000;
    const CLOCK_SKEW_SLACK_MS: i64 = 5_000;
    table.lines().any(|line| {
        let mut cols = line.split_whitespace();
        let Some(ppid) = cols.next().and_then(|s| s.parse::<u32>().ok()) else {
            return false;
        };
        if ppid != parent_pid {
            return false;
        }
        let Some(elapsed_ms) = cols.next().and_then(parse_ps_etime_ms) else {
            return false;
        };
        if elapsed_ms > MAX_COMMAND_AGE_MS {
            return false;
        }
        if since_ms <= 0 {
            return true;
        }
        now_ms
            .saturating_sub(elapsed_ms)
            .saturating_add(CLOCK_SKEW_SLACK_MS)
            >= since_ms
    })
}

async fn fetch_incremental_turn_trajectory(
    client: &reqwest::Client,
    port: u16,
    token: &str,
    id: &str,
    summary: &Value,
) -> Result<Option<Value>> {
    let Some(total) = summary["stepCount"]
        .as_u64()
        .filter(|&n| n > 0 && n <= 200_000)
    else {
        return Ok(None);
    };
    let Some(turn_offset) = summary["lastUserInputStepIndex"]
        .as_u64()
        .filter(|&off| off < total)
    else {
        return Ok(None);
    };
    let total_usize = total as usize;
    let offset_usize = turn_offset as usize;
    let mut steps = Vec::new();
    while offset_usize + steps.len() < total_usize && steps.len() < 50_000 {
        let more = query(
            client,
            port,
            token,
            "GetCascadeTrajectorySteps",
            json!({"cascadeId": id, "stepOffset": offset_usize + steps.len()}),
        )
        .await?;
        let Some(batch) = more["steps"].as_array().filter(|b| !b.is_empty()) else {
            break;
        };
        steps.extend(batch.iter().cloned());
    }
    if offset_usize + steps.len() != total_usize {
        return Ok(None);
    }
    let mut metas = Vec::new();
    loop {
        if metas.len() >= 20_000 {
            return Ok(None);
        }
        let page = query(
            client,
            port,
            token,
            "GetCascadeTrajectoryExecutorMetadatas",
            json!({"cascadeId": id, "executorMetadataOffset": metas.len()}),
        )
        .await?;
        let Some(batch) = page["executorMetadata"]
            .as_array()
            .or_else(|| page["executorMetadatas"].as_array())
            .filter(|b| !b.is_empty())
        else {
            break;
        };
        metas.extend(batch.iter().cloned());
    }
    Ok(Some(json!({
        "status": summary["status"],
        "stepOffset": turn_offset,
        "numTotalSteps": total,
        "trajectory": {
            "cascadeId": id,
            "trajectoryId": summary["trajectoryId"],
            "steps": steps,
            "executorMetadatas": metas,
        }
    })))
}

async fn fetch_probe_trajectory(
    client: &reqwest::Client,
    port: u16,
    token: &str,
    id: &str,
    summary: &Value,
) -> Result<Value> {
    if let Ok(Some(incremental)) =
        fetch_incremental_turn_trajectory(client, port, token, id, summary).await
    {
        return Ok(incremental);
    }
    fetch_complete_trajectory(client, port, token, id).await
}

async fn fetch_complete_trajectory(
    client: &reqwest::Client,
    port: u16,
    token: &str,
    id: &str,
) -> Result<Value> {
    let mut response = query(
        client,
        port,
        token,
        "GetCascadeTrajectory",
        json!({"cascadeId": id, "disableRehydration": true}),
    )
    .await?;
    let total = response["numTotalSteps"].as_u64().unwrap_or_else(|| {
        response["trajectory"]["steps"]
            .as_array()
            .map_or(0, Vec::len) as u64
    }) as usize;
    let mut current_len = response["trajectory"]["steps"]
        .as_array()
        .map_or(0, Vec::len);
    while current_len < total && current_len < 50_000 {
        let more = query(
            client,
            port,
            token,
            "GetCascadeTrajectorySteps",
            json!({"cascadeId": id, "stepOffset": current_len}),
        )
        .await?;
        let Some(batch) = more["steps"].as_array().filter(|b| !b.is_empty()) else {
            break;
        };
        if let Some(steps) = response["trajectory"]["steps"].as_array_mut() {
            steps.extend(batch.iter().cloned());
            current_len = steps.len();
        } else {
            break;
        }
    }
    Ok(response)
}
pub async fn scan() -> Result<NativeScan> {
    let (endpoint, client, port, summaries) = connect().await?;
    scan_at(
        &client,
        port,
        &endpoint.token,
        endpoint.pid,
        &summaries,
        None,
    )
    .await
}
pub async fn source_current(delivery: &nexushub_core::db::NativeDelivery) -> Result<bool> {
    let (endpoint, client, port, summaries) = connect().await?;
    let scan = scan_at(
        &client,
        port,
        &endpoint.token,
        endpoint.pid,
        &summaries,
        Some(&delivery.thread_id),
    )
    .await?;
    super::provider_monitor::local_event_current(&scan, delivery)
}
async fn scan_at(
    client: &reqwest::Client,
    port: u16,
    token: &str,
    pid: u32,
    value: &Value,
    only: Option<&str>,
) -> Result<NativeScan> {
    let summaries = value["trajectorySummaries"]
        .as_object()
        .context("missing Antigravity summaries")?;
    ensure!(
        summaries.len() <= 5000,
        "Antigravity source inventory exceeds limit"
    );
    let subagents_by_root = collect_subagents_by_root(summaries);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut scan = NativeScan::default();
    for (id, summary) in summaries {
        if is_subtrajectory(id, summary) {
            continue;
        }
        if only.is_some_and(|target| target != id) {
            continue;
        }
        let subagents = subagents_by_root.get(id).cloned().unwrap_or_default();
        let has_active_subagents = subagents.values().any(|s| s.active);
        let signature = hex::encode(Sha256::digest(serde_json::to_vec(&json!([
            pid,
            summary,
            has_active_subagents
        ]))?));
        // Revalidation always obtains fresh native terminal / interaction state.
        let cached = if only.is_none() {
            let mut cache = CACHE.lock().await;
            cache
                .iter_mut()
                .find(|(key, sig, s, _)| {
                    key == id
                        && sig == &signature
                        && s.record_count == s.settled_count
                        && !s
                            .events
                            .iter()
                            .any(|e| matches!(e.kind.as_str(), "reply_needed" | "authorization"))
                })
                .map(|(_, _, s, touched)| {
                    *touched = std::time::Instant::now();
                    s.clone()
                })
        } else {
            None
        };
        if let Some(snapshot) = cached {
            scan.streams.push(snapshot);
            continue;
        }
        if antigravity::identity(id, summary).is_err() {
            scan.errors += 1;
            continue;
        }
        let result = match fetch_probe_trajectory(client, port, token, id, summary).await {
            Ok(response) => {
                let mut tasks = BTreeMap::new();
                let mut has_active_child_processes = false;
                if !summary_is_active(summary) && !has_active_subagents {
                    if let Some(steps) = response["trajectory"]["steps"].as_array() {
                        let summary_traj_id = summary["trajectoryId"].as_str().unwrap_or("");
                        let pending_logs =
                            antigravity::collect_pending_task_logs(id, summary_traj_id, steps);
                        if !pending_logs.is_empty() {
                            tasks = tokio::task::spawn_blocking(move || {
                                pending_logs
                                    .into_iter()
                                    .filter_map(|(tid, uri)| {
                                        inspect_task_log(&uri).map(|l| (tid, l))
                                    })
                                    .collect()
                            })
                            .await
                            .unwrap_or_default();
                        }
                        if let Some(since_ms) = antigravity::earliest_untracked_task_spawn_ms(
                            id,
                            summary_traj_id,
                            steps,
                            &tasks,
                        ) {
                            has_active_child_processes = tokio::task::spawn_blocking(move || {
                                has_active_child_process_since(pid, since_ms, now_ms)
                            })
                            .await
                            .unwrap_or(false);
                        }
                    }
                }
                let ctx = antigravity::TrajectoryLivenessContext {
                    has_active_subagents,
                    has_active_child_processes,
                    now_ms: Some(now_ms),
                    subagents,
                    tasks,
                };
                antigravity::snapshot_with_context(id, summary, &response, &ctx)
            }
            Err(err) => Err(err),
        };
        match result {
            Ok(snapshot) => {
                let mut cache = CACHE.lock().await;
                cache.retain(|(key, _, _, _)| key != id);
                cache.push_back((
                    id.clone(),
                    signature,
                    snapshot.clone(),
                    std::time::Instant::now(),
                ));
                while cache.len() > 64
                    || cache
                        .iter()
                        .map(|(_, _, s, _)| {
                            s.events
                                .iter()
                                .map(|e| e.body.len() + e.turn_id.len() + 256)
                                .sum::<usize>()
                        })
                        .sum::<usize>()
                        > 32 * 1024 * 1024
                {
                    cache.pop_front();
                }
                scan.streams.push(snapshot);
            }
            Err(_) => scan.errors += 1,
        }
    }
    Ok(scan)
}

#[cfg(test)]
mod tests;
