use super::*;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode},
    response::Response,
    Router,
};
use nexushub_core::{
    db::NewProbeEvent,
    native_probe::{NativeProvider, NativeTurnEvent},
    platform::PlatformKind,
};
use rusqlite::{params, Connection};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const TOKEN: &str = "fixture-gotify-application-token";
const BODY: &str = "这是虚构完成正文。\n中文、换行与标点应完整到达。";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct Reply {
    status: StatusCode,
    body: Vec<u8>,
    location: Option<String>,
    delay: Duration,
}

impl Reply {
    fn json(status: StatusCode, value: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&value).unwrap(),
            location: None,
            delay: Duration::ZERO,
        }
    }

    fn bark() -> Self {
        Self::json(StatusCode::OK, json!({"code":200}))
    }

    fn gotify() -> Self {
        Self::json(StatusCode::OK, json!({"id":17}))
    }
}

#[derive(Clone)]
struct CapturedRequest {
    method: Method,
    uri: String,
    headers: HeaderMap,
    body: Value,
}

struct ServerState {
    reply: Mutex<Reply>,
    requests: Mutex<Vec<CapturedRequest>>,
}

struct LoopbackServer {
    url: String,
    state: Arc<ServerState>,
    task: tokio::task::JoinHandle<()>,
}

async fn capture_request(
    State(state): State<Arc<ServerState>>,
    request: Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 2 * 1024 * 1024).await.unwrap();
    state.requests.lock().unwrap().push(CapturedRequest {
        method: parts.method,
        uri: parts.uri.to_string(),
        headers: parts.headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    });
    let reply = state.reply.lock().unwrap().clone();
    if !reply.delay.is_zero() {
        tokio::time::sleep(reply.delay).await;
    }
    let mut response = Response::builder()
        .status(reply.status)
        .header("Content-Type", "application/json");
    if let Some(location) = reply.location {
        response = response.header("Location", location);
    }
    response.body(Body::from(reply.body)).unwrap()
}

impl LoopbackServer {
    async fn new(reply: Reply) -> Self {
        let state = Arc::new(ServerState {
            reply: Mutex::new(reply),
            requests: Mutex::new(Vec::new()),
        });
        let router = Router::new()
            .fallback(capture_request)
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { url, state, task }
    }

    fn reply(&self, reply: Reply) {
        *self.state.reply.lock().unwrap() = reply;
    }

    fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }

    fn count(&self) -> usize {
        self.state.requests.lock().unwrap().len()
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Fixture {
    root: PathBuf,
    rollout: PathBuf,
    config: Config,
    db: PanelDb,
    bark: LoopbackServer,
    gotify: LoopbackServer,
}

impl Fixture {
    async fn new() -> Self {
        let bark = LoopbackServer::new(Reply::bark()).await;
        let gotify = LoopbackServer::new(Reply::gotify()).await;
        let root = std::env::temp_dir().join(format!(
            "nexushub-channel-delivery-{}",
            uuid::Uuid::new_v4()
        ));
        let home = root.join("codex");
        fs::create_dir_all(home.join("sessions")).unwrap();
        let rollout = home.join("sessions/rollout.jsonl");
        let records = [
            json!({"type":"session_meta","payload":{"id":"fixture-main"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"fixture-turn"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":BODY}]}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"fixture-turn","status":"completed"}}),
        ];
        fs::write(
            &rollout,
            records
                .iter()
                .map(|value| format!("{value}\n"))
                .collect::<String>(),
        )
        .unwrap();
        fs::write(home.join("session_index.jsonl"), "").unwrap();
        let connection = Connection::open(home.join("state_5.sqlite")).unwrap();
        connection.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY, name TEXT, title TEXT, source TEXT, thread_source TEXT,
                agent_path TEXT, rollout_path TEXT, updated_at INTEGER, archived_at INTEGER,
                archived INTEGER, cwd TEXT, model TEXT, has_user_event INTEGER, first_user_message TEXT
             );",
        ).unwrap();
        connection.execute(
            "INSERT INTO threads VALUES ('fixture-main','Fixture task','Fixture task','vscode','user',NULL,?1,?2,NULL,0,'/workspace/fixture','fixture',1,'fixture question')",
            params![rollout.to_str().unwrap(), PanelDb::now()],
        ).unwrap();
        drop(connection);
        let mut config = Config::for_platform_kind_with_home(PlatformKind::Linux, &root);
        config.codex.home = home;
        config.codex.workspace = root.join("workspace");
        config.probe.enabled = true;
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_codex = true;
        config.probe.notifications.notify_completion = true;
        config.probe.notifications.notify_grok = true;
        config.probe.notifications.notify_grok_completion = true;
        config.probe.notifications.notify_claude = true;
        config.probe.notifications.notify_claude_reply_needed = true;
        config.probe.notifications.server_url = bark.url.clone();
        config.probe.notifications.gotify.enabled = true;
        config.probe.notifications.gotify.server_url = format!("{}/gotify", gotify.url);
        config.probe.notifications.gotify.priority = 7;
        let db = PanelDb::open(root.join("panel.sqlite")).unwrap();
        db.set_secret_setting_bytes(PROBE_BARK_DEVICE_KEY_SETTING, b"fixture-bark-key")
            .unwrap();
        db.set_secret_setting_bytes(PROBE_GOTIFY_TOKEN_SETTING, TOKEN.as_bytes())
            .unwrap();
        db.sync_notification_channels(&config).unwrap();
        Self {
            root,
            rollout,
            config,
            db,
            bark,
            gotify,
        }
    }

    fn connection(&self) -> Connection {
        Connection::open(self.db.path()).unwrap()
    }

    fn event_ms(&self) -> i64 {
        self.connection()
            .query_row(
                "SELECT activated_ms + 1 FROM probe_notification_channels WHERE channel='gotify'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn request(&self) -> ProbeBarkRequest {
        ProbeBarkRequest {
            title: "完成 · 中文 fixture".into(),
            body: BODY.into(),
            dedupe_key: "fixture-event".into(),
        }
    }

    fn event(&self) -> ProbeBuiltEvent {
        ProbeBuiltEvent {
            kind: "completion".into(),
            event_type: "task_complete".into(),
            thread_id: Some("fixture-main".into()),
            turn_id: Some("fixture-turn".into()),
            title: "Fixture task".into(),
            message: "Fixture completion".into(),
            bark_title: "完成 · 中文 fixture".into(),
            bark_body: BODY.into(),
            dedupe_namespace: "fixture-completion".into(),
            dedupe_key: "fixture-event".into(),
            ttl_seconds: 3_600,
            source: "fixture-hook".into(),
            payload: json!({"body_source":"assistant_final","feedback_completed_at_ms":self.event_ms()}),
            suppression_reason: None,
        }
    }

    fn native_delivery(&self, provider: NativeProvider, kind: &str) -> NativeDelivery {
        NativeDelivery {
            event_key: format!("fixture-{}-{kind}", provider.as_str()),
            provider,
            session_key: "fixture-native-session".into(),
            thread_id: "fixture-native-thread".into(),
            title: "原生测试线程".into(),
            event: NativeTurnEvent {
                position: 3,
                turn_id: "fixture-native-turn".into(),
                kind: kind.into(),
                body: BODY.into(),
                timestamp_ms: self.event_ms(),
            },
        }
    }

    fn due_now(&self, key: &str) {
        self.connection()
            .execute(
                "UPDATE probe_notification_deliveries SET next_ms=0 WHERE event_key=?1",
                [key],
            )
            .unwrap();
    }

    fn status(&self, channel: NotificationChannel, key: &str) -> String {
        self.connection().query_row(
            "SELECT status FROM probe_notification_deliveries WHERE channel=?1 AND event_key=?2",
            params![channel.as_str(), key],
            |row| row.get(0),
        ).unwrap()
    }

    fn assert_payload_cleared(&self, channel: NotificationChannel, key: &str) {
        let cleared = self.connection().query_row(
            "SELECT ciphertext IS NULL AND nonce IS NULL FROM probe_notification_deliveries WHERE channel=?1 AND event_key=?2",
            params![channel.as_str(), key],
            |row| row.get::<_, bool>(0),
        ).unwrap();
        assert!(cleared);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn gotify_posts_the_header_subpath_and_complete_chinese_body_with_priority() {
    let fixture = Fixture::new().await;
    fixture.gotify.reply(Reply::json(
        StatusCode::OK,
        json!({"id":17,"message":BODY,"echoed_token":TOKEN}),
    ));
    let request = fixture.request();
    let outcome = send_gotify(&fixture.config, TOKEN.as_bytes(), &request, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(outcome.sent);
    assert!(!outcome.skipped);
    assert_eq!(outcome.http_status, Some(200));
    assert_eq!(outcome.request_count, 1);
    assert_eq!(outcome.dedupe_key.as_deref(), Some("fixture-event"));
    let requests = fixture.gotify.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::POST);
    assert_eq!(requests[0].uri, "/gotify/message");
    assert_eq!(requests[0].headers["x-gotify-key"].to_str().unwrap(), TOKEN);
    assert!(!requests[0].headers.contains_key("authorization"));
    assert_eq!(requests[0].body["message"], BODY);
    assert_eq!(requests[0].body["priority"], 7);
    assert!(requests[0].body["title"]
        .as_str()
        .unwrap()
        .ends_with(&request.title));
    assert_eq!(requests[0].body.as_object().unwrap().len(), 3);
    assert!(!requests[0].uri.contains(TOKEN));
    assert!(!requests[0].body.to_string().contains(TOKEN));
    let serialized = serde_json::to_string(&outcome).unwrap();
    assert!(!serialized.contains(TOKEN));
    assert!(!serialized.contains(BODY));
}

#[tokio::test]
async fn gotify_never_follows_redirects_or_sends_the_key_to_the_redirect_target() {
    let fixture = Fixture::new().await;
    let target = LoopbackServer::new(Reply::gotify()).await;
    let mut redirect = Reply::json(StatusCode::TEMPORARY_REDIRECT, json!({"id":17}));
    redirect.location = Some(format!("{}/redirected-message", target.url));
    fixture.gotify.reply(redirect);
    let outcome = send_gotify(
        &fixture.config,
        TOKEN.as_bytes(),
        &fixture.request(),
        REQUEST_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(!outcome.sent && !outcome.skipped);
    assert_eq!(outcome.http_status, Some(307));
    assert_eq!(outcome.request_count, 1);
    assert_eq!(fixture.gotify.count(), 1);
    assert_eq!(target.count(), 0);
    assert!(!serde_json::to_string(&outcome).unwrap().contains(TOKEN));
}

#[tokio::test]
async fn gotify_rejects_invalid_or_oversized_acknowledgements_instead_of_claiming_success() {
    let fixture = Fixture::new().await;
    for body in [
        b"not-json".to_vec(),
        b"{}".to_vec(),
        br#"{"id":"17"}"#.to_vec(),
        br#"{"id":-1}"#.to_vec(),
        serde_json::to_vec(&json!({"id":17,"padding":"x".repeat(65_536)})).unwrap(),
    ] {
        let mut reply = Reply::gotify();
        reply.body = body;
        fixture.gotify.reply(reply);
        let outcome = send_gotify(
            &fixture.config,
            TOKEN.as_bytes(),
            &fixture.request(),
            REQUEST_TIMEOUT,
        )
        .await
        .unwrap();
        assert!(!outcome.sent && !outcome.skipped);
        assert_eq!(outcome.reason.as_deref(), Some("response_invalid"));
        assert_eq!(outcome.request_count, 1);
    }
    assert_eq!(fixture.gotify.count(), 5);
}

#[tokio::test]
async fn gotify_http_failures_never_become_success_even_with_a_success_shaped_body() {
    let fixture = Fixture::new().await;
    for status in [401, 429, 500, 501, 503, 599] {
        fixture.gotify.reply(Reply::json(
            StatusCode::from_u16(status).unwrap(),
            json!({"id":17,"error":TOKEN}),
        ));
        let outcome = send_gotify(
            &fixture.config,
            TOKEN.as_bytes(),
            &fixture.request(),
            REQUEST_TIMEOUT,
        )
        .await
        .unwrap();
        assert!(!outcome.sent && !outcome.skipped);
        assert_eq!(outcome.http_status, Some(status));
        assert_eq!(outcome.request_count, 1);
        assert!(!serde_json::to_string(&outcome).unwrap().contains(TOKEN));
    }
    assert_eq!(fixture.gotify.count(), 6);
}

#[tokio::test]
async fn invalid_gotify_configuration_and_token_encoding_fail_before_any_request() {
    let mut fixture = Fixture::new().await;
    for url in [
        "http://notify.example.invalid/gotify".to_string(),
        format!("{}/gotify?token=fixture", fixture.gotify.url),
        format!("{}/gotify#fragment", fixture.gotify.url),
        fixture
            .gotify
            .url
            .replace("http://", "http://test:fixture-password@"),
    ] {
        fixture.config.probe.notifications.gotify.server_url = url;
        let outcome = send_gotify(
            &fixture.config,
            TOKEN.as_bytes(),
            &fixture.request(),
            REQUEST_TIMEOUT,
        )
        .await
        .unwrap();
        assert!(!outcome.sent && !outcome.skipped);
        assert_eq!(outcome.reason.as_deref(), Some("invalid_server_url"));
        assert_eq!(outcome.request_count, 0);
    }
    fixture.config.probe.notifications.gotify.server_url = format!("{}/gotify", fixture.gotify.url);
    let outcome = send_gotify(
        &fixture.config,
        &[0xff],
        &fixture.request(),
        REQUEST_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(!outcome.sent);
    assert_eq!(outcome.reason.as_deref(), Some("invalid_token"));
    assert_eq!(outcome.request_count, 0);
    assert_eq!(fixture.gotify.count(), 0);
}

#[tokio::test]
async fn gotify_test_requires_enabled_channel_and_an_existing_token() {
    let mut fixture = Fixture::new().await;
    fixture.config.probe.notifications.gotify.enabled = false;
    let disabled = gotify_test(&fixture.config, &fixture.db).await.unwrap();
    assert!(disabled.skipped && !disabled.sent);
    assert_eq!(disabled.reason.as_deref(), Some("notifications_disabled"));
    fixture.config.probe.notifications.gotify.enabled = true;
    fixture
        .db
        .remove_secret_setting(PROBE_GOTIFY_TOKEN_SETTING)
        .unwrap();
    let missing = gotify_test(&fixture.config, &fixture.db).await.unwrap();
    assert!(missing.skipped && !missing.sent);
    assert!(!missing.device_key_configured);
    assert_eq!(fixture.gotify.count(), 0);
    fixture
        .db
        .set_secret_setting_bytes(PROBE_GOTIFY_TOKEN_SETTING, TOKEN.as_bytes())
        .unwrap();
    let sent = gotify_test(&fixture.config, &fixture.db).await.unwrap();
    assert!(sent.sent);
    assert_eq!(fixture.gotify.count(), 1);
    assert_eq!(fixture.bark.count(), 0);
}

#[tokio::test]
async fn gotify_delivers_codex_events_when_bark_is_disabled_or_legacy_bark_dedupe_hits() {
    for bark_enabled in [false, true] {
        let mut fixture = Fixture::new().await;
        fixture.config.probe.notifications.enabled = bark_enabled;
        let event = fixture.event();
        let (bark, gotify) = codex(&fixture.config, &fixture.db, &event, false, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(bark.skipped && !bark.sent);
        assert!(gotify.sent);
        assert_eq!(fixture.bark.count(), 0);
        assert_eq!(fixture.gotify.count(), 1);
        assert_eq!(
            fixture.status(NotificationChannel::Gotify, &codex_key(&event)),
            "sent"
        );
    }
}

#[tokio::test]
async fn a_failure_in_either_channel_does_not_prevent_the_other_from_delivering() {
    for failed_channel in NotificationChannel::ALL {
        let fixture = Fixture::new().await;
        let failure = Reply::json(
            StatusCode::UNAUTHORIZED,
            json!({"error":"fixture rejection"}),
        );
        match failed_channel {
            NotificationChannel::Bark => fixture.bark.reply(failure),
            NotificationChannel::Gotify => fixture.gotify.reply(failure),
        }
        let event = fixture.event();
        let (bark, gotify) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert_eq!(bark.sent, failed_channel != NotificationChannel::Bark);
        assert_eq!(gotify.sent, failed_channel != NotificationChannel::Gotify);
        assert!(!bark.skipped && !gotify.skipped);
        assert_eq!(fixture.bark.count(), 1);
        assert_eq!(fixture.gotify.count(), 1);
        assert_eq!(fixture.status(failed_channel, &codex_key(&event)), "failed");
    }
}

#[tokio::test]
async fn concurrent_codex_entrances_send_once_per_channel_and_reopening_does_not_replay() {
    let fixture = Fixture::new().await;
    let event = fixture.event();
    let (first, second) = tokio::join!(
        codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT),
        codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT),
    );
    let (first_bark, first_gotify) = first.unwrap();
    let (second_bark, second_gotify) = second.unwrap();
    assert_eq!(
        usize::from(first_bark.sent) + usize::from(second_bark.sent),
        1
    );
    assert_eq!(
        usize::from(first_gotify.sent) + usize::from(second_gotify.sent),
        1
    );
    assert_eq!(fixture.bark.count(), 1);
    assert_eq!(fixture.gotify.count(), 1);
    let reopened = PanelDb::open(fixture.db.path()).unwrap();
    let (bark, gotify) = codex(&fixture.config, &reopened, &event, true, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(bark.skipped && gotify.skipped);
    retry_pending(&fixture.config, &reopened).await.unwrap();
    assert_eq!(fixture.bark.count(), 1);
    assert_eq!(fixture.gotify.count(), 1);
}

#[tokio::test]
async fn gotify_baselines_old_codex_and_native_events_without_suppressing_bark() {
    let fixture = Fixture::new().await;
    let old_ms = fixture.event_ms() - 60_000;
    let mut event = fixture.event();
    event.payload["feedback_completed_at_ms"] = json!(old_ms);
    let (bark, gotify) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(bark.sent);
    assert!(gotify.skipped && !gotify.sent);
    let mut delivery = fixture.native_delivery(NativeProvider::Grok, "completion");
    delivery.event.timestamp_ms = old_ms;
    let (bark, gotify) = native(&fixture.config, &fixture.db, &delivery, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(bark.sent);
    assert!(gotify.skipped && !gotify.sent);
    assert_eq!(fixture.bark.count(), 2);
    assert_eq!(fixture.gotify.count(), 0);
}

#[tokio::test]
async fn gotify_receives_grok_and_claude_events_with_bark_disabled_and_dedupes_native_replay() {
    for (provider, kind) in [
        (NativeProvider::Grok, "completion"),
        (NativeProvider::Claude, "reply_needed"),
    ] {
        let mut fixture = Fixture::new().await;
        fixture.config.probe.notifications.enabled = false;
        let delivery = fixture.native_delivery(provider, kind);
        // Direct delivery only: native retry scanning is not invoked, so the test never discovers user sessions.
        let (bark, gotify) = native(&fixture.config, &fixture.db, &delivery, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(bark.skipped && !bark.sent);
        assert!(gotify.sent);
        let requests = fixture.gotify.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].body["message"],
            format!("{BODY}\n\n线程 ID：fixture-native-thread\n回合：fixture-native-turn")
        );
        assert!(requests[0].body["title"]
            .as_str()
            .unwrap()
            .contains(provider.as_str()));
        assert_eq!(fixture.bark.count(), 0);
        let reopened = PanelDb::open(fixture.db.path()).unwrap();
        let (_, duplicate) = native(&fixture.config, &reopened, &delivery, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(duplicate.skipped && !duplicate.sent);
        assert_eq!(fixture.gotify.count(), 1);
    }
}

#[tokio::test]
async fn explicit_transient_failures_are_retried_at_most_three_times_across_restart() {
    for status in [429, 500, 502, 503, 504] {
        let mut fixture = Fixture::new().await;
        fixture.config.probe.notifications.enabled = false;
        fixture.gotify.reply(Reply::json(
            StatusCode::from_u16(status).unwrap(),
            json!({"error":"fixture temporary rejection"}),
        ));
        let event = fixture.event();
        assert!(codex_fingerprint(&fixture.config, &event).is_some());
        let key = codex_key(&event);
        let (_, first) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(!first.sent && !first.skipped);
        assert_eq!(first.http_status, Some(status));
        assert_eq!(fixture.status(NotificationChannel::Gotify, &key), "pending");
        retry_pending(&fixture.config, &fixture.db).await.unwrap();
        assert_eq!(fixture.gotify.count(), 1);
        for attempt in 2..=3 {
            fixture.due_now(&key);
            let reopened = PanelDb::open(fixture.db.path()).unwrap();
            retry_pending(&fixture.config, &reopened).await.unwrap();
            assert_eq!(fixture.gotify.count(), attempt);
        }
        assert_eq!(fixture.status(NotificationChannel::Gotify, &key), "failed");
        fixture.assert_payload_cleared(NotificationChannel::Gotify, &key);
        fixture.due_now(&key);
        retry_pending(&fixture.config, &fixture.db).await.unwrap();
        let (_, duplicate) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(duplicate.skipped);
        assert_eq!(fixture.gotify.count(), 3);
        assert_eq!(fixture.bark.count(), 0);
    }
}

#[tokio::test]
async fn retry_sends_only_the_failed_channel_and_updates_one_existing_business_event() {
    let fixture = Fixture::new().await;
    fixture.gotify.reply(Reply::json(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":"fixture rejection"}),
    ));
    let event = fixture.event();
    let key = codex_key(&event);
    let (bark, gotify) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(bark.sent && !gotify.sent);
    fixture
        .db
        .record_probe_event(NewProbeEvent {
            kind: "completion",
            thread_id: Some("fixture-main"),
            title: Some("Fixture task"),
            message: Some("Fixture completion"),
            dedupe_key: Some("fixture-event"),
            source: "fixture-hook",
            payload: json!({"notification_event_key":key,"bark":bark,"gotify":gotify}),
        })
        .unwrap();
    fixture.gotify.reply(Reply::gotify());
    fixture.due_now(&key);
    let reopened = PanelDb::open(fixture.db.path()).unwrap();
    retry_pending(&fixture.config, &reopened).await.unwrap();
    assert_eq!(fixture.bark.count(), 1);
    assert_eq!(fixture.gotify.count(), 2);
    assert_eq!(fixture.status(NotificationChannel::Gotify, &key), "sent");
    let (count, payload) = fixture
        .connection()
        .query_row(
            "SELECT count(*),payload_json FROM probe_events WHERE dedupe_key='fixture-event'",
            [],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(count, 1);
    let payload: Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(payload["bark"]["sent"], true);
    assert_eq!(payload["gotify"]["sent"], true);
    assert!(!payload.to_string().contains(TOKEN));
}

#[tokio::test]
async fn retry_rechecks_source_identity_fingerprint_provider_event_and_channel_switches() {
    for change in ["transcript", "identity", "provider", "event", "channel"] {
        let mut fixture = Fixture::new().await;
        fixture.config.probe.notifications.enabled = false;
        fixture.gotify.reply(Reply::json(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"fixture rejection"}),
        ));
        let event = fixture.event();
        let key = codex_key(&event);
        let (_, outcome) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
            .await
            .unwrap();
        assert!(!outcome.sent);
        fixture.due_now(&key);
        match change {
            "transcript" => writeln!(fs::OpenOptions::new().append(true).open(&fixture.rollout).unwrap(), "{}", json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"next-fixture-turn"}})).unwrap(),
            "identity" => {
                Connection::open(fixture.config.codex.home.join("state_5.sqlite")).unwrap().execute("DELETE FROM threads WHERE id='fixture-main'", []).unwrap();
            }
            "provider" => fixture.config.probe.notifications.notify_codex = false,
            "event" => fixture.config.probe.notifications.notify_completion = false,
            "channel" => fixture.config.probe.notifications.gotify.enabled = false,
            _ => unreachable!(),
        }
        fixture.gotify.reply(Reply::gotify());
        retry_pending(&fixture.config, &fixture.db).await.unwrap();
        assert_eq!(fixture.gotify.count(), 1, "{change}");
        assert_eq!(
            fixture.status(NotificationChannel::Gotify, &key),
            "skipped",
            "{change}"
        );
        fixture.assert_payload_cleared(NotificationChannel::Gotify, &key);
    }
}

#[tokio::test]
async fn timeout_keeps_unknown_outcome_and_never_resends_after_restart() {
    let mut fixture = Fixture::new().await;
    fixture.config.probe.notifications.enabled = false;
    let mut delayed = Reply::gotify();
    delayed.delay = Duration::from_secs(1);
    fixture.gotify.reply(delayed);
    let event = fixture.event();
    let key = codex_key(&event);
    let (_, outcome) = codex(
        &fixture.config,
        &fixture.db,
        &event,
        true,
        Duration::from_millis(100),
    )
    .await
    .unwrap();
    assert!(!outcome.sent && !outcome.skipped);
    assert!(outcome.http_status.is_none());
    assert_eq!(outcome.request_count, 1);
    assert_eq!(fixture.gotify.count(), 1);
    assert_eq!(fixture.status(NotificationChannel::Gotify, &key), "unknown");
    fixture.assert_payload_cleared(NotificationChannel::Gotify, &key);
    fixture.gotify.reply(Reply::gotify());
    fixture.due_now(&key);
    let reopened = PanelDb::open(fixture.db.path()).unwrap();
    retry_pending(&fixture.config, &reopened).await.unwrap();
    let (_, duplicate) = codex(&fixture.config, &reopened, &event, true, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(duplicate.skipped && !duplicate.sent);
    assert_eq!(fixture.gotify.count(), 1);
}

#[tokio::test]
async fn a_real_pending_retry_contains_no_token_or_plaintext_message_in_the_database() {
    let mut fixture = Fixture::new().await;
    fixture.config.probe.notifications.enabled = false;
    fixture.gotify.reply(Reply::json(
        StatusCode::TOO_MANY_REQUESTS,
        json!({"error":TOKEN}),
    ));
    let event = fixture.event();
    let key = codex_key(&event);
    let (_, outcome) = codex(&fixture.config, &fixture.db, &event, true, REQUEST_TIMEOUT)
        .await
        .unwrap();
    assert!(!serde_json::to_string(&outcome).unwrap().contains(TOKEN));
    assert!(!serde_json::to_string(&fixture.config)
        .unwrap()
        .contains(TOKEN));
    fixture.due_now(&key);
    let queued = fixture.db.due_notification_deliveries(100).unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].payload["request"]["body"], BODY);
    assert!(!queued[0].payload.to_string().contains(TOKEN));
    for path in [
        fixture.db.path().to_path_buf(),
        fixture.db.path().with_extension("sqlite-wal"),
    ] {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("cannot read fixture database: {error}"),
        };
        for secret in [TOKEN, BODY] {
            assert!(!bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }
}
