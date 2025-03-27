use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, WebSocketStream};

struct AxumWSSink {
    sink: SplitSink<WebSocket, Message>,
}
#[async_trait]
impl SinkAdapter for AxumWSSink {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = Message::Text(Utf8Bytes::from(serde_json::to_string(&response)?));
        self.sink.send(message).await.map_err(|e| Box::new(e) as _)
    }
}

struct AxumWSStream {
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
    joint: AbstractJoint<R, AxumWSSink>,
    tcp_listener: Option<TcpListener>,
}

impl<R: Dispatchable + 'static> AxumWSJoint<R> {
    pub fn new() -> Self {
        AxumWSJoint {
            joint: AbstractJoint::new(),
            tcp_listener: None,
        }
    }

    pub async fn bind(&mut self, addr: &str) {
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        self.tcp_listener = Some(tcp_listener);
    }

    // pub async fn handler(&mut self, ws: WebSocketUpgrade) -> impl IntoResponse {
    //     ws.on_upgrade(|socket| async move {
    //         let (mut sender, mut receiver) = socket.split();
    //
    //         let mut sender_wrapper = AxumWSStream { stream: receiver };
    //
    //         let receiver_wrapper = AxumWSSink { sink: sender };
    //
    //         let mut joint_guard = self.joint.lock().await;
    //
    //         joint_guard
    //             .handle_stream(&mut sender_wrapper, receiver_wrapper)
    //             .await;
    //     })
    // }
}
