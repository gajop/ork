use axum::{
    extract::{
        State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::Html,
    response::IntoResponse,
};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use crate::api::ApiServer;

pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, api))
}

async fn websocket_connection(socket: WebSocket, api: ApiServer) {
    use axum::extract::ws::Message;

    let (mut sender, mut receiver) = socket.split();
    let mut rx = api.broadcast_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            let json = serde_json::to_string(&update).unwrap_or_default();
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Client messages ignored for now (could add ping/pong here)
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

pub(crate) async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ui_serves_embedded_html() {
        let Html(body) = ui().await;
        assert!(body.contains("<html"));
    }
}
