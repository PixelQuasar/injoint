use crate::event::Event;
use anyhow::{anyhow, Result};
use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tokio::sync::Mutex;

type RoomCreatedHandler = Box<dyn Fn(u64) -> BoxFuture<'static, Result<()>> + Send>;
type RoomJoinedHandler = Box<dyn Fn(u64) -> BoxFuture<'static, Result<()>> + Send>;
type StateSentHandler<T> = Box<dyn Fn(T) -> BoxFuture<'static, Result<()>> + Send>;
type ActionHandler<A, T> = Box<dyn Fn(A, u64, T) -> BoxFuture<'static, Result<()>> + Send>;
type RoomLeftHandler = Box<dyn Fn(u64) -> BoxFuture<'static, Result<()>> + Send>;
type NotFoundErrorHandler = Box<dyn Fn(String) -> BoxFuture<'static, Result<()>> + Send>;
type ClientErrorHandler = Box<dyn Fn(String) -> BoxFuture<'static, Result<()>> + Send>;
type ServerErrorHandler = Box<dyn Fn(String) -> BoxFuture<'static, Result<()>> + Send>;

pub enum Handler<A, T>
where
    A: DeserializeOwned + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    HandleRoomCreated(RoomCreatedHandler),
    HandleRoomJoined(RoomJoinedHandler),
    HandleStateSent(StateSentHandler<T>),
    HandleActionBox(ActionHandler<A, T>),
    HandleRoomLeft(RoomLeftHandler),
    HandleNotFoundError(NotFoundErrorHandler),
    HandleClientError(ClientErrorHandler),
    HandleServerError(ServerErrorHandler),
}

pub struct Handlers<A, T>
where
    A: DeserializeOwned + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    pub handle_room_created: Option<RoomCreatedHandler>,
    pub handle_room_joined: Option<RoomJoinedHandler>,
    pub handle_state_sent: Option<StateSentHandler<T>>,
    pub handle_action_box: Option<ActionHandler<A, T>>,
    pub handle_room_left: Option<RoomLeftHandler>,
    pub handle_not_found_error: Option<NotFoundErrorHandler>,
    pub handle_client_error: Option<ClientErrorHandler>,
    pub handle_server_error: Option<ServerErrorHandler>,
}

impl<A, T> Handlers<A, T>
where
    A: DeserializeOwned + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    pub fn new() -> Self {
        Handlers {
            handle_room_created: None,
            handle_room_joined: None,
            handle_state_sent: None,
            handle_action_box: None,
            handle_room_left: None,
            handle_not_found_error: None,
            handle_server_error: None,
            handle_client_error: None,
        }
    }
}

pub struct EventListener<A, T>
where
    A: DeserializeOwned + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    handlers: Arc<Mutex<Handlers<A, T>>>,
}

impl<A, T> EventListener<A, T>
where
    A: DeserializeOwned + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    pub fn new() -> Self {
        EventListener {
            handlers: Arc::new(Mutex::new(Handlers::new())),
        }
    }

    pub async fn register_handler(&self, handler: Handler<A, T>) {
        let mut handlers = self.handlers.lock().await;
        match handler {
            Handler::HandleRoomCreated(handler) => handlers.handle_room_created = Some(handler),
            Handler::HandleRoomJoined(handler) => handlers.handle_room_joined = Some(handler),
            Handler::HandleStateSent(handler) => handlers.handle_state_sent = Some(handler),
            Handler::HandleActionBox(handler) => handlers.handle_action_box = Some(handler),
            Handler::HandleRoomLeft(handler) => handlers.handle_room_left = Some(handler),
            Handler::HandleNotFoundError(handler) => {
                handlers.handle_not_found_error = Some(handler)
            }
            Handler::HandleClientError(handler) => handlers.handle_client_error = Some(handler),
            Handler::HandleServerError(handler) => handlers.handle_server_error = Some(handler),
        }
    }

    pub async fn handle_event(&self, message: &str) -> Result<()> {
        let event = match serde_json::from_str(message) {
            Ok(event) => event,
            Err(e) => return Err(anyhow!("Failed to parse event: {}", e)),
        };

        let handlers = self.handlers.lock().await;
        match event {
            Event::RoomCreated(room_id) => {
                if let Some(handler) = &handlers.handle_room_created {
                    handler(room_id).await?;
                } else {
                    return Err(anyhow!("No handler for RoomCreated event"));
                }
            }
            Event::RoomJoined(client_id) => {
                if let Some(handler) = &handlers.handle_room_joined {
                    handler(client_id).await?;
                } else {
                    return Err(anyhow!("No handler for RoomJoined event"));
                }
            }
            Event::StateSent(payload) => {
                if let Some(handler) = &handlers.handle_state_sent {
                    handler(payload).await?;
                } else {
                    return Err(anyhow!("No handler for StateSent event"));
                }
            }
            Event::Action(payload) => {
                if let Some(handler) = &handlers.handle_action_box {
                    handler(payload.action, payload.client_id, payload.state).await?;
                } else {
                    return Err(anyhow!("No handler for Action event"));
                }
            }
            Event::RoomLeft(client_id) => {
                if let Some(handler) = &handlers.handle_room_left {
                    handler(client_id).await?;
                } else {
                    return Err(anyhow!("No handler for RoomLeft event"));
                }
            }
            Event::NotFound(msg) => {
                if let Some(handler) = &handlers.handle_not_found_error {
                    handler(msg).await?;
                } else {
                    return Err(anyhow!("No handler for NotFound event"));
                }
            }
            Event::ClientError(msg) => {
                if let Some(handler) = &handlers.handle_client_error {
                    handler(msg).await?;
                } else {
                    return Err(anyhow!("No handler for ClientError event"));
                }
            }
            Event::ServerError(msg) => {
                if let Some(handler) = &handlers.handle_server_error {
                    handler(msg).await?;
                } else {
                    return Err(anyhow!("No handler for ServerError event"));
                }
            }
        }
        Ok(())
    }
}
