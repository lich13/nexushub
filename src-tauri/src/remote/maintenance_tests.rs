use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const RESPONSE_LIMIT: usize = 32 * 1024 * 1024;
const FIXTURE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
enum Framing {
    ContentLength,
    UntilClose,
    Chunked,
}

struct ResponseServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ResponseServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn response_server(status: &str, body: Vec<u8>, framing: Framing) -> ResponseServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let framing_header = match framing {
        Framing::ContentLength => format!("Content-Length: {}\r\n", body.len()),
        Framing::UntilClose => String::new(),
        Framing::Chunked => "Transfer-Encoding: chunked\r\n".into(),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\n{framing_header}\r\n"
    );
    let task = tokio::spawn(async move {
        tokio::time::timeout(FIXTURE_TIMEOUT, async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let length = socket.read(&mut chunk).await.unwrap();
                assert!(length > 0, "fixture request ended before its body");
                request.extend_from_slice(&chunk[..length]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let body_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + body_length {
                        break;
                    }
                }
                assert!(
                    request.len() < 16 * 1024,
                    "fixture request exceeded its bound"
                );
            }
            // Oversized responses may be rejected before the fixture finishes writing.
            if socket.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            for chunk in body.chunks(64 * 1024) {
                if matches!(framing, Framing::Chunked)
                    && socket
                        .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                        .await
                        .is_err()
                {
                    return;
                }
                if socket.write_all(chunk).await.is_err() {
                    return;
                }
                if matches!(framing, Framing::Chunked) && socket.write_all(b"\r\n").await.is_err() {
                    return;
                }
            }
            if matches!(framing, Framing::Chunked) {
                let _ = socket.write_all(b"0\r\n\r\n").await;
            }
            let _ = socket.shutdown().await;
        })
        .await
        .expect("fixture server timed out");
    });
    ResponseServer { base, task }
}

fn connection_without_keychain_access() -> RemoteConnections {
    RemoteConnections {
        state: Arc::new(Mutex::new(ConnectionState::default())),
        path: PathBuf::new(),
        http: Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(FIXTURE_TIMEOUT)
            .build()
            .unwrap(),
        credentials: CredentialAccess::default(),
    }
}

fn json_body_of_size(bytes: usize) -> Vec<u8> {
    let mut body = b"{\"data\":\"".to_vec();
    body.resize(bytes - 2, b'A');
    body.extend_from_slice(b"\"}");
    assert_eq!(body.len(), bytes);
    body
}

#[tokio::test(flavor = "current_thread")]
async fn remote_error_responses_never_echo_the_supplied_api_key() {
    let connection = connection_without_keychain_access();
    let key = format!("nhk_{}", "a1".repeat(32));
    for (status, body) in [
        (
            "400 Bad Request",
            serde_json::to_vec(&json!({"error":format!("fixture rejected {key}")})).unwrap(),
        ),
        (
            "500 Internal Server Error",
            serde_json::to_vec(&json!({
                "error":format!("fixture before\n{key}\nfixture after"),
                "nested":{"credential":key}
            }))
            .unwrap(),
        ),
        (
            "503 Service Unavailable",
            format!("{{\"error\":\"{key}\"").into_bytes(),
        ),
    ] {
        let mut server = response_server(status, body, Framing::ContentLength).await;
        let result = connection
            .call(&server.base, &key, "system.capabilities", &json!({}))
            .await;
        (&mut server.task).await.unwrap();
        let Err(error) = result else {
            panic!("malicious fixture response was accepted");
        };
        assert!(!error.is_empty());
        assert!(!error.contains(&key), "remote error exposed the API key");
        assert!(
            !error.contains(&key[4..]),
            "remote error exposed the API key payload"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn remote_response_accepts_exactly_32_mib_with_all_http_framings() {
    let connection = connection_without_keychain_access();
    for framing in [
        Framing::ContentLength,
        Framing::UntilClose,
        Framing::Chunked,
    ] {
        let mut server =
            response_server("200 OK", json_body_of_size(RESPONSE_LIMIT), framing).await;
        let result = connection
            .call(
                &server.base,
                "fixture",
                "sessions.attachmentRead",
                &json!({}),
            )
            .await;
        (&mut server.task).await.unwrap();
        let value = result.expect("the exact response size limit was rejected");
        assert_eq!(value["data"].as_str().unwrap().len(), RESPONSE_LIMIT - 11);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn remote_response_rejects_32_mib_plus_one_even_without_content_length() {
    let connection = connection_without_keychain_access();
    for framing in [
        Framing::ContentLength,
        Framing::UntilClose,
        Framing::Chunked,
    ] {
        let mut server =
            response_server("200 OK", json_body_of_size(RESPONSE_LIMIT + 1), framing).await;
        let result = connection
            .call(
                &server.base,
                "fixture",
                "sessions.attachmentRead",
                &json!({}),
            )
            .await;
        (&mut server.task).await.unwrap();
        let Err(error) = result else {
            panic!("an oversized response was accepted");
        };
        assert_eq!(error, "远程响应超出大小限制");
    }
}
