use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
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
    joint: Arc<AbstractJoint<R, WSSink>>,
    tcp_listener: Option<TcpListener>,
}

impl<R: Dispatchable + Send + 'static> WebsocketJoint<R> {
    pub fn new() -> Self {
        WebsocketJoint {
            joint: Arc::new(AbstractJoint::new()),
            tcp_listener: None,
        }
    }

    pub async fn bind_listener(&mut self, listener: TcpListener) {
        self.tcp_listener = Some(listener);
    }

    pub async fn bind_addr(&mut self, addr: &str) {
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        self.tcp_listener = Some(tcp_listener);
    }

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

    async fn stream_worker(stream: TcpStream, joint: Arc<AbstractJoint<R, WSSink>>)
    where
        R: Dispatchable + Send + 'static,
    {
        let websocket = accept_async(stream).await.unwrap();
        let (sink, stream) = websocket.split();

        let mut sender_wrapper = WSStream { stream };

        let receiver_adapter = WSSink { sink };

        joint
            .handle_stream(&mut sender_wrapper, receiver_adapter)
            .await;
    }

    pub async fn dispatch(
        &self,
        client_id: u64,
        action: R::Action,
    ) -> Result<ActionResponse<R::Response>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
