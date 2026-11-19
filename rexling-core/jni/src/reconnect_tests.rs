//! Reconnecting to a session the server already destroyed; loopback peer only.
use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{Request, Response},
    },
};

const LIVE: &str = "session:live";
const DEAD: &str = "session:gone";

/// Drive one reconnect handshake. `attach` decides whether the viewed session is
/// still there, gone, or unreachable for a reason that must stay retryable.
async fn reconnect(attach: Option<(String, u16)>) -> (Actor, Arc<Mutex<Shared>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint =
        Endpoint::new(&format!("http://{}", listener.local_addr().unwrap()), Duration::from_secs(3))
            .unwrap();
    let methods = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded = methods.clone();
    let peer = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = accept_hdr_async(socket, |request: &Request, mut response: Response| {
            response.headers_mut().insert("Sec-WebSocket-Protocol", request.headers()["Sec-WebSocket-Protocol"].clone());
            Ok(response)
        })
        .await
        .unwrap();
        ws.next().await.unwrap().unwrap();
        ws.send(Message::Text(
            json!({"type":"welcome","protocol":"rex.control.v1","api_revision":8,
                "limits":{"max_message_bytes":1_048_576},"client":{"id":"client:test","resumed":false}})
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
        while let Some(Ok(message)) = ws.next().await {
            if message.is_close() {
                break;
            }
            if !message.is_text() {
                continue;
            }
            let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            let method = request["method"].as_str().unwrap().to_owned();
            recorded.lock().unwrap().push(method.clone());
            let mut response = json!({"type":"response","id":request["id"],"payload":{}});
            match method.as_str() {
                "session.attach_all" | "session.list" => {
                    response["payload"] = json!({"sessions":[{"session_id":LIVE,"label":"Live"}]})
                }
                "server.whoami" => response["payload"] = json!({"principal":{"display":"tester"}}),
                "session.attach" => match &attach {
                    None => response["payload"] =
                        json!({"view":{"windows":[{"window_id":"w","layers":[{"kind":"tiled","blocks":[]}]}]}}),
                    Some((code, status)) => {
                        response["error"] = json!({"code":code,"message":"fixture","status":status})
                    }
                },
                other => panic!("unexpected control call: {other}"),
            }
            ws.send(Message::Text(response.to_string().into())).await.unwrap();
        }
    });
    let control = Control::connect(endpoint.clone(), None, Trace::default()).await.unwrap();
    let shared = Arc::new(Mutex::new(Shared {
        revision: 0,
        json: String::new(),
        pending_previews: BTreeMap::new(),
        frame: None,
        key: String::new(),
        wire: crate::updates::WireBase::default(),
        activity: Arc::default(),
    }));
    let mut actor = Actor::new(shared.clone(), Arc::new(crate::updates::Updates::default()));
    actor.endpoint = Some(endpoint);
    actor.phase = "reconnecting".into();
    actor.selected_session = DEAD.into();
    actor.selected_block = "block:b".into();
    actor.controlled = true;
    actor.windows = json!([{"id":"w","label":"Tab","blocks":[{"id":"block:b","label":"Terminal"}]}]);
    actor.connected(control).await.ok();
    // The peer only exits when it sees the close frame, so end the handshake cleanly.
    if let Some(control) = actor.control.as_mut() {
        let _ = control.close().await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(10), peer).await;
    (actor, shared)
}

#[test]
fn a_session_that_ended_while_away_reconnects_without_the_viewed_session() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let (actor, shared) = runtime.block_on(reconnect(Some(("session_not_found".to_owned(), 404))));
    // The handshake succeeded, so the client must not fall back into a reconnect loop.
    assert_eq!(actor.phase, "connected");
    assert!(actor.selected_session.is_empty() && actor.selected_block.is_empty());
    assert!(actor.windows.as_array().unwrap().is_empty());
    assert!(!actor.controlled);
    assert_eq!(actor.stale_session, DEAD);
    assert_eq!(actor.sessions.as_array().unwrap().len(), 1);
    let published: Value = serde_json::from_str(&shared.lock().unwrap().json).unwrap();
    assert_eq!(published["phase"], "connected");
    assert_eq!(published["selectedSession"], "");
    assert_eq!(published["staleSession"], DEAD);
    assert_eq!(published["sessions"].as_array().unwrap().len(), 1);
}

#[test]
fn an_attach_failure_that_is_not_a_missing_session_still_asks_for_a_retry() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let (actor, _shared) = runtime.block_on(reconnect(Some(("internal".to_owned(), 500))));
    // Nothing was cleared: recover() keeps the target so the next backoff can restore it.
    assert_eq!(actor.selected_session, DEAD);
    assert_eq!(actor.selected_block, "block:b");
    assert!(actor.stale_session.is_empty());
}
