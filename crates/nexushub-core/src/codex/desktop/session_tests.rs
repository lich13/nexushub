use super::super::{
    commands::{CollaborationMode, CommandResult, DesktopCommand, MessageInput},
    framing::{write_frame, FrameReader, FrameWriter},
    references::NativeReference,
    state::{NativeState, RuntimeState},
    DesktopError, DesktopEvent, DesktopReadSession,
};
use super::DesktopSession;
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
use tokio::{
    net::UnixStream,
    time::{timeout, Duration},
};

const THREAD_ID: &str = "11111111-1111-4111-8111-111111111111";
const CLIENT_ID: &str = "55555555-5555-4555-8555-555555555555";
const OWNER_ID: &str = "66666666-6666-4666-8666-666666666666";
const OTHER_OWNER_ID: &str = "77777777-7777-4777-8777-777777777777";
const TURN_ID: &str = "33333333-3333-4333-8333-333333333333";

struct ReferenceFixture(PathBuf);

impl ReferenceFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nexushub-desktop-session-references-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("synthetic reference fixture directory is created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("synthetic reference fixture directory is private");
        Self(root)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("synthetic reference fixture file is written");
        path
    }
}

impl Drop for ReferenceFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn idle_state() -> Value {
    json!({
        "id": THREAD_ID,
        "hostId": "local",
        "parentThreadId": null,
        "archived": false,
        "source": {},
        "threadSource": {},
        "threadRuntimeStatus": {"type": "idle"},
        "turns": [],
        "requests": []
    })
}

fn session_pair() -> (DesktopSession, UnixStream) {
    session_pair_with_state(idle_state())
}

fn active_state() -> Value {
    json!({
        "id": THREAD_ID,
        "hostId": "local",
        "parentThreadId": null,
        "archived": false,
        "source": {},
        "threadSource": {},
        "threadRuntimeStatus": {"type": "active"},
        "turns": [{"id": TURN_ID, "status": "inProgress", "items": []}],
        "requests": [],
        "cwd": "/fixture/workspace"
    })
}

fn session_pair_with_state(initial_state: Value) -> (DesktopSession, UnixStream) {
    let (client, broker) = UnixStream::pair().expect("fixture socket pair is created");
    let mut state = NativeState::new(THREAD_ID).expect("fixture thread ID is valid");
    state
        .apply(DesktopEvent::Snapshot {
            revision: 1,
            state: initial_state,
        })
        .expect("fixture snapshot is accepted");
    let connection = DesktopReadSession {
        stream: Some(client),
        reader: FrameReader::default(),
        writer: FrameWriter::default(),
        conversation_id: THREAD_ID.into(),
        client_id: CLIENT_ID.into(),
        owner_id: OWNER_ID.into(),
        revision: Some(1),
    };
    (DesktopSession { connection, state }, broker)
}

async fn read_frame(stream: &mut UnixStream) -> std::result::Result<Value, DesktopError> {
    FrameReader::default().read(stream).await
}

async fn send_owner_response(stream: &mut UnixStream, request: &Value, owner_id: &str) {
    let response = json!({
        "type": "response",
        "requestId": request["requestId"],
        "resultType": "success",
        "handledByClientId": owner_id
    });
    write_frame(stream, &response)
        .await
        .expect("fixture owner response is written");
}

fn start_command() -> DesktopCommand {
    DesktopCommand::Start(MessageInput {
        client_id: "88888888-8888-4888-8888-888888888888".into(),
        text: "fixture start".into(),
        references: vec![],
    })
}

fn settings_command() -> DesktopCommand {
    DesktopCommand::Settings {
        model: "fixture-model".into(),
        reasoning_effort: Some("medium".into()),
        mode: CollaborationMode::Default,
    }
}

fn steer_command() -> DesktopCommand {
    DesktopCommand::Steer {
        message: MessageInput {
            client_id: "88888888-8888-4888-8888-888888888888".into(),
            text: "fixture steer".into(),
            references: vec![],
        },
        expected_turn_id: TURN_ID.into(),
    }
}

async fn execute_with_native_result(
    command: DesktopCommand,
    initial_state: Value,
    native_result: Value,
) -> std::result::Result<CommandResult, DesktopError> {
    let (mut session, mut broker) = session_pair_with_state(initial_state);
    let broker_task = tokio::spawn(async move {
        let recheck = read_frame(&mut broker)
            .await
            .expect("owner recheck is sent");
        assert_eq!(recheck["method"], "thread-owner-discovery");
        send_owner_response(&mut broker, &recheck, OWNER_ID).await;

        let mutation = read_frame(&mut broker)
            .await
            .expect("mutation is sent to the verified owner");
        let response = json!({
            "type": "response",
            "requestId": mutation["requestId"],
            "resultType": "success",
            "handledByClientId": OWNER_ID,
            "result": native_result
        });
        write_frame(&mut broker, &response)
            .await
            .expect("fixture mutation response is written");
    });

    let result = session.execute(&command).await;
    broker_task.await.expect("fixture broker task completes");
    result
}

#[tokio::test]
async fn owner_change_during_recheck_prevents_any_mutation_request() {
    let (mut session, mut broker) = session_pair();
    let broker_task = tokio::spawn(async move {
        let request = read_frame(&mut broker)
            .await
            .expect("owner recheck is sent");
        assert_eq!(request["method"], "thread-owner-discovery");
        assert!(request.get("targetClientId").is_none());
        send_owner_response(&mut broker, &request, OTHER_OWNER_ID).await;

        let next = timeout(Duration::from_secs(2), read_frame(&mut broker))
            .await
            .expect("session closes instead of sending a mutation")
            .expect_err("no mutation frame is sent after owner changes");
        assert_eq!(next, DesktopError::Disconnected);
    });

    let result = session.execute(&start_command()).await;
    assert!(matches!(result, Err(DesktopError::OwnerUnavailable)));
    broker_task.await.expect("fixture broker task completes");
}

#[tokio::test]
async fn each_dispatch_revalidates_references_before_owner_recheck() {
    let fixture = ReferenceFixture::new();
    let reference_path = fixture.file("fixture-reference.txt", b"synthetic selected reference");
    let reference = NativeReference::selected_file(reference_path.clone())
        .expect("synthetic file reference is selected and captured");
    fs::write(&reference_path, b"x").expect("synthetic reference is truncated after capture");

    let (mut session, mut broker) = session_pair();
    let broker_task = tokio::spawn(async move {
        let next = timeout(Duration::from_secs(2), read_frame(&mut broker))
            .await
            .expect("reference guard closes the fixture session without sending a request")
            .expect_err("no owner or mutation request follows an invalid reference");
        assert_eq!(next, DesktopError::Disconnected);
    });

    for client_id in [
        "88888888-8888-4888-8888-888888888888",
        "99999999-9999-4999-8999-999999999999",
    ] {
        let command = DesktopCommand::Start(MessageInput {
            client_id: client_id.into(),
            text: "fixture reference validation".into(),
            references: vec![reference.clone()],
        });
        assert!(matches!(
            session.execute(&command).await,
            Err(DesktopError::RequestExpired)
        ));
    }

    drop(session);
    broker_task.await.expect("fixture broker task completes");
}

#[tokio::test]
async fn start_and_steer_require_nested_native_result_envelopes() {
    let start = execute_with_native_result(
        start_command(),
        idle_state(),
        json!({"result": {"turn": {"id": TURN_ID}}}),
    )
    .await
    .expect("nested native Start result is recognized");
    assert_eq!(start.turn_id.as_deref(), Some(TURN_ID));

    let steer = execute_with_native_result(
        steer_command(),
        active_state(),
        json!({"result": {"turnId": TURN_ID}}),
    )
    .await
    .expect("nested native Steer result is recognized");
    assert_eq!(steer.turn_id.as_deref(), Some(TURN_ID));

    let single_layer_start = execute_with_native_result(
        start_command(),
        idle_state(),
        json!({"turn": {"id": TURN_ID}}),
    )
    .await;
    assert!(matches!(
        single_layer_start,
        Err(DesktopError::OutcomeUnknown)
    ));

    let single_layer_steer =
        execute_with_native_result(steer_command(), active_state(), json!({"turnId": TURN_ID}))
            .await;
    assert!(matches!(
        single_layer_steer,
        Err(DesktopError::OutcomeUnknown)
    ));
}

#[tokio::test]
async fn state_patches_received_while_waiting_for_response_are_applied() {
    let (mut session, mut broker) = session_pair();
    let broker_task = tokio::spawn(async move {
        let recheck = read_frame(&mut broker)
            .await
            .expect("owner recheck is sent");
        assert_eq!(recheck["method"], "thread-owner-discovery");
        send_owner_response(&mut broker, &recheck, OWNER_ID).await;

        let mutation = read_frame(&mut broker)
            .await
            .expect("start request is sent after the owner recheck");
        assert_eq!(mutation["method"], "thread-follower-update-thread-settings");
        assert_eq!(mutation["targetClientId"], OWNER_ID);

        let patch = json!({
            "type": "broadcast",
            "sourceClientId": OWNER_ID,
            "method": "thread-stream-state-changed",
            "version": 11,
            "params": {
                "conversationId": THREAD_ID,
                "change": {
                    "type": "patches",
                    "revision": 2,
                    "baseRevision": 1,
                    "patches": [
                        {"op": "replace", "path": ["threadRuntimeStatus"], "value": {"type": "active"}},
                        {"op": "replace", "path": ["turns"], "value": [{"id": TURN_ID, "status": "inProgress", "items": []}]},
                        {"op": "replace", "path": ["requests"], "value": [{
                            "id": "fixture-pending-request",
                            "method": "item/commandExecution/requestApproval",
                            "params": {"threadId": THREAD_ID, "turnId": TURN_ID}
                        }]}
                    ]
                }
            }
        });
        write_frame(&mut broker, &patch)
            .await
            .expect("fixture state patch is written before the response");

        let response = json!({
            "type": "response",
            "requestId": mutation["requestId"],
            "resultType": "success",
            "handledByClientId": OWNER_ID,
            "result": {"applied": true}
        });
        write_frame(&mut broker, &response)
            .await
            .expect("fixture mutation response is written");
    });

    let result = session
        .execute(&settings_command())
        .await
        .expect("known native settings response is returned");
    assert!(result.turn_id.is_none());
    assert_eq!(session.state().revision(), Some(2));
    assert_eq!(session.state().runtime(), RuntimeState::Waiting);
    assert_eq!(
        session
            .state()
            .pending_request(
                &json!("fixture-pending-request"),
                "item/commandExecution/requestApproval"
            )
            .expect("request from interleaved patch remains available")["params"]["turnId"],
        TURN_ID
    );
    broker_task.await.expect("fixture broker task completes");
}

#[tokio::test]
async fn missing_mutation_response_is_reported_as_outcome_unknown() {
    let (mut session, mut broker) = session_pair();
    let broker_task = tokio::spawn(async move {
        let recheck = read_frame(&mut broker)
            .await
            .expect("owner recheck is sent");
        assert_eq!(recheck["method"], "thread-owner-discovery");
        send_owner_response(&mut broker, &recheck, OWNER_ID).await;

        let mutation = read_frame(&mut broker)
            .await
            .expect("mutation request is sent to the verified owner");
        assert_eq!(mutation["method"], "thread-follower-start-turn");
        // Dropping the broker side leaves the mutation outcome unconfirmed.
    });

    let result = session.execute(&start_command()).await;
    assert!(matches!(result, Err(DesktopError::OutcomeUnknown)));
    broker_task.await.expect("fixture broker task completes");
}
