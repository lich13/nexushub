use crate::{db::PanelDb, security::redact_output};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::broadcast,
};
use uuid::Uuid;

struct CodexJobCommand {
    codex_home: std::path::PathBuf,
    cwd: std::path::PathBuf,
    args: Vec<String>,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: String,
    pub status: String,
    pub chunk: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexActionResult {
    pub bridge: bool,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub job_id: Option<String>,
    pub fallback: bool,
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct JobRunner {
    db: PanelDb,
    tx: broadcast::Sender<JobEvent>,
    running: Arc<Mutex<HashMap<String, u32>>>,
}

impl JobRunner {
    pub fn new(db: PanelDb) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            db,
            tx,
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.tx.subscribe()
    }

    pub fn start_shell_job(&self, kind: &str, title: &str, command: String) -> Result<String> {
        self.start_shell_job_inner(kind, title, command, None)
    }

    pub fn start_exclusive_shell_job(
        &self,
        kind: &str,
        title: &str,
        command: String,
        group: &str,
    ) -> Result<String> {
        self.start_shell_job_inner(kind, title, command, Some(group))
    }

    fn start_shell_job_inner(
        &self,
        kind: &str,
        title: &str,
        command: String,
        exclusive_group: Option<&str>,
    ) -> Result<String> {
        let exclusive_group_lock_key = exclusive_group.map(exclusive_group_key);
        if let Some(group) = exclusive_group {
            let key = exclusive_group_lock_key
                .as_deref()
                .expect("exclusive group key");
            let mut running = self.running.lock().expect("running jobs");
            if running.contains_key(key) {
                anyhow::bail!("codex update job already running: {group}");
            }
            running.insert(key.to_string(), 0);
        }
        let id = Uuid::new_v4().to_string();
        if let Err(err) = self.db.create_job(&id, kind, title) {
            if let Some(key) = exclusive_group_lock_key.as_deref() {
                self.running.lock().expect("running jobs").remove(key);
            }
            return Err(err);
        }
        let db = self.db.clone();
        let tx = self.tx.clone();
        let running = self.running.clone();
        let id_for_task = id.clone();
        let exclusive_group = exclusive_group.map(str::to_string);
        spawn_job_task(async move {
            let result = run_shell_command(
                &db,
                &tx,
                &running,
                &id_for_task,
                &command,
                exclusive_group.clone(),
            )
            .await;
            if let Err(err) = result {
                if let Some(group) = exclusive_group.as_deref() {
                    running
                        .lock()
                        .expect("running jobs")
                        .remove(&exclusive_group_key(group));
                }
                let _ = db.append_job_output(&id_for_task, &format!("\nerror: {err}\n"));
                let _ = db.finish_job(&id_for_task, "failed", None, Some(&err.to_string()));
                let _ = tx.send(JobEvent {
                    job_id: id_for_task,
                    status: "failed".to_string(),
                    chunk: Some(err.to_string()),
                });
            }
        });
        Ok(id)
    }

    pub fn exclusive_group_job(&self, group: &str) -> Option<String> {
        let key = exclusive_group_key(group);
        let running = self.running.lock().expect("running jobs");
        running.contains_key(&key).then_some(group.to_string())
    }

    pub fn start_codex_job(
        &self,
        title: &str,
        codex_home: &Path,
        cwd: &Path,
        args: Vec<String>,
        prompt: String,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        self.db.create_job(&id, "codex_chat", title)?;
        let db = self.db.clone();
        let tx = self.tx.clone();
        let running = self.running.clone();
        let id_for_task = id.clone();
        let command = CodexJobCommand {
            codex_home: codex_home.to_path_buf(),
            cwd: cwd.to_path_buf(),
            args,
            prompt,
        };
        spawn_job_task(async move {
            let result = run_codex_command(&db, &tx, &running, &id_for_task, command).await;
            if let Err(err) = result {
                let _ = db.append_job_output(&id_for_task, &format!("\nerror: {err}\n"));
                let _ = db.finish_job(&id_for_task, "failed", None, Some(&err.to_string()));
                let _ = tx.send(JobEvent {
                    job_id: id_for_task,
                    status: "failed".to_string(),
                    chunk: Some(err.to_string()),
                });
            }
        });
        Ok(id)
    }

    pub fn cancel_job(&self, id: &str) -> Result<bool> {
        let pid = self.running.lock().expect("running jobs").get(id).copied();
        let Some(pid) = pid else {
            return Ok(false);
        };
        #[cfg(unix)]
        {
            let status = std::process::Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status()
                .context("send TERM to job")?;
            Ok(status.success())
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            Ok(false)
        }
    }
}

async fn run_shell_command(
    db: &PanelDb,
    tx: &broadcast::Sender<JobEvent>,
    running: &Arc<Mutex<HashMap<String, u32>>>,
    job_id: &str,
    command: &str,
    exclusive_group: Option<String>,
) -> Result<()> {
    let mut child = Command::new("bash")
        .arg("-lc")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn job command: {command}"))?;
    if let Some(pid) = child.id() {
        running
            .lock()
            .expect("running jobs")
            .insert(job_id.to_string(), pid);
    }

    let mut handles = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        handles.push(stream_reader(
            db.clone(),
            tx.clone(),
            job_id.to_string(),
            stdout,
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        handles.push(stream_reader(
            db.clone(),
            tx.clone(),
            job_id.to_string(),
            stderr,
        ));
    }
    let status = child.wait().await?;
    {
        let mut running = running.lock().expect("running jobs");
        running.remove(job_id);
        if let Some(group) = exclusive_group.as_deref() {
            running.remove(&exclusive_group_key(group));
        }
    }
    for handle in handles {
        let _ = handle.await;
    }
    let code = status.code();
    let status_text = if status.success() {
        "succeeded"
    } else {
        "failed"
    };
    db.finish_job(job_id, status_text, code, None)?;
    let _ = tx.send(JobEvent {
        job_id: job_id.to_string(),
        status: status_text.to_string(),
        chunk: None,
    });
    Ok(())
}

fn exclusive_group_key(group: &str) -> String {
    format!("exclusive:{group}")
}

fn spawn_job_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(future);
        return;
    }

    std::thread::Builder::new()
        .name("nexushub-job-runtime".to_string())
        .spawn(move || {
            match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(future),
                Err(err) => eprintln!("failed to start NexusHub job runtime: {err}"),
            }
        })
        .expect("spawn NexusHub job runtime thread");
}

async fn run_codex_command(
    db: &PanelDb,
    tx: &broadcast::Sender<JobEvent>,
    running: &Arc<Mutex<HashMap<String, u32>>>,
    job_id: &str,
    command: CodexJobCommand,
) -> Result<()> {
    let mut process = codex_command_builder(&command.codex_home);
    let mut child = process
        .args(command.args)
        .current_dir(command.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn codex chat job")?;
    if let Some(pid) = child.id() {
        running
            .lock()
            .expect("running jobs")
            .insert(job_id.to_string(), pid);
    }

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(command.prompt.as_bytes()).await?;
        stdin.shutdown().await?;
    }

    let mut handles = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        handles.push(stream_reader(
            db.clone(),
            tx.clone(),
            job_id.to_string(),
            stdout,
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        handles.push(stream_reader(
            db.clone(),
            tx.clone(),
            job_id.to_string(),
            stderr,
        ));
    }
    let status = child.wait().await?;
    running.lock().expect("running jobs").remove(job_id);
    for handle in handles {
        let _ = handle.await;
    }
    let code = status.code();
    let status_text = if status.success() {
        "succeeded"
    } else {
        "failed"
    };
    db.finish_job(job_id, status_text, code, None)?;
    let _ = tx.send(JobEvent {
        job_id: job_id.to_string(),
        status: status_text.to_string(),
        chunk: None,
    });
    Ok(())
}

fn codex_command_builder(codex_home: &Path) -> Command {
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        let mut command_builder = Command::new("codex");
        command_builder.env("CODEX_HOME", codex_home);
        command_builder
    } else {
        let mut command_builder = Command::new("sudo");
        command_builder
            .args(["-n", "env"])
            .arg(format!("CODEX_HOME={}", codex_home.display()))
            .arg("codex");
        command_builder
    }
}

#[cfg(test)]
fn codex_command_shape(codex_home: &Path) -> (String, Vec<String>, Option<String>) {
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        (
            "codex".to_string(),
            Vec::new(),
            Some(codex_home.display().to_string()),
        )
    } else {
        (
            "sudo".to_string(),
            vec![
                "-n".to_string(),
                "env".to_string(),
                format!("CODEX_HOME={}", codex_home.display()),
                "codex".to_string(),
            ],
            None,
        )
    }
}

fn stream_reader<R>(
    db: PanelDb,
    tx: broadcast::Sender<JobEvent>,
    job_id: String,
    reader: R,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buf = [0_u8; 2048];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = redact_output(&String::from_utf8_lossy(&buf[..n]));
                    let _ = db.append_job_output(&job_id, &chunk);
                    let _ = tx.send(JobEvent {
                        job_id: job_id.clone(),
                        status: "running".to_string(),
                        chunk: Some(chunk),
                    });
                }
                Err(_) => break,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{codex_command_shape, JobRunner};
    use crate::db::PanelDb;
    use std::{fs, path::Path, thread, time::Duration};
    use uuid::Uuid;

    #[tokio::test]
    async fn exclusive_shell_jobs_reject_same_group_until_finished() {
        let db = PanelDb::open(":memory:").unwrap();
        let runner = JobRunner::new(db);
        let mut events = runner.subscribe();

        let first = runner
            .start_exclusive_shell_job(
                "panel_update_start",
                "Panel update",
                "sleep 0.2".to_string(),
                "panel_update",
            )
            .unwrap();
        assert!(!first.is_empty());

        let second = runner.start_exclusive_shell_job(
            "panel_update_prune",
            "Panel prune",
            "true".to_string(),
            "panel_update",
        );
        assert!(second.is_err());

        wait_for_job_status(&mut events, &first, "succeeded").await;
        assert!(runner.exclusive_group_job("panel_update").is_none());

        let third = runner.start_exclusive_shell_job(
            "panel_update_prune",
            "Panel prune",
            "true".to_string(),
            "panel_update",
        );
        assert!(third.is_ok());
    }

    async fn wait_for_job_status(
        events: &mut tokio::sync::broadcast::Receiver<super::JobEvent>,
        job_id: &str,
        status: &str,
    ) {
        let timeout = tokio::time::sleep(Duration::from_secs(3));
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                _ = &mut timeout => panic!("timed out waiting for job {job_id} to reach {status}"),
                event = events.recv() => {
                    let event = event.expect("job event");
                    if event.job_id == job_id && event.status == status {
                        return;
                    }
                }
            }
        }
    }

    #[test]
    fn shell_jobs_start_without_current_tokio_runtime() {
        let db_path =
            std::env::temp_dir().join(format!("nexushub-job-runner-{}.sqlite", Uuid::new_v4()));
        let db = PanelDb::open(&db_path).unwrap();
        let runner = JobRunner::new(db.clone());

        let job_id = runner
            .start_exclusive_shell_job(
                "probe_bark_test",
                "Probe Bark test",
                "true".to_string(),
                "probe:bark-test",
            )
            .unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let job = db.job(&job_id).unwrap().expect("job exists");
            if job.status == "succeeded" {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!(
                    "timed out waiting for job {job_id}, last status {}",
                    job.status
                );
            }
            thread::sleep(Duration::from_millis(50));
        }

        drop(runner);
        drop(db);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn desktop_codex_jobs_do_not_require_sudo() {
        let (program, args, codex_home_env) =
            codex_command_shape(Path::new("/Users/example/.codex"));

        if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
            assert_eq!(program, "codex");
            assert!(args.is_empty());
            assert_eq!(codex_home_env.as_deref(), Some("/Users/example/.codex"));
        } else {
            assert_eq!(program, "sudo");
            assert_eq!(args.first().map(String::as_str), Some("-n"));
            assert!(args
                .iter()
                .any(|arg| arg == "CODEX_HOME=/Users/example/.codex"));
        }
    }
}
