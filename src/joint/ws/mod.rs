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

#[derive(Clone)]
struct WSSink {
    tx: mpsc::Sender<Result<Message, tungstenite::Error>>,
}

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
    local_addr: Option<SocketAddr>,
}

impl<R: Dispatchable + Send + 'static> WebsocketJoint<R> {
    pub fn new(default_reducer: R) -> Self {
        WebsocketJoint {
            joint: Arc::new(AbstractJoint::new(default_reducer)),
            tcp_listener: None,
            local_addr: None,
        }
    }

    pub async fn bind_addr(&mut self, addr: &str) -> io::Result<()> {
        let tcp_listener = TcpListener::bind(addr).await?;
        self.local_addr = Some(tcp_listener.local_addr()?);
        self.tcp_listener = Some(tcp_listener);
        Ok(())
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
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

    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
