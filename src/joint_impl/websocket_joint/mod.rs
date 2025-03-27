use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use crate::joint::{AbstractJoint, Joint};
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tungstenite::{Message, Utf8Bytes};

struct WSSink {
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
}
#[async_trait]
impl SinkAdapter for WSSink {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = Message::Text(Utf8Bytes::from(serde_json::to_string(&response)?));
        self.sink.send(message).await.map_err(|e| Box::new(e) as _)
    }
}

struct WSStream {
    stream: SplitStream<WebSocketStream<TcpStream>>,
}

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

pub struct WebsocketJoint<R: Dispatchable + 'static> {
    joint: Arc<Mutex<AbstractJoint<R, WSSink>>>,
    tcp_listener: Option<TcpListener>,
}

impl<R: Dispatchable + 'static> WebsocketJoint<R> {
    pub fn new() -> Self {
        WebsocketJoint {
            joint: Arc::new(Mutex::new(AbstractJoint::new())),
            tcp_listener: None,
        }
    }

    pub async fn bind(&mut self, addr: &str) {
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        self.tcp_listener = Some(tcp_listener);
    }

    async fn stream_worker(stream: TcpStream, joint: Arc<Mutex<AbstractJoint<R, WSSink>>>)
    where
        R: Dispatchable,
    {
        let websocket = accept_async(stream).await.unwrap();

        let (sender, receiver) = websocket.split();

        let mut sender_wrapper = WSStream { stream: receiver };

        let sink_wrapper = WSSink { sink: sender };

        let mut joint_guard = joint.lock().await;

        joint_guard
            .handle_stream(&mut sender_wrapper, sink_wrapper)
            .await;
    }
}

#[async_trait]
impl<R: Dispatchable + 'static> Joint for WebsocketJoint<R> {
    // initialize polling loop
    async fn listen(&mut self) {
        loop {
            if let Some(tcp_listener) = &self.tcp_listener {
                let (stream, _) = tcp_listener.accept().await.unwrap();

                tokio::spawn(Self::stream_worker(stream, self.joint.clone()));
            } else {
                panic!("Websocket joint poll error: no listener bound");
            }
        }
    }
}
