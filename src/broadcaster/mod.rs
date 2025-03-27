use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use crate::message::{JointMessage, JointMessageMethod};
use crate::response::{ErrorResponse, Response, RoomResponse};
use crate::{Room, RoomStatus};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Broadcaster<S>
where
    S: SinkAdapter + Unpin,
{
    clients: HashMap<u64, Client>,
    connections: HashMap<u64, S>,
    rooms: HashMap<u64, Room>,
}

// Class that implements main publish-subscribe logic, by handling clients and rooms
impl<S> Broadcaster<S>
where
    S: SinkAdapter + Unpin,
{
    pub fn new() -> Self {
        Broadcaster {
            clients: HashMap::<u64, Client>::new(),
            connections: HashMap::<u64, S>::new(),
            rooms: HashMap::<u64, Room>::new(),
        }
    }

    // handles create room event
    fn handle_create<R: Dispatchable>(
        &mut self,
        client_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let client = self
            .clients
            .get(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = self.rooms.len() as u64;
        let room = Room {
            id: room_id,
            owner_id: client.id,
            client_ids: HashSet::<u64>::new(),
            status: RoomStatus::Public,
        };
        self.rooms.insert(room_id, room);
        Ok(RoomResponse::create_room(room_id))
    }

    // handles join room event
    fn handle_join<R: Dispatchable>(
        &mut self,
        client_id: u64,
        room_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let client = self
            .clients
            .get(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        match self.rooms.get_mut(&room_id) {
            None => Err(ErrorResponse::not_found(
                client.id,
                "Room not found".to_string(),
            )),
            Some(room) => {
                let client_id = client.id;
                room.client_ids.insert(client_id);
                Ok(RoomResponse::join_room(room_id, client_id))
            }
        }
    }

    // handles dispatchable action event
    async fn handle_action<R: Dispatchable>(
        &mut self,
        client_id: u64,
        action: R::Action,
        reducer: Arc<Mutex<R>>,
    ) -> Result<RoomResponse, ErrorResponse> {
        if let Some(client) = self.clients.get(&client_id) {
            let mut reducer_guard = reducer.lock().await;
            match reducer_guard.dispatch(client_id, action).await {
                Ok(state) => Ok(RoomResponse::action(
                    client.room_id.unwrap(),
                    serde_json::to_string(&state).unwrap(),
                )),
                Err(e) => Err(ErrorResponse::not_found(0, "Client not found".to_string())),
            }
        } else {
            Err(ErrorResponse::not_found(0, "Client not found".to_string()))
        }
    }

    // handles user leave event
    fn handle_leave<R: Dispatchable>(
        &mut self,
        client_id: u64,
    ) -> Result<RoomResponse, ErrorResponse> {
        let client = self
            .clients
            .get(&client_id)
            .ok_or_else(|| ErrorResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err(ErrorResponse::not_found(
                client.id,
                "Client not in room".to_string(),
            ));
        }
        let room_id = room_id.unwrap();

        let room = self.rooms.get_mut(&room_id);
        if room.is_none() {
            return Err(ErrorResponse::not_found(
                client.id,
                "Room not found".to_string(),
            ));
        }
        let room = room.unwrap();

        room.client_ids.remove(&client.id);
        Ok(RoomResponse::leave_room(room_id, client.id))
    }

    // processes abstract event
    async fn process_event<R: Dispatchable>(
        &mut self,
        client_id: u64,
        event: JointMessage,
        reducer: Arc<Mutex<R>>,
    ) -> Result<RoomResponse, ErrorResponse> {
        // Decouple client lookup from subsequent logic to reduce scope of mutable borrow
        let client_exists = self.clients.contains_key(&client_id);
        if !client_exists {
            eprintln!("Client {} not found", client_id);
            return Err(ErrorResponse::not_found(
                client_id,
                "Client not found".to_string(),
            ));
        }

        // Handle events based on the message type
        match event.message {
            JointMessageMethod::Create => self.handle_create::<R>(client_id),
            JointMessageMethod::Join(room_id) => self.handle_join::<R>(client_id, room_id),
            JointMessageMethod::Action(raw_action) => {
                let action: R::Action = serde_json::from_str(&raw_action).map_err(|_| {
                    ErrorResponse::server_error(client_id, "Invalid action".to_string())
                })?;

                self.handle_action(client_id, action, reducer).await
            }
            JointMessageMethod::Leave => self.handle_leave::<R>(client_id),
        }
    }

    // broadcasts response state to all clients in room
    async fn react_on_message(&mut self, room_id: u64, response: Response) -> Result<(), String> {
        let room = self
            .rooms
            .get(&room_id)
            .ok_or("Room not found".to_string())?;

        // broadcast response state to all clients in room
        for client_id in room.client_ids.iter() {
            if let Some(client) = self.clients.get_mut(client_id) {
                let _ = self
                    .connections
                    .get_mut(&client.id)
                    .unwrap()
                    .send(response.clone())
                    .await;
            }
        }

        Ok(())
    }

    // asynchronously handles WebSocket rx instance
    pub async fn handle_rx<R, C>(&mut self, client_id: u64, rx: &mut C, reducer: Arc<Mutex<R>>)
    where
        R: Dispatchable,
        C: StreamAdapter + Unpin,
    {
        while let Ok(event) = rx.next().await {
            let response = self.process_event(client_id, event, reducer.clone()).await;

            match response {
                Ok(room_response) => {
                    if let Err(e) = self
                        .react_on_message(room_response.room, room_response.response)
                        .await
                    {
                        eprintln!("Error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
    }

    pub fn get_client(&self, id: u64) -> (Option<&Client>) {
        self.clients.get(&id)
    }

    pub fn get_connection(&self, id: u64) -> Option<&S> {
        self.connections.get(&id)
    }

    pub fn add_client_connection(&mut self, client: Client, sender: S) {
        let id = client.id;
        self.clients.insert(id, client);
        self.connections.insert(id, sender);
    }

    pub fn remove_client_connection(&mut self, client_id: u64) {
        self.clients.remove(&client_id);
        self.connections.remove(&client_id);
    }
}
