/// Provides joint implementations for Axum applications using WebSockets.
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

/// An implementation of [`SinkAdapter`] for sending messages over an Axum WebSocket connection.
///
/// This struct uses an internal `tokio::sync::mpsc::channel` to allow cloning and
/// sending messages asynchronously to the underlying WebSocket sink task.
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

/// An implementation of [`StreamAdapter`] for receiving messages from an Axum WebSocket connection.
///
/// This struct represents the stream of messages received from an Axum WebSocket connection.
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

/// An `injoint` joint specifically designed for integration with the Axum web framework.
///
/// It manages WebSocket connections and integrates with the `injoint` core broadcasting
/// and state management logic.
///
/// `R` represents the application-specific `Dispatchable` state/reducer.
impl<R: Dispatchable + 'static> AxumWSJoint<R> {
    /// Creates a new `AxumJoint`.
    ///
    /// Requires a default instance of the application's `Dispatchable` reducer.
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

    /// Axum WebSocket handler function.
    ///
    /// This function should be used with `axum::routing::get` to handle WebSocket upgrade requests.
    /// It manages the WebSocket lifecycle, splitting it into a sink and stream, and passes
    /// them to the underlying `AbstractJoint`.
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

    /// Attaches the WebSocket handler (`ws_handler`) to an Axum router at the specified path.
    ///
    /// This is a convenience method for setting up the WebSocket route.
    pub fn attach_router(&self, path: &str, router: Router) -> Router {
        let joint = self.joint.clone();
        router.route(path, get(move |ws| AxumWSJoint::ws_handler(ws, joint)))
    }

    /// Allows dispatching an action to the joint\'s reducer from outside the WebSocket context.
    ///
    /// This can be useful for triggering state changes from other parts of the application
    /// (e.g., HTTP request handlers, background jobs).
    ///
    /// # Arguments
    /// * `client_id` - The ID of the client on whose behalf the action is dispatched.
    ///                 Note: The client must exist and be in a room for the dispatch to succeed.
    /// * `action` - A string slice representing the action to be dispatched (must be JSON serializable
    ///              according to the `Dispatchable::Action` type).
    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
