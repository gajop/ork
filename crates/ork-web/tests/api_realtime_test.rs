use std::sync::Arc;
use std::time::Duration;

use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use ork_state::SqliteDatabase;
use ork_web::api::{ApiServer, StateUpdate, build_router};

fn free_local_addr() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free tcp port");
    listener.local_addr().expect("local addr")
}

#[tokio::test]
async fn test_websocket_receives_broadcast_updates() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let api = ApiServer::new(db);
    let tx = api.broadcast_tx.clone();
    let app = build_router(api);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve websocket app");
    });

    let ws_url = format!("ws://{addr}/ws");
    let (mut socket, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect websocket");

    tx.send(StateUpdate::RunUpdated {
        run_id: "run-1".to_string(),
        status: "running".to_string(),
    })
    .expect("broadcast update");

    let incoming = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("websocket receive timeout")
        .expect("websocket frame")
        .expect("websocket text");

    let payload = match incoming {
        Message::Text(text) => text.to_string(),
        other => panic!("expected text websocket frame, got {other:?}"),
    };
    let json: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
    assert_eq!(json["type"], "run_updated");
    assert_eq!(json["run_id"], "run-1");
    assert_eq!(json["status"], "running");

    socket
        .send(Message::Text("ping".to_string().into()))
        .await
        .expect("send client message");
    socket
        .send(Message::Text("second-message".to_string().into()))
        .await
        .expect("send second client message");
    sleep(Duration::from_millis(30)).await;
    socket.close(None).await.expect("close websocket");

    server.abort();
}

#[tokio::test]
async fn test_websocket_sender_stops_after_client_disconnect() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let api = ApiServer::new(db);
    let tx = api.broadcast_tx.clone();
    let app = build_router(api);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve websocket app");
    });

    let ws_url = format!("ws://{addr}/ws");
    let (socket, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect websocket");

    drop(socket);
    for idx in 0..4 {
        let _ = tx.send(StateUpdate::RunUpdated {
            run_id: format!("run-disconnected-{idx}"),
            status: "running".to_string(),
        });
        sleep(Duration::from_millis(20)).await;
    }

    let response = reqwest::get(format!("http://{addr}/"))
        .await
        .expect("server should still respond");
    assert!(response.status().is_success());

    server.abort();
}

#[tokio::test]
async fn test_api_server_serve_handles_http_requests() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let addr = free_local_addr();
    let api = ApiServer::new(db);
    let handle = api.serve(addr).await;

    let response = timeout(Duration::from_secs(3), async {
        loop {
            match reqwest::get(format!("http://{addr}/")).await {
                Ok(response) => break response,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("http timeout");
    assert!(response.status().is_success());
    let body = response.text().await.expect("response text");
    assert!(body.contains("<html"));

    handle.abort();
}
