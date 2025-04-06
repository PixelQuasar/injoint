mod test;

use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct AxumWSSink {
    tx: mpsc::Sender<Result<Message, axum::Error>>,
}

#[async_trait]
impl SinkAdapter for AxumWSSink {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message_text = serde_json::to_string(&response)?;
        self.tx
            .send(Ok(Message::Text(message_text.into())))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

pub struct AxumWSStream {
    stream: SplitStream<WebSocket>,
}

#[async_trait]
impl StreamAdapter for AxumWSStream {
    async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>> {
        let message = self.stream.next().await.unwrap()?;
        let message = match message {
            Message::Text(text) => text,
            _ => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid data",
                )))
            }
        };
        let message = serde_json::from_slice((&message).as_ref())?;
        Ok(message)
    }
}

pub struct AxumWSJoint<R: Dispatchable + 'static> {
    joint: Arc<AbstractJoint<R, AxumWSSink>>,
    tcp_listener: Option<TcpListener>,
}

impl<R: Dispatchable + 'static> AxumWSJoint<R> {
    pub fn new(default_reducer: R) -> Self {
        AxumWSJoint {
            joint: Arc::new(AbstractJoint::new(default_reducer)),
            tcp_listener: None,
        }
    }

    pub async fn bind(&mut self, addr: &str) {
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        self.tcp_listener = Some(tcp_listener);
    }

    pub async fn ws_handler(
        ws: WebSocketUpgrade,
        joint: Arc<AbstractJoint<R, AxumWSSink>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(|socket| async move {
            let (mut websocket_sink, websocket_stream) = socket.split();

            let (tx, mut rx) = mpsc::channel::<Result<Message, axum::Error>>(100);

            tokio::spawn(async move {
                while let Some(result) = rx.recv().await {
                    match result {
                        Ok(msg) => {
                            if websocket_sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Error received in AxumWSSink channel: {}", e);
                            let _ = websocket_sink.close().await;
                            break;
                        }
                    }
                }
                let _ = websocket_sink.close().await;
            });

            let mut stream_adapter = AxumWSStream {
                stream: websocket_stream,
            };

            let sink_adapter = AxumWSSink { tx };

            joint
                .clone()
                .handle_stream(&mut stream_adapter, sink_adapter)
                .await;
        })
    }

    pub fn attach_router(&self, path: &str, router: Router) -> Router {
        let joint = self.joint.clone();
        router.route(path, get(move |ws| AxumWSJoint::ws_handler(ws, joint)))
    }

    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
