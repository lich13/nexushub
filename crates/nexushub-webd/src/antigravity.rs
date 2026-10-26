//! Local-only, allowlisted access to the already running Antigravity App.
use anyhow::{ensure, Context, Result};
use nexushub_core::{
    antigravity,
    native_probe::{NativeScan, NativeStreamSnapshot},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, time::Duration};

const SERVICE: &str = "/exa.language_server_pb.LanguageServerService/";
const LIMIT: usize = nexushub_core::resource_limits::REMOTE_RESPONSE_BYTES;
static CACHE: tokio::sync::Mutex<VecDeque<(String, String, NativeStreamSnapshot)>> =
    tokio::sync::Mutex::const_new(VecDeque::new());
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
        .filter_map(|line| {
            line.strip_prefix("n127.0.0.1:")
                .and_then(|p| p.parse::<u16>().ok())
        })
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
        matches!(method, "GetAllCascadeTrajectories" | "GetCascadeTrajectory"),
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
            json!({"excludeSubtrajectories":true}),
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
        summaries.len() <= 2000,
        "Antigravity source inventory exceeds limit"
    );
    let mut scan = NativeScan::default();
    for (id, summary) in summaries {
        if only.is_some_and(|target| target != id) {
            continue;
        }
        let signature = hex::encode(Sha256::digest(serde_json::to_vec(&json!([pid, summary]))?));
        // Revalidation always obtains fresh native terminal / interaction state.
        let cached = if only.is_none() {
            CACHE
                .lock()
                .await
                .iter()
                .find(|(key, sig, s)| {
                    key == id
                        && sig == &signature
                        && s.record_count == s.settled_count
                        && !s
                            .events
                            .iter()
                            .any(|e| matches!(e.kind.as_str(), "reply_needed" | "authorization"))
                })
                .map(|(_, _, s)| s.clone())
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
        let result = query(
            client,
            port,
            token,
            "GetCascadeTrajectory",
            json!({"cascadeId":id,"disableRehydration":true}),
        )
        .await
        .and_then(|response| antigravity::snapshot(id, summary, &response));
        match result {
            Ok(snapshot) => {
                let mut cache = CACHE.lock().await;
                cache.retain(|(key, _, _)| key != id);
                cache.push_back((id.clone(), signature, snapshot.clone()));
                while cache.len() > 64
                    || cache
                        .iter()
                        .map(|(_, _, s)| {
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
