use super::super::framing::{write_frame, FrameReader};
use super::super::{DesktopError, DesktopSession};
use super::DesktopWorker;
use crate::{
    crypto::SecretBox,
    db::{desktop_submissions::DesktopLedger, PanelDb},
    services::codex_desktop::{DesktopAction, DesktopView},
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot},
    time::timeout,
};

const THREAD_ID: &str = "11111111-1111-4111-8111-111111111111";
const CLIENT_ID: &str = "55555555-5555-4555-8555-555555555555";
const OWNER_ID: &str = "66666666-6666-4666-8666-666666666666";
const TURN_ID: &str = "33333333-3333-4333-8333-333333333333";
const OTHER_TURN_ID: &str = "44444444-4444-4444-8444-444444444444";
const NEXT_TURN_ID: &str = "77777777-7777-4777-8777-777777777777";
const FIRST_MESSAGE_ID: &str = "88888888-8888-4888-8888-888888888888";
const SECOND_MESSAGE_ID: &str = "99999999-9999-4999-8999-999999999999";

struct Fixture {
    root: PathBuf,
    codex_home: PathBuf,
    socket: PathBuf,
    database: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!("nh-desk-worker-{}", uuid::Uuid::new_v4()));
        let codex_home = root.join("codex-home");
        let ipc = codex_home.join("ipc");
        fs::create_dir_all(&ipc).expect("temporary IPC directories are created");
        set_private_directory(&root);
        set_private_directory(&codex_home);
        set_private_directory(&ipc);
        Self {
            socket: ipc.join("ipc.sock"),
            database: root.join("panel.sqlite"),
            root,
            codex_home,
        }
    }

    fn bind(&self) -> UnixListener {
        let listener = UnixListener::bind(&self.socket).expect("fake IPC socket is bound");
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o600))
            .expect("fake IPC socket permissions are restricted");
        listener
    }

    fn ledger(&self) -> DesktopLedger {
        let crypto = SecretBox::deterministic_dev();
        let db = PanelDb::open_with_secret_box(&self.database, crypto.clone())
            .expect("temporary PanelDb is initialized");
        DesktopLedger::new(db, crypto)
    }

    fn enqueue_pair(&self, ledger: &DesktopLedger, source: &str) {
        ledger
            .enqueue(
                source,
                THREAD_ID,
                FIRST_MESSAGE_ID,
                &json!({"kind":"queue","text":"fixture first queued message"}),
            )
            .expect("first fixture message is queued");
        ledger
            .enqueue(
                source,
                THREAD_ID,
                SECOND_MESSAGE_ID,
                &json!({"kind":"queue","text":"fixture second queued message"}),
            )
            .expect("second fixture message is queued");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn set_private_directory(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("temporary fixture directory permissions are restricted");
}

fn source_id() -> String {
    "ab".repeat(32)
}

#[test]
fn empty_spans_preserve_legacy_ledger_bytes_and_nonempty_spans_bind_identity() {
    use super::super::references::{NativeReference, ReferenceSpan};
    let fixture = Fixture::new();
    let ledger = fixture.ledger();
    let source = source_id();
    let references = vec![NativeReference::app("Fixture".into(), "app://fixture".into()).unwrap()];
    let legacy = json!({"kind":"queue","text":"@Fixture","references":references});
    let body = super::submission_body("queue", "@Fixture", &references, &[]);
    assert_eq!(
        serde_json::to_vec(&legacy).unwrap(),
        serde_json::to_vec(&body).unwrap()
    );
    ledger
        .enqueue(&source, THREAD_ID, FIRST_MESSAGE_ID, &legacy)
        .unwrap();
    ledger
        .enqueue(&source, THREAD_ID, FIRST_MESSAGE_ID, &body)
        .unwrap();
    let body = super::submission_body(
        "queue",
        "@Fixture",
        &references,
        &[ReferenceSpan {
            reference: 0,
            start: 0,
            end: 8,
        }],
    );
    assert!(ledger
        .enqueue(&source, THREAD_ID, FIRST_MESSAGE_ID, &body)
        .is_err());
    ledger
        .enqueue(&source, THREAD_ID, SECOND_MESSAGE_ID, &body)
        .unwrap();
    let reopened = fixture.ledger();
    let items = reopened.list(&source, THREAD_ID).unwrap();
    assert_eq!(items[1].body.as_ref(), Some(&body));
}

fn turn(id: &str, status: &str) -> Value {
    json!({"id":id,"status":status,"items":[]})
}

fn thread_state(runtime: &str, turns: Vec<Value>) -> Value {
    json!({
        "id": THREAD_ID,
        "hostId": "local",
        "parentThreadId": null,
        "archived": false,
        "source": {},
        "threadSource": {},
        "threadRuntimeStatus": {"type": runtime},
        "turns": turns,
        "requests": []
    })
}

async fn read_frame(stream: &mut UnixStream) -> std::result::Result<Value, DesktopError> {
    FrameReader::default().read(stream).await
}

async fn send_success(
    stream: &mut UnixStream,
    request: &Value,
    result: Value,
    handled_by: Option<&str>,
) {
    let mut response = json!({
        "type":"response",
        "requestId":request["requestId"],
        "resultType":"success",
        "result":result
    });
    if let Some(owner) = handled_by {
        response["handledByClientId"] = json!(owner);
    }
    write_frame(stream, &response)
        .await
        .expect("fixture IPC response is written");
}

async fn send_snapshot(stream: &mut UnixStream, revision: u64, state: Value) {
    write_frame(
        stream,
        &json!({
            "type":"broadcast",
            "sourceClientId":OWNER_ID,
            "method":"thread-stream-state-changed",
            "version":11,
            "params":{
                "conversationId":THREAD_ID,
                "change":{"type":"snapshot","revision":revision,"conversationState":state}
            }
        }),
    )
    .await
    .expect("fixture state snapshot is written");
}

async fn accept_session(listener: &UnixListener, state: Value) -> UnixStream {
    let (mut stream, _) = listener
        .accept()
        .await
        .expect("fake IPC accepts the local test session");

    let initialize = read_frame(&mut stream)
        .await
        .expect("session initialization is received");
    assert_eq!(initialize["method"], "initialize");
    send_success(
        &mut stream,
        &initialize,
        json!({"clientId":CLIENT_ID}),
        None,
    )
    .await;

    let discovery = read_frame(&mut stream)
        .await
        .expect("initial owner discovery is received");
    assert_eq!(discovery["method"], "thread-owner-discovery");
    send_success(&mut stream, &discovery, json!({}), Some(OWNER_ID)).await;

    let following = read_frame(&mut stream)
        .await
        .expect("session follow broadcast is received");
    assert_eq!(following["method"], "thread-stream-following-changed");
    assert_eq!(following["params"]["following"], true);
    send_snapshot(&mut stream, 1, state).await;
    stream
}

async fn expect_request(stream: &mut UnixStream, method: &str) -> Value {
    let request = read_frame(stream)
        .await
        .unwrap_or_else(|error| panic!("expected fixture request {method}, got {error}"));
    assert_eq!(request["type"], "request");
    assert_eq!(request["method"], method);
    request
}

async fn expect_no_frame(stream: &mut UnixStream) {
    match timeout(Duration::from_millis(250), read_frame(stream)).await {
        Err(_) => {}
        Ok(Ok(frame)) => {
            panic!("unexpected native IPC frame before the allowed transition: {frame}")
        }
        Ok(Err(error)) => panic!("fixture IPC closed before the expected transition: {error}"),
    }
}

async fn wait_for_view(
    worker: &DesktopWorker,
    matches: impl Fn(&DesktopView) -> bool,
) -> DesktopView {
    let mut view = worker.subscribe();
    timeout(Duration::from_secs(5), async {
        loop {
            let current = view.borrow().clone();
            if matches(&current) {
                return current;
            }
            view.changed()
                .await
                .expect("worker view remains available while actor is running");
        }
    })
    .await
    .expect("expected desktop worker view arrives before timeout")
}

enum BrokerCommand {
    Snapshot { revision: u64, state: Value },
    CheckNoFrame(oneshot::Sender<()>),
}

#[tokio::test]
async fn queued_dispatch_waits_for_the_exact_turn_to_finish() {
    let fixture = Fixture::new();
    let ledger = fixture.ledger();
    let source = source_id();
    fixture.enqueue_pair(&ledger, &source);
    let listener = fixture.bind();
    let (broker_commands, mut commands) = mpsc::unbounded_channel();
    let (first_started_tx, first_started_rx) = oneshot::channel();
    let (second_started_tx, second_started_rx) = oneshot::channel();

    let broker = tokio::spawn(async move {
        let mut stream = accept_session(&listener, thread_state("idle", Vec::new())).await;

        let owner_check = expect_request(&mut stream, "thread-owner-discovery").await;
        assert!(owner_check.get("targetClientId").is_none());
        send_success(&mut stream, &owner_check, json!({}), Some(OWNER_ID)).await;

        let first = expect_request(&mut stream, "thread-follower-start-turn").await;
        assert_eq!(
            first["params"]["turnStart"]["request"]["clientUserMessageId"],
            FIRST_MESSAGE_ID
        );
        send_success(
            &mut stream,
            &first,
            json!({"result":{"turn":{"id":TURN_ID}}}),
            Some(OWNER_ID),
        )
        .await;
        first_started_tx
            .send(())
            .expect("test receives the first start acknowledgement");

        let BrokerCommand::Snapshot { revision, state } = commands
            .recv()
            .await
            .expect("test supplies a nonmatching completion snapshot")
        else {
            panic!("expected a nonmatching completion snapshot");
        };
        send_snapshot(&mut stream, revision, state).await;

        let BrokerCommand::CheckNoFrame(done) = commands
            .recv()
            .await
            .expect("test asks for the no-resend observation")
        else {
            panic!("expected a no-resend observation request");
        };
        expect_no_frame(&mut stream).await;
        done.send(()).expect("no-resend observation is reported");

        let BrokerCommand::Snapshot { revision, state } = commands
            .recv()
            .await
            .expect("test supplies the exact completed turn")
        else {
            panic!("expected the exact completed-turn snapshot");
        };
        send_snapshot(&mut stream, revision, state).await;

        let owner_check = expect_request(&mut stream, "thread-owner-discovery").await;
        send_success(&mut stream, &owner_check, json!({}), Some(OWNER_ID)).await;
        let second = expect_request(&mut stream, "thread-follower-start-turn").await;
        assert_eq!(
            second["params"]["turnStart"]["request"]["clientUserMessageId"],
            SECOND_MESSAGE_ID
        );
        send_success(
            &mut stream,
            &second,
            json!({"result":{"turn":{"id":NEXT_TURN_ID}}}),
            Some(OWNER_ID),
        )
        .await;
        second_started_tx
            .send(())
            .expect("test receives the second start acknowledgement");

        let following = timeout(Duration::from_secs(3), read_frame(&mut stream))
            .await
            .expect("worker closes its session after the test finishes")
            .expect("session close broadcasts are framed");
        assert_eq!(following["method"], "thread-stream-following-changed");
        assert_eq!(following["params"]["following"], false);
    });

    let session = DesktopSession::attach(&fixture.codex_home, THREAD_ID)
        .await
        .expect("worker attaches to the fake desktop socket");
    let worker = DesktopWorker::spawn(session, ledger, source)
        .await
        .expect("worker starts with the temporary encrypted queue");
    worker
        .act(DesktopAction::ResumeQueue)
        .await
        .expect("fixture queue resume action is accepted");
    first_started_rx
        .await
        .expect("first message reaches the fake owner");

    wait_for_view(&worker, |view| {
        view.queue.len() == 1
            && view.queue[0].id == SECOND_MESSAGE_ID
            && view.queue[0].status == "queued"
    })
    .await;

    broker_commands
        .send(BrokerCommand::Snapshot {
            revision: 2,
            state: thread_state("idle", vec![turn(OTHER_TURN_ID, "completed")]),
        })
        .expect("nonmatching completion snapshot reaches fake owner");
    wait_for_view(&worker, |view| view.revision == 2).await;
    let (no_frame_tx, no_frame_rx) = oneshot::channel();
    broker_commands
        .send(BrokerCommand::CheckNoFrame(no_frame_tx))
        .expect("worker dispatch is checked before the exact completion");
    no_frame_rx
        .await
        .expect("fake owner completes the no-resend observation");

    broker_commands
        .send(BrokerCommand::Snapshot {
            revision: 3,
            state: thread_state("idle", vec![turn(TURN_ID, "completed")]),
        })
        .expect("exact completed turn reaches fake owner");
    second_started_rx
        .await
        .expect("second message starts only after its predecessor finishes");
    wait_for_view(&worker, |view| view.queue.is_empty()).await;

    drop(worker);
    broker.await.expect("fake broker assertions pass");
}

#[tokio::test]
async fn outcome_unknown_is_not_replayed_after_worker_restart_and_resume() {
    let fixture = Fixture::new();
    let ledger = fixture.ledger();
    let source = source_id();
    fixture.enqueue_pair(&ledger, &source);
    let listener = fixture.bind();
    let (check_no_replay_tx, mut check_no_replay_rx) =
        mpsc::unbounded_channel::<oneshot::Sender<()>>();
    let (second_session_tx, second_session_rx) = oneshot::channel();

    let broker = tokio::spawn(async move {
        let mut first_stream = accept_session(&listener, thread_state("idle", Vec::new())).await;
        let owner_check = expect_request(&mut first_stream, "thread-owner-discovery").await;
        send_success(&mut first_stream, &owner_check, json!({}), Some(OWNER_ID)).await;
        let first = expect_request(&mut first_stream, "thread-follower-start-turn").await;
        assert_eq!(
            first["params"]["turnStart"]["request"]["clientUserMessageId"],
            FIRST_MESSAGE_ID
        );
        write_frame(
            &mut first_stream,
            &json!({
                "type":"response",
                "requestId":first["requestId"],
                "resultType":"error",
                "error":"fixture-native-error"
            }),
        )
        .await
        .expect("ambiguous native error is returned");
        let closed = timeout(Duration::from_secs(3), read_frame(&mut first_stream))
            .await
            .expect("ambiguous submission closes the IPC session")
            .expect_err("no automatic retry uses the uncertain session");
        assert_eq!(closed, DesktopError::Disconnected);

        let mut second_stream = accept_session(&listener, thread_state("idle", Vec::new())).await;
        second_session_tx
            .send(())
            .expect("test observes the replacement session handshake");
        let done = check_no_replay_rx
            .recv()
            .await
            .expect("test requests a restart replay check");
        expect_no_frame(&mut second_stream).await;
        done.send(())
            .expect("replacement session sent no replay request");

        let following = timeout(Duration::from_secs(3), read_frame(&mut second_stream))
            .await
            .expect("replacement worker closes its test session")
            .expect("replacement session close is framed");
        assert_eq!(following["method"], "thread-stream-following-changed");
        assert_eq!(following["params"]["following"], false);
    });

    let first_session = DesktopSession::attach(&fixture.codex_home, THREAD_ID)
        .await
        .expect("first worker attaches to fake desktop socket");
    let first_worker = DesktopWorker::spawn(first_session, ledger.clone(), source.clone())
        .await
        .expect("first worker starts");
    first_worker
        .act(DesktopAction::ResumeQueue)
        .await
        .expect("fixture queue is eligible for its first dispatch");

    wait_for_view(&first_worker, |view| {
        view.queue
            .iter()
            .any(|item| item.id == FIRST_MESSAGE_ID && item.status == "outcome_unknown")
            && view
                .queue
                .iter()
                .any(|item| item.id == SECOND_MESSAGE_ID && item.status == "held")
    })
    .await;
    drop(first_worker);

    ledger
        .recover(&source)
        .expect("restart recovery preserves the ambiguous status");
    let second_session = DesktopSession::attach(&fixture.codex_home, THREAD_ID)
        .await
        .expect("replacement worker attaches to the fake desktop socket");
    second_session_rx
        .await
        .expect("fake broker observes the replacement session");
    let second_worker = DesktopWorker::spawn(second_session, ledger.clone(), source.clone())
        .await
        .expect("replacement worker loads the same encrypted ledger");
    second_worker
        .act(DesktopAction::ResumeQueue)
        .await
        .expect("user may resume definite queued work");
    wait_for_view(&second_worker, |view| {
        view.queue
            .iter()
            .any(|item| item.id == FIRST_MESSAGE_ID && item.status == "outcome_unknown")
            && view
                .queue
                .iter()
                .any(|item| item.id == SECOND_MESSAGE_ID && item.status == "queued")
    })
    .await;
    let (done_tx, done_rx) = oneshot::channel();
    check_no_replay_tx
        .send(done_tx)
        .expect("replacement session is checked for replay");
    done_rx
        .await
        .expect("replacement session confirms no replay request");
    let records = ledger
        .list(&source, THREAD_ID)
        .expect("ambiguous and queued records remain readable");
    assert_eq!(records[0].id, FIRST_MESSAGE_ID);
    assert_eq!(records[0].status, "outcome_unknown");
    assert_eq!(records[1].id, SECOND_MESSAGE_ID);
    assert_eq!(records[1].status, "queued");

    drop(second_worker);
    broker.await.expect("fake broker assertions pass");
}

#[tokio::test]
async fn stop_holds_and_preserves_encrypted_queue_items() {
    let fixture = Fixture::new();
    let ledger = fixture.ledger();
    let source = source_id();
    fixture.enqueue_pair(&ledger, &source);
    let listener = fixture.bind();
    let (no_start_tx, no_start_rx) = oneshot::channel();

    let broker = tokio::spawn(async move {
        let mut stream = accept_session(
            &listener,
            thread_state("active", vec![turn(TURN_ID, "inProgress")]),
        )
        .await;
        let owner_check = expect_request(&mut stream, "thread-owner-discovery").await;
        send_success(&mut stream, &owner_check, json!({}), Some(OWNER_ID)).await;
        let stop = expect_request(&mut stream, "thread-follower-interrupt-turn").await;
        assert_eq!(stop["params"]["expectedTurnId"], TURN_ID);
        send_snapshot(
            &mut stream,
            2,
            thread_state("idle", vec![turn(TURN_ID, "interrupted")]),
        )
        .await;
        send_success(
            &mut stream,
            &stop,
            json!({"ok":true,"interruptedTurnId":TURN_ID}),
            Some(OWNER_ID),
        )
        .await;
        expect_no_frame(&mut stream).await;
        no_start_tx
            .send(())
            .expect("fake owner confirms Stop did not drain the queue");

        let following = timeout(Duration::from_secs(3), read_frame(&mut stream))
            .await
            .expect("worker closes its test session")
            .expect("session close is framed");
        assert_eq!(following["method"], "thread-stream-following-changed");
        assert_eq!(following["params"]["following"], false);
    });

    let session = DesktopSession::attach(&fixture.codex_home, THREAD_ID)
        .await
        .expect("worker attaches to active fake desktop thread");
    let worker = DesktopWorker::spawn(session, ledger.clone(), source.clone())
        .await
        .expect("worker starts with queued items");
    worker
        .act(DesktopAction::Stop {
            turn_id: TURN_ID.into(),
        })
        .await
        .expect("matching active turn is stopped");
    let view = wait_for_view(&worker, |view| {
        view.queue.len() == 2 && view.queue.iter().all(|item| item.status == "held")
    })
    .await;
    assert_eq!(view.queue[0].id, FIRST_MESSAGE_ID);
    assert_eq!(view.queue[1].id, SECOND_MESSAGE_ID);
    no_start_rx
        .await
        .expect("fake owner observes no queue dispatch after Stop");

    let records = ledger
        .list(&source, THREAD_ID)
        .expect("held queue records remain decryptable");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].status, "held");
    assert_eq!(
        records[0].body.as_ref().unwrap()["text"],
        "fixture first queued message"
    );
    assert_eq!(records[1].status, "held");
    assert_eq!(
        records[1].body.as_ref().unwrap()["text"],
        "fixture second queued message"
    );

    drop(worker);
    broker.await.expect("fake broker assertions pass");
}
