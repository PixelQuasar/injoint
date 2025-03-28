mod test;

use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use crate::message::{JointMessage, JointMessageMethod};
use crate::response::{ErrorResponse, Response, RoomResponse};
use crate::room::{Room, RoomStatus};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Broadcaster<S>
where
    S: SinkAdapter + Unpin,
{
    clients: Arc<Mutex<HashMap<u64, Client>>>,
    connections: Arc<Mutex<HashMap<u64, S>>>,
    rooms: Arc<Mutex<HashMap<u64, Room>>>,
}

// Class that implements main publish-subscribe logic, by handling clients and rooms
impl<S> Broadcaster<S>
where
    S: SinkAdapter + Unpin,
{
    pub fn new() -> Self {
        Broadcaster {
            clients: Arc::new(Mutex::new(HashMap::<u64, Client>::new())),
            connections: Arc::new(Mutex::new(HashMap::<u64, S>::new())),
            rooms: Arc::new(Mutex::new(HashMap::<u64, Room>::new())),
        }
    }

    // handles create room event
    async fn handle_create<R: Dispatchable>(
        &self,
        client_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        if client.room_id.is_some() {
            return Err(ErrorResponse::client_error(
                client_id,
                "Leave current room before creating new".to_string(),
            ));
        }

        let mut rooms = self.rooms.lock().await;
        let room_id = rooms.len() as u64;

        let mut room_clients = HashSet::<u64>::new();
        room_clients.insert(client_id);
        client.room_id = Some(room_id);

        let room = Room {
            id: room_id,
            owner_id: client.id,
            client_ids: room_clients,
            status: RoomStatus::Public,
        };

        println!("room created: {:#?}", room);

        rooms.insert(room_id, room);

        Ok(RoomResponse::create_room(room_id))
    }

    // handles join room event
    async fn handle_join<R: Dispatchable>(
        &self,
        client_id: u64,
        room_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        if client.room_id.is_some() {
            return Err(ErrorResponse::client_error(
                client_id,
                "Leave current room before joining new".to_string(),
            ));
        }

        let mut rooms = self.rooms.lock().await;
        match rooms.get_mut(&room_id) {
            None => Err(ErrorResponse::not_found(
                client.id,
                "Room not found".to_string(),
            )),
            Some(room) => {
                let client_id = client.id;
                room.client_ids.insert(client_id);
                client.room_id = Some(room.id);
                Ok(RoomResponse::join_room(room_id, client_id))
            }
        }
    }

    // handles dispatchable action event
    async fn handle_action<R: Dispatchable>(
        &self,
        client_id: u64,
        action: R::Action,
        reducer: Arc<Mutex<R>>,
    ) -> Result<RoomResponse, ErrorResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err(ErrorResponse::not_found(
                client.id,
                "Client not in room".to_string(),
            ));
        }
        let room_id = room_id.unwrap();

        let mut reducer_guard = reducer.lock().await;
        match reducer_guard.dispatch(client_id, action).await {
            Ok(state) => Ok(RoomResponse::action(
                room_id,
                serde_json::to_string(&state).unwrap(),
            )),
            Err(_) => Err(ErrorResponse::not_found(0, "Client not found".to_string())),
        }
    }

    // handles user leave event
    async fn handle_leave<R: Dispatchable>(
        &self,
        client_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err(ErrorResponse::not_found(
                client.id,
                "Client not in room".to_string(),
            ));
        }
        let room_id = room_id.unwrap();

        let mut rooms = self.rooms.lock().await;
        let room = rooms.get_mut(&room_id);
        if room.is_none() {
            return Err(ErrorResponse::not_found(
                client.id,
                "Room not found".to_string(),
            ));
        }
        let room = room.unwrap();

        room.client_ids.remove(&client.id);
        client.room_id = None;
        Ok(RoomResponse::leave_room(room_id, client.id))
    }

    // processes abstract event
    async fn process_event<R: Dispatchable>(
        &self,
        client_id: u64,
        event: JointMessage,
        reducer: Arc<Mutex<R>>,
    ) -> Result<RoomResponse, ErrorResponse> {
        // Decouple client lookup from subsequent logic to reduce scope of mutable borrow
        let clients = self.clients.lock().await;
        let client_exists = clients.contains_key(&client_id);
        if !client_exists {
            eprintln!("Client {} not found", client_id);
            return Err(ErrorResponse::not_found(
                client_id,
                "Client not found".to_string(),
            ));
        }
        drop(clients);

        // Handle events based on the message type
        match event.message {
            JointMessageMethod::Create => self.handle_create::<R>(client_id).await,
            JointMessageMethod::Join(room_id) => self.handle_join::<R>(client_id, room_id).await,
            JointMessageMethod::Action(raw_action) => {
                let action: R::Action = serde_json::from_str(&raw_action).map_err(|_| {
                    ErrorResponse::server_error(client_id, "Invalid action".to_string())
                })?;

                self.handle_action(client_id, action, reducer).await
            }
            JointMessageMethod::Leave => self.handle_leave::<R>(client_id).await,
        }
    }

    // broadcasts response state to all clients in room
    async fn react_on_message(&self, room_id: u64, response: Response) {
        let rooms = self.rooms.lock().await;
        let room = rooms.get(&room_id).ok_or("Room not found".to_string());

        if let Ok(room) = room {
            // broadcast response state to all clients in room
            let mut clients = self.clients.lock().await;
            let mut connections = self.connections.lock().await;
            for client_id in room.client_ids.iter() {
                if let Some(client) = clients.get_mut(client_id) {
                    let _ = connections
                        .get_mut(&client.id)
                        .unwrap()
                        .send(response.clone())
                        .await;
                }
            }
        }
    }

    async fn react_with_error(&self, client_id: u64, error: Response) {
        let mut connections = self.connections.lock().await;
        if let Some(sender) = connections.get_mut(&client_id) {
            let _ = sender.send(error).await;
        }
    }

    // asynchronously handles WebSocket rx instance
    pub async fn handle_rx<R, C>(&self, client_id: u64, rx: &mut C, reducer: Arc<Mutex<R>>)
    where
        R: Dispatchable,
        C: StreamAdapter + Unpin,
    {
        while let Ok(event) = rx.next().await {
            println!("message from {}: {:#?}", client_id, event);
            let response = self.process_event(client_id, event, reducer.clone()).await;

            println!("resp: {:#?}", response);

            match response {
                Ok(room_response) => {
                    self.react_on_message(room_response.room, room_response.response)
                        .await
                }
                Err(error_response) => {
                    self.react_with_error(error_response.client, error_response.response)
                        .await
                }
            }
        }
    }

    pub async fn add_client_connection(&self, client: Client, sender: S) {
        let id = client.id;
        let mut clients = self.clients.lock().await;
        clients.insert(id, client);
        let mut connections = self.connections.lock().await;
        connections.insert(id, sender);
    }

    pub async fn remove_client_connection(&self, client_id: u64) {
        let mut clients = self.clients.lock().await;
        clients.remove(&client_id);
        let mut connections = self.connections.lock().await;
        connections.remove(&client_id);
    }

    pub fn get_clients(&self) -> Arc<Mutex<HashMap<u64, Client>>> {
        self.clients.clone()
    }

    pub fn get_rooms(&self) -> Arc<Mutex<HashMap<u64, Room>>> {
        self.rooms.clone()
    }

    pub fn get_connections(&self) -> Arc<Mutex<HashMap<u64, S>>> {
        self.connections.clone()
    }
}
