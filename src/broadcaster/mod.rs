mod test;

use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::message::{JointMessage, JointMessageMethod};
use crate::response::{ClientResponse, Response, RoomResponse};
use crate::room::{Room, RoomStatus};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Broadcaster<S, R>
where
    S: SinkAdapter + Unpin,
    R: Dispatchable + Send,
{
    clients: Arc<Mutex<HashMap<u64, Client>>>,
    connections: Arc<Mutex<HashMap<u64, S>>>,
    rooms: Arc<Mutex<HashMap<u64, Room<R>>>>,
    default_reducer: R,
}

// Class that implements main publish-subscribe logic, by handling clients and rooms
impl<S, R> Broadcaster<S, R>
where
    S: SinkAdapter + Unpin,
    R: Dispatchable + Send,
{
    pub fn new(default_reducer: R) -> Self {
        Broadcaster {
            clients: Arc::new(Mutex::new(HashMap::<u64, Client>::new())),
            connections: Arc::new(Mutex::new(HashMap::<u64, S>::new())),
            rooms: Arc::new(Mutex::new(HashMap::<u64, Room<R>>::new())),
            default_reducer,
        }
    }

    // handles create room event
    pub(crate) async fn handle_create(
        &self,
        client_id: u64,
    ) -> Result<RoomResponse, ClientResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ClientResponse::not_found(client_id, "Client not found".to_string()))?;

        if client.room_id.is_some() {
            return Err(ClientResponse::client_error(
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
            reducer: Arc::new(Mutex::new(self.default_reducer.clone())),
        };

        rooms.insert(room_id, room);

        Ok(RoomResponse::create_room(room_id))
    }

    // handles join room event
    pub(crate) async fn handle_join(
        &self,
        client_id: u64,
        room_id: u64,
    ) -> Result<RoomResponse, ClientResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ClientResponse::not_found(client_id, "Client not found".to_string()))?;

        if client.room_id.is_some() {
            return Err(ClientResponse::client_error(
                client_id,
                "Leave current room before joining new".to_string(),
            ));
        }

        let mut rooms = self.rooms.lock().await;
        match rooms.get_mut(&room_id) {
            None => Err(ClientResponse::not_found(
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
    pub(crate) async fn handle_action(
        &self,
        client_id: u64,
        action: R::Action,
        reducer: Arc<Mutex<R>>,
    ) -> Result<RoomResponse, ClientResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ClientResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err(ClientResponse::not_found(
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
            Err(_) => Err(ClientResponse::not_found(0, "Client not found".to_string())),
        }
    }

    // handles user leave event
    pub(crate) async fn handle_leave(
        &self,
        client_id: u64,
    ) -> Result<RoomResponse, ClientResponse> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| ClientResponse::not_found(client_id, "Client not found".to_string()))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err(ClientResponse::not_found(
                client.id,
                "Client not in room".to_string(),
            ));
        }
        let room_id = room_id.unwrap();

        let mut rooms = self.rooms.lock().await;
        let room = rooms.get_mut(&room_id);
        if room.is_none() {
            return Err(ClientResponse::not_found(
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
    pub(crate) async fn process_event(
        &self,
        client_id: u64,
        event: JointMessage,
    ) -> Result<RoomResponse, ClientResponse> {
        {
            let clients = self.clients.lock().await;
            let client_exists = clients.contains_key(&client_id);
            if !client_exists {
                return Err(ClientResponse::not_found(
                    client_id,
                    "Client not found".to_string(),
                ));
            }
        }

        match event.message {
            JointMessageMethod::Create => {
                let result = self.handle_create(client_id).await;
                if let Ok(room_response) = &result {
                    let _ = self
                        .insert_client_to_room(client_id, room_response.room)
                        .await;
                }
                result
            }
            JointMessageMethod::Join(room_id) => {
                let result = self.handle_join(client_id, room_id).await;
                if let Ok(room_response) = &result {
                    let _ = self
                        .insert_client_to_room(client_id, room_response.room)
                        .await;
                }
                result
            }
            JointMessageMethod::Action(raw_action) => {
                let action: R::Action = serde_json::from_str(&raw_action).map_err(|_| {
                    ClientResponse::server_error(client_id, "Invalid action".to_string())
                })?;

                let clients = self.clients.clone();
                let clients = clients.lock().await;
                let client = clients.get(&client_id).ok_or_else(|| {
                    ClientResponse::not_found(client_id, "Client not found".to_string())
                })?;

                let room_id = client.room_id.ok_or_else(|| {
                    ClientResponse::not_found(client_id, "Client not in room".to_string())
                })?;
                drop(clients);

                let rooms = self.rooms.clone();
                let rooms = rooms.lock().await;
                let room = rooms.get(&room_id).ok_or_else(|| {
                    ClientResponse::not_found(client_id, "Room not found".to_string())
                })?;

                self.handle_action(client_id, action, room.reducer.clone())
                    .await
            }
            JointMessageMethod::Leave => self.handle_leave(client_id).await,
        }
    }

    // broadcasts response state to all clients in room
    pub(crate) async fn react_on_message(&self, room_id: u64, response: Response) {
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

    pub(crate) async fn react_with_error(&self, client_id: u64, error: Response) {
        let mut connections = self.connections.lock().await;
        if let Some(sender) = connections.get_mut(&client_id) {
            let _ = sender.send(error).await;
        }
    }

    // asynchronously handles WebSocket rx instance
    pub async fn handle_rx<C>(&self, client_id: u64, rx: &mut C)
    where
        C: StreamAdapter + Unpin,
    {
        while let Ok(event) = rx.next().await {
            let response = self.process_event(client_id, event).await;

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

    pub async fn extern_dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| format!("Client not found: {}", client_id))?;

        let room_id = client.room_id;
        if room_id.is_none() {
            return Err("Client not in room".to_string());
        }
        let room_id = room_id.unwrap();

        let mut rooms = self.rooms.lock().await;
        let room = rooms.get_mut(&room_id);
        if room.is_none() {
            return Err("Room not found".to_string());
        }
        let room = room.unwrap();

        let mut reducer_guard = room.reducer.lock().await;

        let parsed_action = serde_json::from_str(action).map_err(|e| e.to_string())?;

        reducer_guard.dispatch(client_id, parsed_action).await
    }

    pub(crate) async fn insert_client_to_room(
        &self,
        client_id: u64,
        room_id: u64,
    ) -> Result<(), String> {
        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&client_id)
            .ok_or_else(|| format!("Client not found: {}", client_id))?;

        let mut rooms = self.rooms.lock().await;
        let room = rooms.get_mut(&room_id);
        if room.is_none() {
            return Err("Room not found".to_string());
        }
        let room = room.unwrap();

        room.client_ids.insert(client_id);
        client.room_id = Some(room_id);

        let mut connections = self.connections.lock().await;
        let conn = connections.get_mut(&client_id).unwrap(); // save

        let str_state = serde_json::to_string(&room.reducer.lock().await.get_state()).unwrap();

        let _ = conn.send(Response::StateSent(str_state)).await;

        Ok(())
    }

    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_clients(&self) -> Arc<Mutex<HashMap<u64, Client>>> {
        self.clients.clone()
    }

    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_rooms(&self) -> Arc<Mutex<HashMap<u64, Room<R>>>> {
        self.rooms.clone()
    }

    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_connections(&self) -> Arc<Mutex<HashMap<u64, S>>> {
        self.connections.clone()
    }
}
