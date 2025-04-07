/// This module provides a WebSocket joint implementation for the injoint library.
///
/// It allows for real-time room-split communication between clients and the server using WebSocket connections.
mod test;

use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tungstenite::Message;

/// `WSSink` is a struct that implements the `SinkAdapter` trait for websocket joint implementation.
#[derive(Clone)]
struct WSSink {
    tx: mpsc::Sender<Result<Message, tungstenite::Error>>,
}

/// `StreamAdapter` is a struct that implements the `StreamAdapter` trait for websocket joint implementation.
#[async_trait]
impl SinkAdapter for WSSink {
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

/// `WSStream` is a struct that implements the `StreamAdapter` trait for websocket joint implementation.
struct WSStream {
    stream: SplitStream<WebSocketStream<TcpStream>>,
}

/// `StreamAdapter` is a trait that defines the interface for receiving messages.
#[async_trait]
impl StreamAdapter for WSStream {
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

/// `WebsocketJoint` is a struct that implements websocket joint functionality.
///
/// It is a wrapper around the `AbstractJoint` struct and provides methods to bind to a TCP address and listen for incoming connections.
/// It also provides a method to dispatch actions to the joint.
///
/// # example
/// lookup `examples/shell-chat-app/server` at [injoint repository](https://github.com/PixelQuasar/injoint) for usage.
pub struct WebsocketJoint<R: Dispatchable + Send + 'static> {
    joint: Arc<AbstractJoint<R, WSSink>>,
    tcp_listener: Option<TcpListener>,
    local_addr: Option<SocketAddr>,
}

impl<R: Dispatchable + Send + 'static> WebsocketJoint<R> {
    /// Creates a new `WebsocketJoint` instance with the given default reducer.
    pub fn new(default_reducer: R) -> Self {
        WebsocketJoint {
            joint: Arc::new(AbstractJoint::new(default_reducer)),
            tcp_listener: None,
            local_addr: None,
        }
    }

    /// Binds the joint to the given address.
    ///
    /// This method creates a TCP listener and sets the local address of the joint.
    ///
    /// # Arguments
    /// * `addr` - The address to bind the joint to.
    pub async fn bind_addr(&mut self, addr: &str) -> io::Result<()> {
        let tcp_listener = TcpListener::bind(addr).await?;
        self.local_addr = Some(tcp_listener.local_addr()?);
        self.tcp_listener = Some(tcp_listener);
        Ok(())
    }

    /// Returns the local address of the joint.
    ///
    /// This method returns an `Option<SocketAddr>` that contains the local address of the joint.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Listens for incoming connections on the bound address.
    ///
    /// This method accepts incoming TCP connections and spawns a new task for each connection.
    ///
    /// # Panics
    /// * This method panics if the joint is not bound to an address.
    pub async fn listen(&mut self) {
        loop {
            if let Some(tcp_listener) = &self.tcp_listener {
                let (stream, _) = tcp_listener.accept().await.unwrap();

                tokio::spawn(Self::stream_worker(stream, self.joint.clone()));
            } else {
                panic!("Websocket joint poll error: no listener bound");
            }
        }
    }

    /// Handles a new incoming connection.
    ///
    /// This method accepts a TCP stream and spawns a new task to handle the connection.
    ///
    /// # Arguments
    /// * `stream` - The TCP stream representing the incoming connection.
    /// * `joint` - The joint instance to handle the connection.
    ///
    /// # Panics
    /// * This method panics if the joint is not bound to an address.
    async fn stream_worker(stream: TcpStream, joint: Arc<AbstractJoint<R, WSSink>>)
    where
        R: Dispatchable + Send + 'static,
    {
        let websocket = accept_async(stream).await.unwrap();
        let (mut websocket_sink, websocket_stream) = websocket.split();

        let (tx, mut rx) = mpsc::channel::<Result<Message, tungstenite::Error>>(100);

        tokio::spawn(async move {
            while let Some(result) = rx.recv().await {
                match result {
                    Ok(msg) => {
                        if websocket_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = websocket_sink.close().await;
                        eprintln!(
                            "[Sink Task {:?}] Sink closed due to channel error, breaking loop.",
                            std::thread::current().id()
                        );
                        break;
                    }
                }
            }
        });

        let mut stream_adapter = WSStream {
            stream: websocket_stream,
        };

        let sink_adapter = WSSink { tx };

        joint.handle_stream(&mut stream_adapter, sink_adapter).await;
    }

    /// Dispatches an action to the joint.
    ///
    /// This method takes a `client_id` and an `action` string as parameters.
    ///
    /// # Arguments
    /// * `client_id` - The ID of the client dispatching the action.
    /// * `action` - The action string to be dispatched.
    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
