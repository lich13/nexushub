use crate::{
    config::{DEFAULT_SESSION_TTL_SECONDS, LEGACY_SESSION_TTL_SECONDS},
    crypto::SecretBox,
    security::hash_token,
};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PanelDb {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
    crypto: SecretBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeLogsDbCounts {
    pub event_count: usize,
    pub dedupe_count: usize,
    pub pending_event_count: usize,
    pub pending_dedupe_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Admin {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub admin_id: String,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewSession<'a> {
    pub id: &'a str,
    pub admin_id: &'a str,
    pub token: &'a str,
    pub csrf_token: &'a str,
    pub user_agent: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub turnstile_enabled: bool,
    pub turnstile_required: bool,
    pub turnstile_site_key: Option<String>,
    pub turnstile_secret_configured: bool,
    pub session_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadFollowUp {
    pub id: String,
    pub thread_id: String,
    pub status: String,
    pub message: String,
    pub options_json: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub submitted_at: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub result_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeEvent {
    pub id: String,
    pub kind: String,
    pub thread_id: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub dedupe_key: Option<String>,
    pub source: String,
    pub payload: Value,
    pub created_at: i64,
    pub handled_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewProbeEvent<'a> {
    pub kind: &'a str,
    pub thread_id: Option<&'a str>,
    pub title: Option<&'a str>,
    pub message: Option<&'a str>,
    pub dedupe_key: Option<&'a str>,
    pub source: &'a str,
    pub payload: Value,
}

impl PanelDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_secret_box(path, SecretBox::deterministic_dev())
    }

    pub fn open_with_secret_box(path: impl AsRef<Path>, crypto: SecretBox) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create data dir {}", parent.display()))?;
        }
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        let db = Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
            crypto,
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn now() -> i64 {
        Utc::now().timestamp()
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS admins (
              id TEXT PRIMARY KEY,
              username TEXT NOT NULL UNIQUE,
              password_hash TEXT NOT NULL,
              created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
              id TEXT PRIMARY KEY,
              admin_id TEXT NOT NULL,
              token_hash TEXT NOT NULL UNIQUE,
              csrf_token_hash TEXT NOT NULL,
              user_agent TEXT,
              ip TEXT,
              expires_at INTEGER NOT NULL,
              created_at INTEGER NOT NULL,
              revoked_at INTEGER,
              FOREIGN KEY(admin_id) REFERENCES admins(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_log (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              admin_id TEXT,
              action TEXT NOT NULL,
              target_type TEXT,
              target_id TEXT,
              ip TEXT,
              detail_json TEXT NOT NULL,
              created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS jobs (
              id TEXT PRIMARY KEY,
              kind TEXT NOT NULL,
              status TEXT NOT NULL,
              title TEXT NOT NULL,
              thread_id TEXT,
              turn_id TEXT,
              started_at INTEGER NOT NULL,
              finished_at INTEGER,
              exit_code INTEGER,
              output TEXT NOT NULL DEFAULT '',
              error TEXT
            );

            CREATE TABLE IF NOT EXISTS turnstile_attempts (
              token_hash TEXT PRIMARY KEY,
              action TEXT,
              hostname TEXT,
              remote_ip TEXT,
              success INTEGER NOT NULL,
              error_codes TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              expires_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS thread_followups (
              id TEXT PRIMARY KEY,
              thread_id TEXT NOT NULL,
              status TEXT NOT NULL,
              message TEXT NOT NULL,
              options_json TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              submitted_at INTEGER,
              cancelled_at INTEGER,
              result_json TEXT,
              error TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_thread_followups_thread_status_created
              ON thread_followups(thread_id, status, created_at);

            CREATE TABLE IF NOT EXISTS probe_events (
              id TEXT PRIMARY KEY,
              kind TEXT NOT NULL,
              thread_id TEXT,
              title TEXT,
              message TEXT,
              dedupe_key TEXT,
              source TEXT NOT NULL,
              payload_json TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              handled_at INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_probe_events_created_at
              ON probe_events(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_probe_events_thread_created_at
              ON probe_events(thread_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS probe_dedupe (
              namespace TEXT NOT NULL,
              dedupe_key TEXT NOT NULL,
              expires_at INTEGER NOT NULL,
              created_at INTEGER NOT NULL,
              PRIMARY KEY(namespace, dedupe_key)
            );
            "#,
        )?;
        add_column_if_missing(&conn, "jobs", "thread_id", "TEXT")?;
        add_column_if_missing(&conn, "jobs", "turn_id", "TEXT")?;
        add_column_if_missing(&conn, "probe_events", "handled_at", "INTEGER")?;
        let legacy = LEGACY_SESSION_TTL_SECONDS.to_string();
        let current: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key='session_ttl_seconds'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if current.as_deref() == Some(&legacy) {
            conn.execute(
                "UPDATE settings SET value=?1, updated_at=?2 WHERE key='session_ttl_seconds'",
                params![DEFAULT_SESSION_TTL_SECONDS.to_string(), Self::now()],
            )?;
        }
        drop(conn);
        self.migrate_turnstile_secret_setting()?;
        Ok(())
    }

    fn migrate_turnstile_secret_setting(&self) -> Result<()> {
        let Some(value) = self.get_setting("turnstile_secret_key")? else {
            return Ok(());
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        match encrypted_setting_parts(trimmed) {
            Some(Ok((ciphertext, nonce))) => {
                if let Ok(plaintext) = self.crypto.decrypt(&ciphertext, &nonce) {
                    self.set_secret_setting_bytes("turnstile_secret_key", &plaintext)?;
                }
            }
            Some(Err(_)) => {}
            None => {
                self.set_secret_setting_bytes("turnstile_secret_key", trimmed.as_bytes())?;
            }
        }
        Ok(())
    }

    pub fn admin_count(&self) -> Result<u64> {
        let conn = self.conn.lock().expect("db mutex");
        Ok(conn.query_row("SELECT count(*) FROM admins", [], |row| {
            row.get::<_, u64>(0)
        })?)
    }

    pub fn upsert_admin(&self, id: &str, username: &str, password_hash: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO admins(id, username, password_hash, created_at)
            VALUES(?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET password_hash=excluded.password_hash
            "#,
            params![id, username, password_hash, Self::now()],
        )?;
        Ok(())
    }

    pub fn admin_by_username(&self, username: &str) -> Result<Option<Admin>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            "SELECT id, username, password_hash, created_at FROM admins WHERE username=?1",
            params![username],
            admin_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn admin_by_id(&self, id: &str) -> Result<Option<Admin>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            "SELECT id, username, password_hash, created_at FROM admins WHERE id=?1",
            params![id],
            admin_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn create_session(&self, session: NewSession<'_>) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO sessions(id, admin_id, token_hash, csrf_token_hash, user_agent, ip, expires_at, created_at)
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                session.id,
                session.admin_id,
                hash_token(session.token),
                hash_token(session.csrf_token),
                session.user_agent,
                session.ip,
                session.expires_at,
                Self::now()
            ],
        )?;
        Ok(())
    }

    pub fn session_by_token(&self, token: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            r#"
            SELECT id, admin_id, token_hash, csrf_token_hash, expires_at
            FROM sessions
            WHERE token_hash=?1 AND revoked_at IS NULL AND expires_at > ?2
            "#,
            params![hash_token(token), Self::now()],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    admin_id: row.get(1)?,
                    token_hash: row.get(2)?,
                    csrf_token_hash: row.get(3)?,
                    expires_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn revoke_session(&self, token: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "UPDATE sessions SET revoked_at=?2 WHERE token_hash=?1",
            params![hash_token(token), Self::now()],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            "SELECT value FROM settings WHERE key=?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_setting_with_updated_at(&self, key: &str) -> Result<Option<(String, i64)>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            "SELECT value, updated_at FROM settings WHERE key=?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at
            "#,
            params![key, value, Self::now()],
        )?;
        Ok(())
    }

    pub fn set_secret_setting_bytes(&self, key: &str, plaintext: &[u8]) -> Result<()> {
        if plaintext.is_empty() {
            return Ok(());
        }
        let (ciphertext, nonce) = self.crypto.encrypt(plaintext)?;
        let value = serde_json::json!({
            "ciphertext": general_purpose::STANDARD.encode(ciphertext),
            "nonce": general_purpose::STANDARD.encode(nonce)
        })
        .to_string();
        self.set_setting(key, &value)
    }

    pub fn get_secret_setting_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let Some(value) = self.get_setting(key)? else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        match encrypted_setting_parts(trimmed) {
            Some(Ok((ciphertext, nonce))) => self.crypto.decrypt(&ciphertext, &nonce).map(Some),
            Some(Err(err)) => Err(err),
            None => Ok(Some(trimmed.as_bytes().to_vec())),
        }
    }

    pub fn set_turnstile_secret(&self, value: &str) -> Result<()> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        self.set_secret_setting_bytes("turnstile_secret_key", trimmed.as_bytes())
    }

    pub fn security_settings(&self, default_ttl: u64) -> Result<SecuritySettings> {
        Ok(SecuritySettings {
            turnstile_enabled: setting_bool(self.get_setting("turnstile_enabled")?, false),
            turnstile_required: setting_bool(self.get_setting("turnstile_required")?, false),
            turnstile_site_key: self.get_setting("turnstile_site_key")?,
            turnstile_secret_configured: self
                .turnstile_secret()
                .map(|value| value.is_some_and(|secret| !secret.trim().is_empty()))
                .unwrap_or(false),
            session_ttl_seconds: self
                .get_setting("session_ttl_seconds")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_ttl),
        })
    }

    pub fn turnstile_secret(&self) -> Result<Option<String>> {
        self.get_secret_setting_bytes("turnstile_secret_key")?
            .map(|value| String::from_utf8(value).context("invalid Turnstile secret"))
            .transpose()
    }

    pub fn turnstile_token_seen(&self, token: &str) -> Result<bool> {
        self.prune_expired_turnstile_attempts()?;
        let conn = self.conn.lock().expect("db mutex");
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM turnstile_attempts WHERE token_hash=?1 AND expires_at>?2",
            params![hash_token(token), Self::now()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn record_turnstile_attempt(
        &self,
        token: &str,
        action: &str,
        hostname: Option<&str>,
        remote_ip: Option<&str>,
        success: bool,
        error_codes: &[String],
    ) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        let now = Self::now();
        conn.execute(
            r#"
            INSERT OR IGNORE INTO turnstile_attempts
              (token_hash, action, hostname, remote_ip, success, error_codes, created_at, expires_at)
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                hash_token(token),
                action,
                hostname,
                remote_ip,
                if success { 1 } else { 0 },
                serde_json::to_string(error_codes)?,
                now,
                now + 600,
            ],
        )?;
        Ok(())
    }

    fn prune_expired_turnstile_attempts(&self) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "DELETE FROM turnstile_attempts WHERE expires_at <= ?1",
            params![Self::now()],
        )?;
        Ok(())
    }

    pub fn record_audit(
        &self,
        admin_id: Option<&str>,
        action: &str,
        target_type: Option<&str>,
        target_id: Option<&str>,
        ip: Option<&str>,
        detail: Value,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO audit_log(admin_id, action, target_type, target_id, ip, detail_json, created_at)
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                admin_id,
                action,
                target_type,
                target_id,
                ip,
                detail.to_string(),
                Self::now()
            ],
        )?;
        Ok(())
    }

    pub fn create_job(&self, id: &str, kind: &str, title: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "INSERT INTO jobs(id, kind, status, title, started_at) VALUES(?1, ?2, 'running', ?3, ?4)",
            params![id, kind, title, Self::now()],
        )?;
        Ok(())
    }

    pub fn link_job_thread(
        &self,
        id: &str,
        thread_id: Option<&str>,
        turn_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "UPDATE jobs SET thread_id=?2, turn_id=?3 WHERE id=?1",
            params![id, thread_id, turn_id],
        )?;
        Ok(())
    }

    pub fn append_job_output(&self, id: &str, chunk: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "UPDATE jobs SET output = output || ?2 WHERE id=?1",
            params![id, chunk],
        )?;
        Ok(())
    }

    pub fn finish_job(
        &self,
        id: &str,
        status: &str,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "UPDATE jobs SET status=?2, finished_at=?3, exit_code=?4, error=?5 WHERE id=?1",
            params![id, status, Self::now(), exit_code, error],
        )?;
        Ok(())
    }

    pub fn list_jobs(&self, limit: u32) -> Result<Vec<JobRecord>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare(
            "SELECT id, kind, status, title, thread_id, turn_id, started_at, finished_at, exit_code, substr(output, 1, 24000), error FROM jobs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], job_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn running_thread_jobs(&self) -> Result<Vec<JobRecord>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare(
            r#"
            SELECT id, kind, status, title, thread_id, turn_id, started_at, finished_at, exit_code,
                   substr(output, 1, 24000), error
            FROM jobs
            WHERE status='running' AND thread_id IS NOT NULL
            ORDER BY started_at DESC
            "#,
        )?;
        let rows = stmt.query_map([], job_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn running_job_for_thread(&self, thread_id: &str) -> Result<Option<JobRecord>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            r#"
            SELECT id, kind, status, title, thread_id, turn_id, started_at, finished_at, exit_code,
                   output, error
            FROM jobs
            WHERE thread_id=?1 AND status='running'
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            params![thread_id],
            job_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn job(&self, id: &str) -> Result<Option<JobRecord>> {
        let conn = self.conn.lock().expect("db mutex");
        conn.query_row(
            "SELECT id, kind, status, title, thread_id, turn_id, started_at, finished_at, exit_code, output, error FROM jobs WHERE id=?1",
            params![id],
            job_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn enqueue_followup(
        &self,
        thread_id: &str,
        message: &str,
        options: Value,
    ) -> Result<ThreadFollowUp> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        let options_json = options.to_string();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO thread_followups(id, thread_id, status, message, options_json, created_at, updated_at)
            VALUES(?1, ?2, 'pending', ?3, ?4, ?5, ?5)
            "#,
            params![id, thread_id, message, options_json, now],
        )?;
        Ok(ThreadFollowUp {
            id,
            thread_id: thread_id.to_string(),
            status: "pending".to_string(),
            message: message.to_string(),
            options_json,
            created_at: now,
            updated_at: now,
            submitted_at: None,
            cancelled_at: None,
            result_json: None,
            error: None,
        })
    }

    pub fn list_followups(&self, thread_id: &str, limit: u32) -> Result<Vec<ThreadFollowUp>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, status, message, options_json, created_at, updated_at,
                   submitted_at, cancelled_at, result_json, error
            FROM thread_followups
            WHERE thread_id=?1
            ORDER BY created_at DESC, rowid DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![thread_id, limit.max(1)], followup_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn claim_next_pending_followup(&self, thread_id: &str) -> Result<Option<ThreadFollowUp>> {
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        let followup = conn
            .query_row(
                r#"
                SELECT id, thread_id, status, message, options_json, created_at, updated_at,
                       submitted_at, cancelled_at, result_json, error
                FROM thread_followups
                WHERE thread_id=?1 AND status='pending'
                ORDER BY created_at ASC, rowid ASC
                LIMIT 1
                "#,
                params![thread_id],
                followup_from_row,
            )
            .optional()?;
        let Some(followup) = followup else {
            return Ok(None);
        };
        let changed = conn.execute(
            "UPDATE thread_followups SET status='submitting', updated_at=?2 WHERE id=?1 AND status='pending'",
            params![followup.id, now],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        Ok(Some(ThreadFollowUp {
            status: "submitting".to_string(),
            updated_at: now,
            ..followup
        }))
    }

    pub fn active_followup_upload_ids(&self) -> Result<HashSet<String>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare(
            r#"
            SELECT options_json
            FROM thread_followups
            WHERE status IN ('pending', 'submitting')
            "#,
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = HashSet::new();
        for row in rows {
            let options_json = row?;
            let Ok(options) = serde_json::from_str::<Value>(&options_json) else {
                continue;
            };
            if let Some(attachments) = options.get("attachments").and_then(Value::as_array) {
                ids.extend(
                    attachments
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|id| Uuid::parse_str(id).is_ok())
                        .map(str::to_string),
                );
            }
        }
        Ok(ids)
    }

    pub fn cancel_followup(&self, thread_id: &str, id: &str) -> Result<bool> {
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        let changed = conn.execute(
            r#"
            UPDATE thread_followups
            SET status='cancelled', cancelled_at=?3, updated_at=?3
            WHERE thread_id=?1 AND id=?2 AND status='pending'
            "#,
            params![thread_id, id, now],
        )?;
        Ok(changed > 0)
    }

    pub fn mark_followup_submitted(&self, id: &str, result: Value) -> Result<()> {
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            UPDATE thread_followups
            SET status='submitted', submitted_at=?2, updated_at=?2, result_json=?3, error=NULL
            WHERE id=?1
            "#,
            params![id, now, result.to_string()],
        )?;
        Ok(())
    }

    pub fn mark_followup_error(&self, id: &str, error: &str) -> Result<()> {
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            UPDATE thread_followups
            SET status='error', updated_at=?2, error=?3
            WHERE id=?1
            "#,
            params![id, now, error],
        )?;
        Ok(())
    }

    pub fn record_probe_event(&self, event: NewProbeEvent<'_>) -> Result<ProbeEvent> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        let payload_json = event.payload.to_string();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            r#"
            INSERT INTO probe_events(
              id, kind, thread_id, title, message, dedupe_key, source, payload_json, created_at
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                id,
                event.kind,
                event.thread_id,
                event.title,
                event.message,
                event.dedupe_key,
                event.source,
                payload_json,
                now,
            ],
        )?;
        Ok(ProbeEvent {
            id,
            kind: event.kind.to_string(),
            thread_id: event.thread_id.map(str::to_string),
            title: event.title.map(str::to_string),
            message: event.message.map(str::to_string),
            dedupe_key: event.dedupe_key.map(str::to_string),
            source: event.source.to_string(),
            payload: event.payload,
            created_at: now,
            handled_at: None,
        })
    }

    pub fn list_probe_events(&self, limit: u32) -> Result<Vec<ProbeEvent>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare(
            r#"
            SELECT id, kind, thread_id, title, message, dedupe_key, source, payload_json, created_at, handled_at
            FROM probe_events
            ORDER BY created_at DESC, rowid DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit.clamp(1, 500)], probe_event_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn mark_probe_event_handled(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("db mutex");
        let changed = conn.execute(
            "UPDATE probe_events SET handled_at=?2 WHERE id=?1 AND handled_at IS NULL",
            params![id, Self::now()],
        )?;
        Ok(changed > 0)
    }

    pub fn claim_probe_dedupe(
        &self,
        namespace: &str,
        dedupe_key: &str,
        ttl_seconds: i64,
    ) -> Result<bool> {
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute(
            "DELETE FROM probe_dedupe WHERE expires_at <= ?1",
            params![now],
        )?;
        let changed = conn.execute(
            r#"
            INSERT OR IGNORE INTO probe_dedupe(namespace, dedupe_key, expires_at, created_at)
            VALUES(?1, ?2, ?3, ?4)
            "#,
            params![namespace, dedupe_key, now + ttl_seconds.max(1), now],
        )?;
        Ok(changed > 0)
    }

    pub fn probe_logs_db_counts(&self, retention_days: u32) -> Result<ProbeLogsDbCounts> {
        let cutoff = Self::now() - i64::from(retention_days.max(1)) * 86_400;
        let now = Self::now();
        let conn = self.conn.lock().expect("db mutex");
        let event_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM probe_events", [], |row| row.get(0))?;
        let dedupe_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM probe_dedupe", [], |row| row.get(0))?;
        let pending_event_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM probe_events WHERE created_at < ?1",
            params![cutoff],
            |row| row.get(0),
        )?;
        let pending_dedupe_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM probe_dedupe WHERE expires_at <= ?1",
            params![now],
            |row| row.get(0),
        )?;
        Ok(ProbeLogsDbCounts {
            event_count: event_count as usize,
            dedupe_count: dedupe_count as usize,
            pending_event_count: pending_event_count as usize,
            pending_dedupe_count: pending_dedupe_count as usize,
        })
    }

    pub fn maintain_probe_events(
        &self,
        retention_days: u32,
        max_delete_rows: u32,
        dry_run: bool,
    ) -> Result<(usize, usize)> {
        let cutoff = Self::now() - i64::from(retention_days.max(1)) * 86_400;
        let limit = i64::from(max_delete_rows.max(1));
        let conn = self.conn.lock().expect("db mutex");
        let event_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT rowid FROM probe_events WHERE created_at < ?1 ORDER BY created_at ASC LIMIT ?2
            )",
            params![cutoff, limit],
            |row| row.get(0),
        )?;
        let dedupe_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT rowid FROM probe_dedupe WHERE expires_at <= ?1 ORDER BY expires_at ASC LIMIT ?2
            )",
            params![Self::now(), limit],
            |row| row.get(0),
        )?;
        if dry_run {
            return Ok((event_count as usize, dedupe_count as usize));
        }
        let events_deleted = conn.execute(
            "DELETE FROM probe_events WHERE rowid IN (
                SELECT rowid FROM probe_events WHERE created_at < ?1 ORDER BY created_at ASC LIMIT ?2
            )",
            params![cutoff, limit],
        )?;
        let dedupe_deleted = conn.execute(
            "DELETE FROM probe_dedupe WHERE rowid IN (
                SELECT rowid FROM probe_dedupe WHERE expires_at <= ?1 ORDER BY expires_at ASC LIMIT ?2
            )",
            params![Self::now(), limit],
        )?;
        Ok((events_deleted, dedupe_deleted))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: Option<String>,
}

fn admin_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Admin> {
    Ok(Admin {
        id: row.get(0)?,
        username: row.get(1)?,
        password_hash: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
    Ok(JobRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        title: row.get(3)?,
        thread_id: row.get(4)?,
        turn_id: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        exit_code: row.get(8)?,
        output: row.get(9)?,
        error: row.get(10)?,
    })
}

fn followup_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadFollowUp> {
    Ok(ThreadFollowUp {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        status: row.get(2)?,
        message: row.get(3)?,
        options_json: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        submitted_at: row.get(7)?,
        cancelled_at: row.get(8)?,
        result_json: row.get(9)?,
        error: row.get(10)?,
    })
}

fn probe_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProbeEvent> {
    let payload_json: String = row.get(7)?;
    let payload = serde_json::from_str(&payload_json).unwrap_or(Value::Null);
    Ok(ProbeEvent {
        id: row.get(0)?,
        kind: row.get(1)?,
        thread_id: row.get(2)?,
        title: row.get(3)?,
        message: row.get(4)?,
        dedupe_key: row.get(5)?,
        source: row.get(6)?,
        payload,
        created_at: row.get(8)?,
        handled_at: row.get(9)?,
    })
}

fn setting_bool(value: Option<String>, default: bool) -> bool {
    value
        .as_deref()
        .map(|v| matches!(v, "true" | "1" | "yes" | "on"))
        .unwrap_or(default)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    sql_type: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}"),
            [],
        )?;
    }
    Ok(())
}

fn encrypted_setting_parts(value: &str) -> Option<Result<(Vec<u8>, Vec<u8>)>> {
    let Ok(json) = serde_json::from_str::<Value>(value) else {
        return None;
    };
    let Value::Object(map) = json else {
        return None;
    };
    if !(map.contains_key("ciphertext") || map.contains_key("nonce")) {
        return None;
    }
    let result = (|| {
        let ciphertext = map
            .get("ciphertext")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("invalid encrypted setting ciphertext"))?;
        let nonce = map
            .get("nonce")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("invalid encrypted setting nonce"))?;
        let ciphertext = general_purpose::STANDARD
            .decode(ciphertext)
            .context("invalid encrypted setting ciphertext")?;
        let nonce = general_purpose::STANDARD
            .decode(nonce)
            .context("invalid encrypted setting nonce")?;
        Ok((ciphertext, nonce))
    })();
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::PanelDb;
    use serde_json::json;

    #[test]
    fn creates_admin_and_session() {
        let db = PanelDb::open(":memory:").unwrap();
        db.upsert_admin("a1", "admin", "hash").unwrap();
        assert_eq!(db.admin_count().unwrap(), 1);
        db.create_session(super::NewSession {
            id: "s1",
            admin_id: "a1",
            token: "token",
            csrf_token: "csrf",
            user_agent: None,
            ip: None,
            expires_at: PanelDb::now() + 60,
        })
        .unwrap();
        assert!(db.session_by_token("token").unwrap().is_some());
    }

    #[test]
    fn migrate_replaces_legacy_default_session_ttl() {
        let db = PanelDb::open(":memory:").unwrap();
        db.set_setting("session_ttl_seconds", "604800").unwrap();

        db.migrate().unwrap();

        assert_eq!(
            db.security_settings(300).unwrap().session_ttl_seconds,
            31_536_000
        );
    }

    #[test]
    fn turnstile_attempt_prevents_replay_and_hashes_token() {
        let db = PanelDb::open(":memory:").unwrap();

        assert!(!db.turnstile_token_seen("token-1").unwrap());
        db.record_turnstile_attempt("token-1", "login", Some("661313.xyz"), None, true, &[])
            .unwrap();

        assert!(db.turnstile_token_seen("token-1").unwrap());
        assert_ne!(
            db.get_setting("token-1").unwrap(),
            Some("token-1".to_string())
        );
    }

    #[test]
    fn turnstile_secret_is_encrypted_and_blank_update_preserves_existing() {
        let db = PanelDb::open(":memory:").unwrap();

        db.set_turnstile_secret("secret-one").unwrap();
        let stored = db.get_setting("turnstile_secret_key").unwrap().unwrap();

        assert_ne!(stored, "secret-one");
        assert!(
            db.security_settings(300)
                .unwrap()
                .turnstile_secret_configured
        );
        assert_eq!(
            db.turnstile_secret().unwrap().as_deref(),
            Some("secret-one")
        );

        db.set_turnstile_secret("   ").unwrap();

        assert_eq!(
            db.turnstile_secret().unwrap().as_deref(),
            Some("secret-one")
        );
    }

    #[test]
    fn turnstile_secret_does_not_return_encrypted_json_as_plaintext() {
        let db = PanelDb::open(":memory:").unwrap();
        db.set_setting(
            "turnstile_secret_key",
            r#"{"ciphertext":"not-base64","nonce":"also-bad"}"#,
        )
        .unwrap();

        let err = db.turnstile_secret().unwrap_err().to_string();

        assert!(err.contains("decrypt") || err.contains("invalid encrypted"));
    }

    #[test]
    fn followup_queue_persists_claims_cancels_and_marks_results() {
        let db = PanelDb::open(":memory:").unwrap();
        let first = db
            .enqueue_followup("thread-a", "first follow-up", json!({"model":"gpt-5.5"}))
            .unwrap();
        let second = db
            .enqueue_followup(
                "thread-a",
                "second follow-up",
                json!({"reasoning_effort":"xhigh"}),
            )
            .unwrap();

        let listed = db.list_followups("thread-a", 10).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id);
        assert_eq!(listed[1].id, first.id);

        let claimed = db.claim_next_pending_followup("thread-a").unwrap().unwrap();
        assert_eq!(claimed.id, first.id);
        assert_eq!(claimed.status, "submitting");
        assert!(!db.cancel_followup("thread-a", &first.id).unwrap());
        assert!(db.cancel_followup("thread-a", &second.id).unwrap());

        db.mark_followup_submitted(&first.id, json!({"turn_id":"turn-next"}))
            .unwrap();
        let listed = db.list_followups("thread-a", 10).unwrap();
        let submitted = listed.iter().find(|item| item.id == first.id).unwrap();
        let cancelled = listed.iter().find(|item| item.id == second.id).unwrap();
        assert_eq!(submitted.status, "submitted");
        assert_eq!(
            submitted.result_json.as_deref(),
            Some("{\"turn_id\":\"turn-next\"}")
        );
        assert_eq!(cancelled.status, "cancelled");
    }

    #[test]
    fn active_followup_upload_ids_only_include_pending_or_submitting_items() {
        let db = PanelDb::open(":memory:").unwrap();
        let pending = db
            .enqueue_followup(
                "thread-a",
                "pending",
                json!({"attachments":["00000000-0000-0000-0000-000000000001", "not-a-uuid"]}),
            )
            .unwrap();
        let submitting = db
            .enqueue_followup(
                "thread-a",
                "submitting",
                json!({"attachments":["00000000-0000-0000-0000-000000000002"]}),
            )
            .unwrap();
        let submitted = db
            .enqueue_followup(
                "thread-a",
                "submitted",
                json!({"attachments":["00000000-0000-0000-0000-000000000003"]}),
            )
            .unwrap();

        let claimed = db.claim_next_pending_followup("thread-a").unwrap().unwrap();
        assert_eq!(claimed.id, pending.id);
        db.mark_followup_submitted(&submitted.id, json!({"ok":true}))
            .unwrap();

        let ids = db.active_followup_upload_ids().unwrap();

        assert!(ids.contains("00000000-0000-0000-0000-000000000001"));
        assert!(ids.contains("00000000-0000-0000-0000-000000000002"));
        assert!(!ids.contains("00000000-0000-0000-0000-000000000003"));
        assert!(!ids.contains("not-a-uuid"));
        assert_eq!(submitting.status, "pending");
    }

    #[test]
    fn followup_error_records_message_and_is_not_claimed_again() {
        let db = PanelDb::open(":memory:").unwrap();
        let followup = db
            .enqueue_followup("thread-a", "retry later", json!({}))
            .unwrap();
        let claimed = db.claim_next_pending_followup("thread-a").unwrap().unwrap();
        db.mark_followup_error(&claimed.id, "bridge unavailable")
            .unwrap();

        assert!(db
            .claim_next_pending_followup("thread-a")
            .unwrap()
            .is_none());
        let listed = db.list_followups("thread-a", 10).unwrap();
        let errored = listed.iter().find(|item| item.id == followup.id).unwrap();
        assert_eq!(errored.status, "error");
        assert_eq!(errored.error.as_deref(), Some("bridge unavailable"));
    }

    #[test]
    fn probe_events_and_dedupe_persist_recent_runtime_state() {
        let db = PanelDb::open(":memory:").unwrap();

        let first_claim = db
            .claim_probe_dedupe("reply_needed", "thread-a:turn-1", 60)
            .unwrap();
        let duplicate_claim = db
            .claim_probe_dedupe("reply_needed", "thread-a:turn-1", 60)
            .unwrap();
        assert!(first_claim);
        assert!(!duplicate_claim);

        let event = db
            .record_probe_event(super::NewProbeEvent {
                kind: "reply-needed",
                thread_id: Some("thread-a"),
                title: Some("等待确认"),
                message: Some("等待用户选择"),
                dedupe_key: Some("thread-a:turn-1"),
                source: "hook-stop",
                payload: json!({"turn_id":"turn-1"}),
            })
            .unwrap();

        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
        assert_eq!(events[0].kind, "reply-needed");
        assert_eq!(events[0].thread_id.as_deref(), Some("thread-a"));
        assert_eq!(events[0].payload["turn_id"], "turn-1");
        assert!(events[0].handled_at.is_none());

        assert!(db.mark_probe_event_handled(&event.id).unwrap());
        let handled = db.list_probe_events(10).unwrap();
        assert!(handled[0].handled_at.is_some());
    }

    #[test]
    fn maintain_probe_events_deletes_old_events_and_expired_dedupe_with_limit() {
        let db = PanelDb::open(":memory:").unwrap();
        let now = PanelDb::now();
        {
            let conn = db.conn.lock().expect("db mutex");
            for index in 0..3 {
                conn.execute(
                    r#"
                    INSERT INTO probe_events(
                      id, kind, thread_id, title, message, dedupe_key, source, payload_json, created_at
                    )
                    VALUES(?1, 'hook-stop', 'thread-a', 'old', 'old', ?1, 'test', '{}', ?2)
                    "#,
                    rusqlite::params![format!("old-event-{index}"), now - 172_900],
                )
                .unwrap();
                conn.execute(
                    r#"
                    INSERT INTO probe_dedupe(namespace, dedupe_key, expires_at, created_at)
                    VALUES('probe_event', ?1, ?2, ?3)
                    "#,
                    rusqlite::params![format!("old-dedupe-{index}"), now - 1, now - 172_900],
                )
                .unwrap();
            }
            conn.execute(
                r#"
                INSERT INTO probe_events(
                  id, kind, thread_id, title, message, dedupe_key, source, payload_json, created_at
                )
                VALUES('fresh-event', 'hook-stop', 'thread-a', 'fresh', 'fresh', 'fresh-event', 'test', '{}', ?1)
                "#,
                rusqlite::params![now],
            )
            .unwrap();
            conn.execute(
                r#"
                INSERT INTO probe_dedupe(namespace, dedupe_key, expires_at, created_at)
                VALUES('probe_event', 'fresh-dedupe', ?1, ?2)
                "#,
                rusqlite::params![now + 300, now],
            )
            .unwrap();
        }

        let dry_run = db.maintain_probe_events(1, 2, true).unwrap();
        assert_eq!(dry_run, (2, 2));

        let deleted = db.maintain_probe_events(1, 2, false).unwrap();
        assert_eq!(deleted, (2, 2));

        let counts = db.probe_logs_db_counts(1).unwrap();
        assert_eq!(counts.event_count, 2);
        assert_eq!(counts.dedupe_count, 2);
        assert_eq!(counts.pending_event_count, 1);
        assert_eq!(counts.pending_dedupe_count, 1);

        let deleted = db.maintain_probe_events(1, 10, false).unwrap();
        assert_eq!(deleted, (1, 1));
        let counts = db.probe_logs_db_counts(1).unwrap();
        assert_eq!(counts.event_count, 1);
        assert_eq!(counts.dedupe_count, 1);
        assert_eq!(counts.pending_event_count, 0);
        assert_eq!(counts.pending_dedupe_count, 0);
    }
}
