use chrono::Utc;
use nexushub_core::{
    config::{patch_probe_config_toml, Config},
    db::{NotificationChannel, PanelDb},
    platform::{PlatformKind, PlatformPaths},
    services::settings::{
        build_settings_view, plan_probe_settings_save, ProbeSecretState, ProbeSettingsSaveRequest,
        PROBE_GOTIFY_TOKEN_SETTING,
    },
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

const FIXTURE_TOKEN: &str = "fixture-gotify-application-token";
const FIXTURE_BODY: &str = "独立通知队列的虚构正文，不得明文保存。";

struct Fixture {
    root: PathBuf,
    path: PathBuf,
    config: Config,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nexushub-notification-channels-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("codex/sessions")).unwrap();
        let path = root.join("panel.sqlite");
        let mut config = Config::for_platform_kind_with_home(PlatformKind::Linux, &root);
        config.codex.home = root.join("codex");
        config.codex.workspace = root.join("workspace");
        config.probe.enabled = true;
        config.probe.notifications.enabled = true;
        config.probe.notifications.gotify.enabled = true;
        config.probe.notifications.server_url = "https://bark.example.invalid".into();
        config.probe.notifications.gotify.server_url =
            "https://notify.example.invalid/gotify".into();
        Self { root, path, config }
    }

    fn open(&self) -> PanelDb {
        PanelDb::open(&self.path).unwrap()
    }

    fn connection(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }

    fn activated_ms(&self, channel: NotificationChannel) -> i64 {
        self.connection()
            .query_row(
                "SELECT activated_ms FROM probe_notification_channels WHERE channel=?1",
                [channel.as_str()],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn due_now(&self, channel: NotificationChannel, key: &str) {
        self.connection()
            .execute(
                "UPDATE probe_notification_deliveries SET next_ms=0 WHERE channel=?1 AND event_key=?2",
                params![channel.as_str(), key],
            )
            .unwrap();
    }

    fn row(&self, channel: NotificationChannel, key: &str) -> DeliveryRow {
        self.connection()
            .query_row(
                "SELECT status,attempts,ciphertext IS NOT NULL,nonce IS NOT NULL,next_ms
                 FROM probe_notification_deliveries WHERE channel=?1 AND event_key=?2",
                params![channel.as_str(), key],
                |row| {
                    Ok(DeliveryRow {
                        status: row.get(0)?,
                        attempts: row.get(1)?,
                        has_ciphertext: row.get(2)?,
                        has_nonce: row.get(3)?,
                        next_ms: row.get(4)?,
                    })
                },
            )
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct DeliveryRow {
    status: String,
    attempts: u32,
    has_ciphertext: bool,
    has_nonce: bool,
    next_ms: i64,
}

fn payload() -> Value {
    json!({"request":{"title":"通知 fixture","body":FIXTURE_BODY,"dedupe_key":"fixture-event"}})
}

fn http_failure(status: u16) -> Value {
    json!({"sent":false,"skipped":false,"http_status":status,"request_count":1,"reason":"http_status"})
}

fn stage(db: &PanelDb, fixture: &Fixture, channel: NotificationChannel, key: &str) {
    assert!(db
        .stage_notification_delivery(
            channel,
            key,
            fixture.activated_ms(NotificationChannel::Gotify) + 1,
            &payload(),
        )
        .unwrap());
}

fn assert_no_bytes(path: &Path, text: &str) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("cannot read fixture database: {error}"),
    };
    assert!(!bytes
        .windows(text.len())
        .any(|window| window == text.as_bytes()));
}

#[test]
fn old_config_keeps_bark_settings_and_defaults_gotify_to_disabled() {
    let fixture = Fixture::new();
    let mut legacy = serde_json::to_value(&fixture.config).unwrap();
    legacy["probe"]["notifications"]
        .as_object_mut()
        .unwrap()
        .remove("gotify");
    let migrated: Config = serde_json::from_value(legacy).unwrap();
    assert!(migrated.probe.notifications.enabled);
    assert_eq!(
        migrated.probe.notifications.server_url,
        "https://bark.example.invalid"
    );
    assert!(!migrated.probe.notifications.gotify.enabled);
    assert!(migrated.probe.notifications.gotify.server_url.is_empty());
    assert_eq!(migrated.probe.notifications.gotify.priority, 5);
}

#[test]
fn each_channel_can_be_enabled_without_the_other() {
    for (bark, gotify) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut fixture = Fixture::new();
        fixture.config.probe.notifications.enabled = bark;
        fixture.config.probe.notifications.gotify.enabled = gotify;
        let db = fixture.open();
        db.sync_notification_channels(&fixture.config).unwrap();
        assert_eq!(NotificationChannel::Bark.enabled(&fixture.config), bark);
        assert_eq!(NotificationChannel::Gotify.enabled(&fixture.config), gotify);
        assert_eq!(
            fixture.config.probe.notifications.any_channel_enabled(),
            bark || gotify
        );
        for (channel, enabled) in [
            (NotificationChannel::Bark, bark),
            (NotificationChannel::Gotify, gotify),
        ] {
            assert_eq!(
                db.stage_notification_delivery(
                    channel,
                    "independent-event",
                    fixture.activated_ms(NotificationChannel::Gotify) + 1,
                    &payload(),
                )
                .unwrap(),
                enabled,
            );
            assert_eq!(
                db.claim_notification_delivery(channel, "independent-event")
                    .unwrap(),
                enabled
            );
        }
    }
}

#[test]
fn first_gotify_enable_baselines_history_without_changing_the_existing_bark_baseline() {
    let fixture = Fixture::new();
    let db = fixture.open();
    let before_activation = Utc::now().timestamp_millis() - 60_000;
    db.sync_notification_channels(&fixture.config).unwrap();
    let gotify_baseline = fixture.activated_ms(NotificationChannel::Gotify);
    assert_eq!(fixture.activated_ms(NotificationChannel::Bark), 0);
    assert!(gotify_baseline > before_activation);
    assert!(db
        .stage_notification_delivery(
            NotificationChannel::Bark,
            "old-event",
            before_activation,
            &payload()
        )
        .unwrap());
    assert!(!db
        .stage_notification_delivery(
            NotificationChannel::Gotify,
            "old-event",
            before_activation,
            &payload()
        )
        .unwrap());
    assert!(!db
        .claim_notification_delivery(NotificationChannel::Gotify, "old-event")
        .unwrap());
    assert!(db
        .stage_notification_delivery(
            NotificationChannel::Gotify,
            "new-event",
            gotify_baseline,
            &payload()
        )
        .unwrap());
    db.sync_notification_channels(&fixture.config).unwrap();
    assert_eq!(
        fixture.activated_ms(NotificationChannel::Gotify),
        gotify_baseline
    );
}

#[test]
fn disabling_and_reenabling_gotify_discards_its_queue_and_preserves_bark() {
    let mut fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    for channel in NotificationChannel::ALL {
        stage(&db, &fixture, channel, "shared-event");
    }
    fixture.config.probe.notifications.gotify.enabled = false;
    db.sync_notification_channels(&fixture.config).unwrap();
    let disabled = fixture.row(NotificationChannel::Gotify, "shared-event");
    assert_eq!(disabled.status, "skipped");
    assert!(!disabled.has_ciphertext && !disabled.has_nonce);
    assert_eq!(
        fixture
            .row(NotificationChannel::Bark, "shared-event")
            .status,
        "pending"
    );
    fixture.config.probe.notifications.gotify.enabled = true;
    db.sync_notification_channels(&fixture.config).unwrap();
    let baseline = fixture.activated_ms(NotificationChannel::Gotify);
    assert!(!db
        .stage_notification_delivery(
            NotificationChannel::Gotify,
            "disabled-period-event",
            baseline - 1,
            &payload()
        )
        .unwrap());
    assert!(!db
        .claim_notification_delivery(NotificationChannel::Gotify, "shared-event")
        .unwrap());
    assert!(db
        .claim_notification_delivery(NotificationChannel::Bark, "shared-event")
        .unwrap());
    assert!(db
        .stage_notification_delivery(
            NotificationChannel::Gotify,
            "after-reenable",
            baseline + 1,
            &payload()
        )
        .unwrap());
}

#[test]
fn concurrent_database_connections_claim_one_delivery_per_channel() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    for channel in NotificationChannel::ALL {
        stage(&db, &fixture, channel, "concurrent-event");
    }
    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for index in 0..8 {
        let path = fixture.path.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let db = PanelDb::open(path).unwrap();
            let channel = NotificationChannel::ALL[index % 2];
            barrier.wait();
            (
                channel,
                db.claim_notification_delivery(channel, "concurrent-event")
                    .unwrap(),
            )
        }));
    }
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    for channel in NotificationChannel::ALL {
        assert_eq!(
            outcomes
                .iter()
                .filter(|(actual, won)| *actual == channel && *won)
                .count(),
            1
        );
        assert_eq!(fixture.row(channel, "concurrent-event").attempts, 1);
    }
}

#[test]
fn completed_channel_claims_survive_reopen_and_never_duplicate_the_other_channel() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    for channel in NotificationChannel::ALL {
        stage(&db, &fixture, channel, "restart-event");
    }
    assert!(db
        .claim_notification_delivery(NotificationChannel::Bark, "restart-event")
        .unwrap());
    db.finish_notification_delivery(
        NotificationChannel::Bark,
        "restart-event",
        &json!({"sent":true,"http_status":200,"request_count":1}),
    )
    .unwrap();
    drop(db);

    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    assert!(!db
        .claim_notification_delivery(NotificationChannel::Bark, "restart-event")
        .unwrap());
    assert!(db
        .claim_notification_delivery(NotificationChannel::Gotify, "restart-event")
        .unwrap());
    db.finish_notification_delivery(
        NotificationChannel::Gotify,
        "restart-event",
        &json!({"sent":true,"http_status":200,"request_count":1}),
    )
    .unwrap();
    drop(db);

    let db = fixture.open();
    for channel in NotificationChannel::ALL {
        assert!(!db
            .stage_notification_delivery(
                channel,
                "restart-event",
                Utc::now().timestamp_millis(),
                &payload()
            )
            .unwrap());
        assert!(!db
            .claim_notification_delivery(channel, "restart-event")
            .unwrap());
        let row = fixture.row(channel, "restart-event");
        assert_eq!(row.status, "sent");
        assert_eq!(row.attempts, 1);
        assert!(!row.has_ciphertext && !row.has_nonce);
    }
    assert!(db.due_notification_deliveries(100).unwrap().is_empty());
}

#[test]
fn explicit_transient_http_failures_wait_between_attempts_and_stop_after_three() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    for status in [429, 500, 502, 503, 504] {
        let key = format!("transient-{status}");
        stage(&db, &fixture, NotificationChannel::Gotify, &key);
        for attempt in 1..=3 {
            assert!(db
                .claim_notification_delivery(NotificationChannel::Gotify, &key)
                .unwrap());
            let before_finish = Utc::now().timestamp_millis();
            db.finish_notification_delivery(
                NotificationChannel::Gotify,
                &key,
                &http_failure(status),
            )
            .unwrap();
            let row = fixture.row(NotificationChannel::Gotify, &key);
            assert_eq!(row.attempts, attempt);
            assert!(!db
                .claim_notification_delivery(NotificationChannel::Gotify, &key)
                .unwrap());
            assert!(db.due_notification_deliveries(100).unwrap().is_empty());
            if attempt < 3 {
                assert_eq!(row.status, "pending");
                assert!(row.has_ciphertext && row.has_nonce);
                assert!(row.next_ms >= before_finish + 60_000);
                fixture.due_now(NotificationChannel::Gotify, &key);
                let due = db.due_notification_deliveries(100).unwrap();
                assert_eq!(due.len(), 1);
                assert_eq!(due[0].event_key, key);
                assert_eq!(due[0].payload, payload());
            } else {
                assert_eq!(row.status, "failed");
                assert!(!row.has_ciphertext && !row.has_nonce);
                fixture.due_now(NotificationChannel::Gotify, &key);
                assert!(!db
                    .claim_notification_delivery(NotificationChannel::Gotify, &key)
                    .unwrap());
            }
        }
    }
}

#[test]
fn ambiguous_outcomes_permanent_http_errors_and_partial_bark_sends_are_not_retried() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    let cases = [
        (
            "timeout",
            json!({"sent":false,"skipped":false,"reason":"timeout","request_count":1}),
            "unknown",
        ),
        (
            "unknown",
            json!({"sent":false,"skipped":false,"reason":"delivery_outcome_unknown","request_count":1}),
            "unknown",
        ),
        (
            "invalid-ack",
            json!({"sent":false,"skipped":false,"reason":"response_invalid","request_count":1}),
            "unknown",
        ),
        ("redirect", http_failure(307), "failed"),
        ("bad-request", http_failure(400), "failed"),
        ("unauthorized", http_failure(401), "failed"),
        ("request-timeout", http_failure(408), "failed"),
        ("not-implemented", http_failure(501), "failed"),
        ("unrecognized-server-error", http_failure(599), "failed"),
        (
            "partially-sent",
            json!({"sent":false,"skipped":false,"http_status":503,"request_count":2}),
            "failed",
        ),
    ];
    for (key, outcome, expected) in cases {
        stage(&db, &fixture, NotificationChannel::Bark, key);
        assert!(db
            .claim_notification_delivery(NotificationChannel::Bark, key)
            .unwrap());
        db.finish_notification_delivery(NotificationChannel::Bark, key, &outcome)
            .unwrap();
        fixture.due_now(NotificationChannel::Bark, key);
        let row = fixture.row(NotificationChannel::Bark, key);
        assert_eq!(row.status, expected, "{key}");
        assert_eq!(row.attempts, 1);
        assert!(!row.has_ciphertext && !row.has_nonce);
        assert!(!db
            .claim_notification_delivery(NotificationChannel::Bark, key)
            .unwrap());
    }
    drop(db);
    assert!(fixture
        .open()
        .due_notification_deliveries(100)
        .unwrap()
        .is_empty());
}

#[test]
fn interrupted_delivery_becomes_unknown_after_restart_without_replay() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    stage(
        &db,
        &fixture,
        NotificationChannel::Gotify,
        "interrupted-event",
    );
    assert!(db
        .claim_notification_delivery(NotificationChannel::Gotify, "interrupted-event")
        .unwrap());
    fixture.connection().execute(
        "UPDATE probe_notification_deliveries SET updated_ms=?1 WHERE event_key='interrupted-event'",
        [Utc::now().timestamp_millis() - 120_001],
    ).unwrap();
    drop(db);

    let db = fixture.open();
    assert!(db.due_notification_deliveries(100).unwrap().is_empty());
    let row = fixture.row(NotificationChannel::Gotify, "interrupted-event");
    assert_eq!(row.status, "unknown");
    assert_eq!(row.attempts, 1);
    assert!(!row.has_ciphertext && !row.has_nonce);
    assert!(!db
        .claim_notification_delivery(NotificationChannel::Gotify, "interrupted-event")
        .unwrap());
    assert!(!db
        .stage_notification_delivery(
            NotificationChannel::Gotify,
            "interrupted-event",
            Utc::now().timestamp_millis(),
            &payload()
        )
        .unwrap());
}

#[test]
fn secret_and_pending_body_are_encrypted_and_status_never_returns_the_token() {
    let fixture = Fixture::new();
    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    db.set_secret_setting_bytes(PROBE_GOTIFY_TOKEN_SETTING, FIXTURE_TOKEN.as_bytes())
        .unwrap();
    stage(&db, &fixture, NotificationChannel::Gotify, "private-event");
    let secret_setting = db.get_setting(PROBE_GOTIFY_TOKEN_SETTING).unwrap().unwrap();
    assert!(!secret_setting.contains(FIXTURE_TOKEN));
    assert!(serde_json::from_str::<Value>(&secret_setting).unwrap()["ciphertext"].is_string());
    let due = db.due_notification_deliveries(100).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].payload, payload());
    assert!(!due[0].payload.to_string().contains(FIXTURE_TOKEN));
    assert!(!db
        .notification_channel_status()
        .unwrap()
        .to_string()
        .contains(FIXTURE_TOKEN));
    for path in [&fixture.path, &fixture.path.with_extension("sqlite-wal")] {
        assert_no_bytes(path, FIXTURE_TOKEN);
        assert_no_bytes(path, FIXTURE_BODY);
    }
    drop(db);
    let db = fixture.open();
    assert_eq!(
        db.get_secret_setting_bytes(PROBE_GOTIFY_TOKEN_SETTING)
            .unwrap()
            .as_deref(),
        Some(FIXTURE_TOKEN.as_bytes())
    );
    assert_eq!(
        db.due_notification_deliveries(100).unwrap()[0].payload,
        payload()
    );
    db.remove_secret_setting(PROBE_GOTIFY_TOKEN_SETTING)
        .unwrap();
    assert!(db
        .get_secret_setting_bytes(PROBE_GOTIFY_TOKEN_SETTING)
        .unwrap()
        .is_none());
}

#[test]
fn saving_gotify_token_writes_only_a_secret_plan_and_safe_configuration() {
    let fixture = Fixture::new();
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Linux, &fixture.root);
    let request: ProbeSettingsSaveRequest = serde_json::from_value(json!({
        "gotify": {"enabled":true,"server_url":"  https://notify.example.invalid/gotify/  ","priority":7,"token":format!("  {FIXTURE_TOKEN}  ")}
    })).unwrap();
    assert!(!serde_json::to_string(&request)
        .unwrap()
        .contains(FIXTURE_TOKEN));
    let normalized = request.clone().normalize().unwrap();
    assert_eq!(normalized.gotify_token.as_deref(), Some(FIXTURE_TOKEN));
    assert!(!serde_json::to_string(&normalized)
        .unwrap()
        .contains(FIXTURE_TOKEN));
    let plan = plan_probe_settings_save(&platform, request).unwrap();
    assert_eq!(plan.secret_writes.len(), 1);
    assert_eq!(
        plan.secret_writes[0].setting_key,
        PROBE_GOTIFY_TOKEN_SETTING
    );
    assert_eq!(plan.secret_writes[0].secret_value, FIXTURE_TOKEN);
    assert_eq!(
        plan.audit_detail[PROBE_GOTIFY_TOKEN_SETTING],
        "[configured]"
    );
    assert!(!serde_json::to_string(&plan)
        .unwrap()
        .contains(FIXTURE_TOKEN));
    let written = patch_probe_config_toml(
        "[probe.notifications]\nenabled = true\nserver_url = 'https://bark.example.invalid'\n",
        &plan.config_write.as_ref().unwrap().patch,
    )
    .unwrap();
    let config: toml::Value = toml::from_str(&written).unwrap();
    assert_eq!(
        config["probe"]["notifications"]["enabled"].as_bool(),
        Some(true)
    );
    assert_eq!(
        config["probe"]["notifications"]["gotify"]["enabled"].as_bool(),
        Some(true)
    );
    assert_eq!(
        config["probe"]["notifications"]["gotify"]["priority"].as_integer(),
        Some(7)
    );
    assert_eq!(
        config["probe"]["notifications"]["gotify"]["server_url"].as_str(),
        Some("https://notify.example.invalid/gotify")
    );
    assert!(!written.contains(FIXTURE_TOKEN));
    assert!(config["probe"]["notifications"]["gotify"]
        .get("token")
        .is_none());
    let view = serde_json::to_value(build_settings_view(
        &fixture.config,
        ProbeSecretState::Configured,
    ))
    .unwrap();
    assert!(view["gotify"].get("token").is_none());
    assert!(view["gotify"]["token_configured"].is_boolean());
    assert!(!view.to_string().contains(FIXTURE_TOKEN));
}

#[test]
fn clearing_gotify_token_disables_only_gotify_and_empty_input_preserves_existing_secret() {
    let fixture = Fixture::new();
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Linux, &fixture.root);
    let clear: ProbeSettingsSaveRequest =
        serde_json::from_value(json!({"gotify":{"enabled":true,"clear_token":true}})).unwrap();
    let plan = plan_probe_settings_save(&platform, clear).unwrap();
    assert_eq!(plan.secret_writes.len(), 1);
    assert_eq!(
        plan.secret_writes[0].setting_key,
        PROBE_GOTIFY_TOKEN_SETTING
    );
    assert!(plan.secret_writes[0].secret_value.is_empty());
    assert_eq!(plan.audit_detail[PROBE_GOTIFY_TOKEN_SETTING], "[removed]");
    let notifications = plan.config_patch.probe.unwrap().notifications.unwrap();
    assert_eq!(notifications.enabled, None);
    assert_eq!(notifications.gotify.unwrap().enabled, Some(false));

    let empty: ProbeSettingsSaveRequest =
        serde_json::from_value(json!({"gotify":{"token":"  \n  "}})).unwrap();
    assert!(plan_probe_settings_save(&platform, empty)
        .unwrap()
        .secret_writes
        .is_empty());
    let conflict: ProbeSettingsSaveRequest =
        serde_json::from_value(json!({"gotify":{"token":FIXTURE_TOKEN,"clear_token":true}}))
            .unwrap();
    let error = plan_probe_settings_save(&platform, conflict)
        .unwrap_err()
        .to_string();
    assert!(!error.contains(FIXTURE_TOKEN));
}

#[test]
fn invalid_gotify_settings_are_rejected_without_echoing_token_values() {
    let fixture = Fixture::new();
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Linux, &fixture.root);
    for gotify in [
        json!({"server_url":"http://notify.example.invalid"}),
        json!({"server_url":"https://notify.example.invalid/gotify?token=fixture"}),
        json!({"server_url":"https://notify.example.invalid/gotify#fragment"}),
        json!({"priority":11}),
        json!({"token":format!("{FIXTURE_TOKEN}\ninvalid")}),
        json!({"token":"x".repeat(513)}),
    ] {
        let request: ProbeSettingsSaveRequest =
            serde_json::from_value(json!({"gotify":gotify})).unwrap();
        let error = plan_probe_settings_save(&platform, request)
            .unwrap_err()
            .to_string();
        assert!(!error.contains(FIXTURE_TOKEN));
    }
}

#[test]
fn migration_preserves_legacy_bark_claims_without_marking_gotify_delivered() {
    let fixture = Fixture::new();
    drop(fixture.open());
    let connection = fixture.connection();
    connection
        .execute_batch(
            "DELETE FROM settings WHERE key='probe_channel_migration_v1';
         DROP TABLE probe_notification_deliveries;
         DROP TABLE probe_notification_channels;",
        )
        .unwrap();
    for status in ["sent", "skipped", "delivering", "failed", "pending"] {
        connection.execute(
            "INSERT INTO native_probe_deliveries(event_key,provider,event_json,status,created_at,updated_at) VALUES(?1,'claude_code','{}',?2,1,2)",
            params![format!("legacy-{status}"), status],
        ).unwrap();
    }
    for key in [
        "probe_async_question_delivery:fixture-call",
        "probe_feedback_delivery:fixture-turn",
    ] {
        connection
            .execute(
                "INSERT INTO settings(key,value,updated_at) VALUES(?1,'{}',2)",
                [key],
            )
            .unwrap();
    }
    drop(connection);

    let db = fixture.open();
    db.sync_notification_channels(&fixture.config).unwrap();
    for (key, expected) in [
        ("native:legacy-sent", "sent"),
        ("native:legacy-skipped", "skipped"),
        ("native:legacy-delivering", "unknown"),
        ("probe_async_question_delivery:fixture-call", "legacy"),
        ("probe_feedback_delivery:fixture-turn", "legacy"),
    ] {
        assert_eq!(fixture.row(NotificationChannel::Bark, key).status, expected);
        assert!(!db
            .claim_notification_delivery(NotificationChannel::Bark, key)
            .unwrap());
        assert!(!db
            .stage_notification_delivery(
                NotificationChannel::Bark,
                key,
                Utc::now().timestamp_millis(),
                &payload()
            )
            .unwrap());
        assert!(!db
            .stage_notification_delivery(NotificationChannel::Gotify, key, 1_000, &payload())
            .unwrap());
    }
    assert_eq!(
        fixture
            .connection()
            .query_row(
                "SELECT count(*) FROM probe_notification_deliveries",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        5
    );
    assert_eq!(
        db.get_setting("probe_channel_migration_v1")
            .unwrap()
            .as_deref(),
        Some("1")
    );
    db.set_setting("probe_async_question_delivery:after-migration", "{}")
        .unwrap();
    drop(db);
    drop(fixture.open());
    assert_eq!(
        fixture
            .connection()
            .query_row(
                "SELECT count(*) FROM probe_notification_deliveries",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        5
    );
}
