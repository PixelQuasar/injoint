use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::joint::axum::AxumWSJoint;
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use crate::utils::types::{Broadcastable, Receivable};
use async_trait::async_trait;
use axum::Router;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    async fn dispatch(
        &self,
        client_id: u64,
        action: R::Action,
    ) -> Result<ActionResponse<R::Response>, String> {
        self.joint.dispatch(client_id, action).await
    }
}

// usage example
#[derive(Serialize, Debug, Clone)]
struct TextMessage {
    id: u64,
    author: u64,
    content: String,
    pinned: bool,
}

#[derive(Serialize, Debug, Clone, Default)]
struct DemoState {
    messages: Vec<TextMessage>,
    user_names: HashMap<u64, String>,
}
impl Broadcastable for DemoState {}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum Actions {
    IdentifyUser(String), // client name
    SendMessage(String),  // message content
    DeleteMessage(u64),   // message id
    PinMessage(u64),      // message id
}
impl Receivable for Actions {}

#[derive(Default)]
struct MyJointReducer {
    state: DemoState,
}

impl MyJointReducer {
    async fn action_identify_user(
        &mut self,
        client_id: u64,
        name: String,
    ) -> Result<String, String> {
        self.state.user_names.insert(client_id, name.clone());
        Ok(name)
    }

    async fn action_send_message(
        &mut self,
        client_id: u64,
        content: String,
    ) -> Result<String, String> {
        let id = rand::rng().random::<u64>();
        let message = TextMessage {
            id,
            content,
            author: client_id,
            pinned: false,
        };
        self.state.messages.push(message);
        Ok(id.to_string())
    }

    async fn action_delete_message(
        &mut self,
        client_id: u64,
        message_id: u64,
    ) -> Result<String, String> {
        self.state.messages.retain(|msg| msg.id != message_id);
        Ok(message_id.to_string())
    }

    async fn action_pin_message(
        &mut self,
        client_id: u64,
        message_id: u64,
    ) -> Result<String, String> {
        for msg in self.state.messages.iter_mut() {
            if msg.id == message_id {
                msg.pinned = true;
            }
        }
        Ok(message_id.to_string())
    }
}

impl Dispatchable for MyJointReducer {
    type Action = Actions;
    type Response = DemoState;

    async fn dispatch(
        &mut self,
        client_id: u64,
        action: Actions,
    ) -> Result<ActionResponse<DemoState>, String> {
        let msg = match action {
            Actions::IdentifyUser(name) => self.action_identify_user(client_id, name).await?,
            Actions::SendMessage(content) => self.action_send_message(client_id, content).await?,
            Actions::DeleteMessage(id) => self.action_delete_message(client_id, id).await?,
            Actions::PinMessage(id) => self.action_pin_message(client_id, id).await?,
        };

        Ok(ActionResponse {
            state: self.state.clone(),
            author: client_id,
            data: msg,
        })
    }
}

async fn foo() {
    let mut joint = WebsocketJoint::<MyJointReducer>::new();
    joint.bind_addr("127.0.0.1:8080").await;
    joint.listen().await;
}
