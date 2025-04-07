/// Broadcaster module for implementing `Broadcaster` struct that handles clients, connections, and rooms
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

/// Broadcaster struct that manages clients, connections, and rooms
///
/// This struct is responsible for handling the main publish-subscribe logic,
/// including creating and joining rooms, dispatching actions, and broadcasting messages.
///
/// - S is the type of the sink adapter used for sending messages to clients.
/// - R is the type of the reducer used for managing the state of the rooms.
pub struct Broadcaster<S, R>
where
    S: SinkAdapter + Unpin + Clone,
    R: Dispatchable + Send,
{
    /// A map of client IDs to their corresponding Client objects.
    clients: Arc<Mutex<HashMap<u64, Client>>>,
    /// A map of client IDs to their corresponding connection objects.
    connections: Arc<Mutex<HashMap<u64, S>>>,
    /// A map of room IDs to their corresponding Room objects.
    rooms: Arc<Mutex<HashMap<u64, Room<R>>>>,
    /// The default reducer used for managing the state of the rooms.
    default_reducer: R,
}

// Class that implements main publish-subscribe logic, by handling clients and rooms
impl<S, R> Broadcaster<S, R>
where
    S: SinkAdapter + Unpin + Clone,
    R: Dispatchable + Send,
{
    /// Creates a new Broadcaster instance with the given default reducer.
    pub fn new(default_reducer: R) -> Self {
        Broadcaster {
            clients: Arc::new(Mutex::new(HashMap::<u64, Client>::new())),
            connections: Arc::new(Mutex::new(HashMap::<u64, S>::new())),
            rooms: Arc::new(Mutex::new(HashMap::<u64, Room<R>>::new())),
            default_reducer,
        }
    }

    /// Handles the creation of a new room.
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

    /// Handles the joining of an existing room.
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

    /// handles dispatchable action event
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

    /// handles user leave event
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

    /// processes abstract event
    ///
    /// # Arguments
    /// * `client_id` - The ID of the client sending the event.
    /// * `event` - The event to be processed.
    ///
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

                let reducer_arc = {
                    let clients = self.clients.lock().await;
                    let client = clients.get(&client_id).ok_or_else(|| {
                        ClientResponse::not_found(client_id, "Client not found".to_string())
                    })?;
                    let room_id = client.room_id.ok_or_else(|| {
                        ClientResponse::not_found(client_id, "Client not in room".to_string())
                    })?;

                    let rooms = self.rooms.lock().await;
                    let room = rooms.get(&room_id).ok_or_else(|| {
                        ClientResponse::not_found(client_id, "Room not found".to_string())
                    })?;
                    room.reducer.clone()
                };

                self.handle_action(client_id, action, reducer_arc).await
            }
            JointMessageMethod::Leave => self.handle_leave(client_id).await,
        }
    }

    /// broadcasts response state to all clients in room
    ///
    /// # Arguments
    /// * `room_id` - The ID of the room to which the response should be sent.
    /// * `response` - The response to be sent to the clients.
    ///
    pub(crate) async fn react_on_message(&self, room_id: u64, response: Response) {
        let client_connections_to_send: Vec<(u64, S)> = {
            let clients = self.clients.lock().await;
            let rooms = self.rooms.lock().await;
            let connections = self.connections.lock().await;

            let room = match rooms.get(&room_id) {
                Some(r) => r,
                None => {
                    eprintln!("Warning: Trying to react in non-existent room {}", room_id);
                    return;
                }
            };

            let mut connections_to_send = Vec::new();
            for client_id in room.client_ids.iter() {
                if clients.contains_key(client_id) {
                    if let Some(connection) = connections.get(client_id) {
                        connections_to_send.push((*client_id, connection.clone()));
                    } else {
                        eprintln!(
                            "Warning: Connection not found for client {} in room {}",
                            client_id, room_id
                        );
                    }
                } else {
                    eprintln!(
                        "Warning: Client {} not found for room {}",
                        client_id, room_id
                    );
                }
            }
            connections_to_send
        };

        for (_, mut connection) in client_connections_to_send {
            if let Err(_) = connection.send(response.clone()).await {}
        }
    }

    /// sends error message to client
    pub(crate) async fn react_with_error(&self, client_id: u64, error: Response) {
        let connection_to_send: Option<S> = {
            let connections = self.connections.lock().await;
            connections.get(&client_id).cloned()
        };

        if let Some(mut sender) = connection_to_send {
            if let Err(e) = sender.send(error).await {
                eprintln!(
                    "Error sending error message to client {}: {}. Consider removing client.",
                    client_id, e
                );
            }
        }
    }

    /// asynchronously handles WebSocket rx instance
    ///
    /// # Arguments
    /// * `client_id` - The ID of the client sending the event.
    /// * `rx` - The rx instance to be processed.
    ///
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

    /// adds a new client connection
    pub async fn add_client_connection(&self, client: Client, sender: S) {
        let id = client.id;
        let mut clients = self.clients.lock().await;
        clients.insert(id, client);
        let mut connections = self.connections.lock().await;
        connections.insert(id, sender);
    }

    /// removes a client connection
    pub async fn remove_client_connection(&self, client_id: u64) {
        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get(&client_id) {
            if let Some(room_id) = client.room_id {
                let mut rooms = self.rooms.lock().await;
                if let Some(room) = rooms.get_mut(&room_id) {
                    room.client_ids.remove(&client_id);
                }
            }
        }

        clients.remove(&client_id);
        let mut connections = self.connections.lock().await;
        connections.remove(&client_id);
    }

    /// dispatches an action to the reducer
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

    /// inserts a client into a room and sends the initial state to the client
    pub(crate) async fn insert_client_to_room(
        &self,
        client_id: u64,
        room_id: u64,
    ) -> Result<(), String> {
        let (state_str, connection_to_send) = {
            let mut clients = self.clients.lock().await;
            let mut rooms = self.rooms.lock().await;
            let connections = self.connections.lock().await;

            let client = clients
                .get_mut(&client_id)
                .ok_or_else(|| format!("Client {} not found", client_id))?;

            let room = rooms
                .get_mut(&room_id)
                .ok_or_else(|| format!("Room {} not found", room_id))?;

            let connection = connections
                .get(&client_id)
                .ok_or_else(|| format!("Connection not found for client {}", client_id))?;

            room.client_ids.insert(client_id);
            client.room_id = Some(room_id);

            let state = room.reducer.lock().await.get_state();
            let state_str = serde_json::to_string(&state)
                .map_err(|e| format!("Failed to serialize state: {}", e))?;

            (state_str, connection.clone())
        };

        let mut connection = connection_to_send;
        if let Err(e) = connection.send(Response::StateSent(state_str)).await {
            eprintln!(
                "Error sending initial state to client {}: {}. Client may not be fully joined.",
                client_id, e
            );
        }

        Ok(())
    }

    /// returns broadcaster clients
    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_clients(&self) -> Arc<Mutex<HashMap<u64, Client>>> {
        self.clients.clone()
    }

    /// returns broadcaster rooms
    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_rooms(&self) -> Arc<Mutex<HashMap<u64, Room<R>>>> {
        self.rooms.clone()
    }

    /// returns broadcaster connections
    #[allow(dead_code)] // getter is used in tests
    pub(crate) fn get_connections(&self) -> Arc<Mutex<HashMap<u64, S>>> {
        self.connections.clone()
    }
}
