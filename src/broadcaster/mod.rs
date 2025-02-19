use crate::dispatcher::Dispatchable;
use crate::message::Response;
use crate::utils::types::{Broadcastable, WebMethods};
use crate::{Client, Room, RoomStatus};
use futures_util::SinkExt;
use serde_json;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tokio::sync::mpsc::UnboundedReceiver;
use tungstenite::Message;

pub struct Broadcaster {
    clients: HashMap<u64, Client>,
    rooms: HashMap<u64, Room>,
    current_target_room_id: Option<u64>,
}

impl Broadcaster {
    pub fn new() -> Broadcaster {
        Broadcaster {
            clients: HashMap::<u64, Client>::new(),
            rooms: HashMap::<u64, Room>::new(),
            current_target_room_id: None,
        }
    }

    fn handle_create<T: Broadcastable>(&mut self, client: Client) -> Response<T> {
        let room_id = self.rooms.len() as u64;
        let room = Room {
            id: room_id,
            owner_id: client.id,
            client_ids: HashSet::<u64>::new(),
            status: RoomStatus::Public,
        };
        self.rooms.insert(room_id, room);
        self.clients.insert(client.id, client);
        self.current_target_room_id = Some(room_id);
        Response::RoomCreated(room_id)
    }

    fn handle_join<T: Broadcastable>(&mut self, client: Client, room_id: u64) -> Response<T> {
        match self.rooms.get_mut(&room_id) {
            None => Response::NotFound("Room not found".to_string()),
            Some(room) => {
                room.client_ids.insert(client.id);
                self.clients.insert(client.id, client);
                self.current_target_room_id = Some(room_id);
                Response::RoomJoined(room_id)
            }
        }
    }

    async fn handle_action<A, T: Broadcastable, R: Dispatchable<A, T>>(
        &mut self,
        client_id: u64,
        action: A,
        reducer: Rc<RefCell<R>>,
    ) -> Response<T> {
        if let Some(client) = self.clients.get(&client_id) {
            self.current_target_room_id = client.room_id;
            match reducer.borrow_mut().dispatch(client_id, action).await {
                Ok(state) => Response::Action(state),
                Err(e) => Response::ClientError(e),
            }
        } else {
            Response::NotFound("Client not found".to_string())
        }
    }

    fn handle_leave<T: Broadcastable>(&mut self, client_id: u64) -> Response<T> {
        let client = self.clients.remove(&client_id);
        if client.is_none() {
            return Response::NotFound("Client not found".to_string());
        }
        let client = client.unwrap();

        let room_id = client.room_id;
        if room_id.is_none() {
            return Response::NotFound("Client not in room".to_string());
        }
        let room_id = room_id.unwrap();

        let room = self.rooms.get_mut(&room_id);
        if room.is_none() {
            return Response::NotFound("Room not found".to_string());
        }
        let room = room.unwrap();

        room.client_ids.remove(&client_id);
        self.current_target_room_id = Some(room_id);
        Response::RoomLeft(room_id)
    }

    async fn process_event<A, T: Broadcastable, R: Dispatchable<A, T>>(
        &mut self,
        event: WebMethods<A>,
        reducer: Rc<RefCell<R>>,
    ) -> Result<Response<T>, Response<T>> {
        match event {
            WebMethods::Create(client) => Ok(self.handle_create(client)),
            WebMethods::Join(client, room_id, password) => Ok(self.handle_join(client, room_id)),
            WebMethods::Action(client_id, action) => {
                Ok(self.handle_action(client_id, action, reducer.clone()).await)
            }
            WebMethods::Leave(client_id) => Ok(self.handle_leave(client_id)),
        }
    }

    async fn react_message<T: Broadcastable>(
        &mut self,
        response: Response<T>,
    ) -> Result<(), String> {
        let room_id = self
            .current_target_room_id
            .ok_or("No room selected".to_string())?;

        let room = self
            .rooms
            .get(&room_id)
            .ok_or("Room not found".to_string())?;

        // broadcast response state to all clients in room
        for client_id in room.client_ids.iter() {
            if let Some(client) = self.clients.get_mut(client_id) {
                let _ = client
                    .conn
                    .send(Message::from(serde_json::to_string(&response).unwrap()))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn handle_rx<A, T: Broadcastable, R: Dispatchable<A, T>>(
        &mut self,
        mut rx: UnboundedReceiver<WebMethods<A>>,
        reducer: Rc<RefCell<R>>,
    ) {
        while let Some(event) = rx.recv().await {
            let response = self.process_event(event, reducer.clone()).await;

            match response {
                Ok(response) => {
                    if let Err(e) = self.react_message(response).await {
                        eprintln!("Error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
    }
}
