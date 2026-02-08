use axum::{
    extract::{
        State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::Html,
    response::IntoResponse,
};
use futures::sink::Sink;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::broadcast;

use crate::api::{ApiServer, StateUpdate};

pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, api))
}

async fn websocket_connection(socket: WebSocket, api: ApiServer) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = api.broadcast_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        let _ = pump_broadcast_updates(&mut sender, &mut rx).await;
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

async fn pump_broadcast_updates<S>(
    sender: &mut S,
    rx: &mut broadcast::Receiver<StateUpdate>,
) -> Result<(), S::Error>
where
    S: Sink<axum::extract::ws::Message> + Unpin,
{
    use axum::extract::ws::Message;

    while let Ok(update) = rx.recv().await {
        let json = serde_json::to_string(&update).unwrap_or_default();
        if sender.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
    Ok(())
}

pub(crate) async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ws::Message;
    use futures::task::{Context, Poll};
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_ui_serves_embedded_html() {
        let Html(body) = ui().await;
        assert!(body.contains("<html"));
    }

    #[derive(Clone, Default)]
    struct RecordingSink {
        fail_on_send: bool,
        sent: Arc<Mutex<Vec<Message>>>,
    }

    impl Sink<Message> for RecordingSink {
        type Error = std::io::Error;

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            if self.fail_on_send {
                return Err(std::io::Error::other("send failed"));
            }
            self.sent.lock().expect("sent lock").push(item);
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_pump_broadcast_updates_sends_and_stops_when_channel_closes() {
        let (tx, mut rx) = broadcast::channel(4);
        let mut sink = RecordingSink::default();
        tx.send(StateUpdate::RunUpdated {
            run_id: "run-1".to_string(),
            status: "running".to_string(),
        })
        .expect("send update");
        drop(tx);

        pump_broadcast_updates(&mut sink, &mut rx)
            .await
            .expect("pump updates");
        let sent = sink.sent.lock().expect("sent lock");
        assert_eq!(sent.len(), 1);
    }

    #[tokio::test]
    async fn test_pump_broadcast_updates_breaks_on_send_error() {
        let (tx, mut rx) = broadcast::channel(4);
        let mut sink = RecordingSink {
            fail_on_send: true,
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        tx.send(StateUpdate::RunUpdated {
            run_id: "run-2".to_string(),
            status: "running".to_string(),
        })
        .expect("send update");
        drop(tx);

        pump_broadcast_updates(&mut sink, &mut rx)
            .await
            .expect("pump should stop cleanly");
        let sent = sink.sent.lock().expect("sent lock");
        assert!(sent.is_empty());
    }

    #[tokio::test]
    async fn test_recording_sink_close_calls_poll_close() {
        let mut sink = RecordingSink::default();
        sink.close().await.expect("close should succeed");
    }
}
