use crate::event_listener::{EventListener, Handler};
use crate::message::MessageRequest;
use crate::utils::{WSSink, WSStream};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

pub struct JointClient<A, T>
where
    A: DeserializeOwned + Serialize + Send + Sync + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
{
    listener: Arc<Mutex<EventListener<A, T>>>,
    pub client_id: Option<Arc<Mutex<u64>>>,
    pub room_id: Option<Arc<Mutex<u64>>>,
    sink: Option<Arc<Mutex<WSSink>>>,
    stream: Option<Arc<Mutex<WSStream>>>,
}

impl<A, T> JointClient<A, T>
where
    A: DeserializeOwned + Serialize + Send + Sync + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
{
    pub fn new() -> Self {
        JointClient {
            listener: Arc::new(Mutex::new(EventListener::new())),
            client_id: None,
            room_id: None,
            sink: None,
            stream: None,
        }
    }

    pub async fn register_handler(&mut self, handler: Handler<A, T>) {
        let listener = self.listener.lock().await;
        listener.register_handler(handler).await
    }

    pub async fn connect(&mut self, addr: &str) -> Result<()> {
        let (ws_stream, _) = connect_async(addr).await?;
        let (sink, stream) = ws_stream.split();
        self.sink = Some(Arc::new(Mutex::new(sink)));
        self.stream = Some(Arc::new(Mutex::new(stream)));
        Ok(())
    }

    pub async fn listen(&self) -> Result<()> {
        if let Some(stream) = self.stream.clone() {
            let listener = self.listener.clone();

            tokio::spawn(async move {
                let mut stream = stream.lock().await;
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let listener = listener.lock().await; // Ensure listener is locked within the async block
                            if let Err(e) = listener.handle_event(&text).await {
                                eprintln!("Error handling message: {}", e);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("WebSocket error: {}", e);
                            break;
                        }
                    }
                }
            });

            Ok(())
        } else {
            Err(anyhow!("WebSocket stream is not initialized"))
        }
    }

    pub async fn create_room(&self) -> Result<()> {
        self.send_event(MessageRequest::create_room()).await
    }

    pub async fn join_room(&self, room_id: u64) -> Result<()> {
        self.send_event(MessageRequest::join_room(room_id)).await
    }

    pub async fn leave_room(&self) -> Result<()> {
        self.send_event(MessageRequest::leave_room()).await
    }

    pub async fn dispatch_action(&self, action: A) -> Result<()> {
        self.send_event(MessageRequest::action(action)).await
    }

    async fn send_event(&self, request: MessageRequest<A>) -> Result<()> {
        let create_json = serde_json::to_string(&request)?;
        if let Some(sink) = &self.sink {
            let mut sink = sink.lock().await;
            sink.send(Message::Text(create_json.into())).await?;
            Ok(())
        } else {
            Err(anyhow!("WebSocket stream is not initialized"))
        }
    }
}
