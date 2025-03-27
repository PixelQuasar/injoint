use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use crate::joint::AbstractJoint;
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

pub struct WebsocketJoint<R: Dispatchable + Send + 'static> {
    joint: Arc<Mutex<AbstractJoint<R, WSSink>>>,
}

impl<R: Dispatchable + Send + 'static> WebsocketJoint<R> {
    pub fn new() -> Self {
        WebsocketJoint {
            joint: Arc::new(Mutex::new(AbstractJoint::new())),
        }
    }

    // initialize polling loop
    pub async fn poll(&mut self, tcp_listener: TcpListener) {
        loop {
            let (stream, _) = tcp_listener.accept().await.unwrap();

            tokio::spawn(Self::stream_worker(stream, self.joint.clone()));
        }
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

async fn create_message_from_stream(stream: &mut TcpStream) -> io::Result<JointMessage> {
    let mut buffer = vec![0; 1024];
    let n = stream.read(&mut buffer).await?;

    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Connection closed",
        ));
    }

    let client_id = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
    let content = String::from_utf8_lossy(&buffer[8..n]).to_string();
    let message = serde_json::from_str(&content).unwrap();

    Ok(JointMessage {
        client_token: client_id.to_string(),
        message,
    })
}
