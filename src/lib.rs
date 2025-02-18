pub mod broadcaster;
pub mod connection;
pub mod controller;
pub mod message;
pub mod reducer;
pub mod room;
pub mod store;
pub mod tcp_handler;
pub mod utils;


use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use utils::get_id;
use crate::message::Response;
use futures::SinkExt;
use futures_util::stream::SplitSink;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use tokio::net::TcpStream;
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tungstenite::{client, Utf8Bytes};

#[derive(Serialize, Deserialize, Clone)]
struct DemoModel {
    id: usize,
    name: String,
}

impl DemoModel {
    pub fn new(name: String) -> Self {
        DemoModel { id: get_id(), name }
    }
}

type CollectionNameType = String;

type JointQueryName = String;

struct Joint<T> {
    id: usize,
    reducers: Arc<DashMap<JointQueryName, Operation<T>>>,
}

impl<T> Joint<T> {
    pub async fn new() -> Self {
        Joint {
            id: get_id(),
            reducers: Arc::new(DashMap::new()),
        }
    }
}

// async fn test() {
//     // let server = TcpListener::bind("0.0.0.0:8080").await.unwrap();

//     // // Создаём неограниченный канал mpsc
//     // let (broadcast_sender, broadcast_receiver) = mpsc::unbounded_channel::<BroadcastMessage>();

//     // // Создаём новый поток токио и на выполнение ему передаём нашу
//     // // функцию Broadcast.
//     // tokio::spawn(broadcast::run(broadcast_receiver));

//     // let game = Game::new(broadcast_sender);

//     // // В цикле принимаем все запросы на соединение
//     // loop {
//     //     let (stream, _) = server.accept().await.unwrap();
//     //     // Обрабатываем все соединения в отдельном токио потоке
//     //     tokio::spawn(process_con(stream, game.clone()));
//     // }
// }

// build_joint!(MyJoint, DemoState, (
//     (
//         "/route/add_user",
//         |state: DemoState, name: String| {
//             let mut state = state.lock().await;
//             state.users.push(name);
//             state.count += 1;
//         }
//     ),
//     (
//         "/route/remove_user",
//         |state: DemoState, name: String| {
//             let mut state = state.lock().await;
//             state.users.retain(|user| user != name);
//             state.count -= 1;
//         }
//     )
// )

// BUILDING JOINT WITH MACROS EXAMPLE:
// impl MyJoint {
//     build_joint!(
//         (
//             "/route/add_user",
//             |state: DemoState, name: String| {
//                 let mut state = state.lock().await;
//                 state.users.push(name);
//                 state.count += 1;
//             }
//         ),
//         (
//             "/route/remove_user",
//             |state: DemoState, name: String| {
//                 let mut state = state.lock().await;
//                 state.users.retain(|user| user != name);
//                 state.count -= 1;
//             }
//         )
//     );
// }

// THIS WOULD EXPAND TO SOMETHING LIKE THIS:

#[derive(Serialize, Clone)]
struct TextMessage {
    id: u64,
    author: u64,
    content: String,
    pinned: bool,
}

#[derive(Serialize, Clone)]
struct DemoState {
    messages: Vec<TextMessage>,
    user_names: HashMap<u64, String>,
}

struct WebMessage {
    sender: u64,
    data: String,
}

type Connection = SplitSink<WebSocketStream<TcpStream>, Message>;

struct Client {
    id: u64,
    conn: Connection,
    room_id: Option<u64>,
}

enum RoomStatus {
    Public,
    Private(String),
}

struct Room {
    pub id: u64,
    pub owner_id: u64,
    pub client_ids: HashSet<u64>,
    pub status: RoomStatus,
}

enum WebMethods<T> {
    Create(Client),                    // client who creates the room
    Join(Client, u64, Option<String>), // client, room, password
    Action(u64, T),                    // client id, action data
    Leave(u64),                        // client id
}

// THIS IS AN EXAMPLE OF WHAT JOINT MACRO WOULD GENERATE.
// AND ALSO MY SANDBOX CURRENTLY.
struct MyJoint {
    state: DemoState,
}

enum Actions {
    IdentifyUser(String), // client name
    SendMessage(String),  // message content
    DeleteMessage(u64),   // message id
    PinMessage(u64),      // message id
}

impl MyJoint {

    // built with macro from given reducers
    async fn impl_identify_user(&mut self, client_id: u64, name: String) {
        self.state.user_names.insert(client_id, name);
    }

    async fn send_message(&mut self, client_id: u64, content: String) {
        let message = TextMessage {
            id: 0,
            content,
            author: client_id,
            pinned: false,
        };
        self.state.messages.push(message);
    }

    async fn delete_message(&mut self, client_id: u64, message_id: u64) {
        self.state.messages.retain(|msg| msg.id != message_id);
    }

    async fn pin_message(&mut self, client_id: u64, message_id: u64) {
        for msg in self.state.messages.iter_mut() {
            if msg.id == message_id {
                msg.pinned = true;
            }
        }
    }

    // special events
    async fn on_create(&mut self, client_id: u64, message_id: u64) {
        
    }

    async fn dispatch(&mut self, client_id: u64, action: Actions) {
        match action {
            Actions::IdentifyUser(name) => self.impl_identify_user(client_id, name).await,
            Actions::SendMessage(content) => self.send_message(client_id, content).await,
            Actions::DeleteMessage(id) => self.delete_message(client_id, id).await,
            Actions::PinMessage(id) => self.pin_message(client_id, id).await,
        }
    }

    pub async fn broadcast<T>(&mut self, mut rx: UnboundedReceiver<WebMethods<Actions>>) -> Result<(), String> {
        // map of all active client connections
        let mut clients: HashMap<u64, Client> = HashMap::<u64, Client>::new();

        // map of rooms
        let mut rooms: HashMap<u64, Room> = HashMap::<u64, Room>::new();

        let mut current_target_room_id: Option<u64> = None;

        while let Some(event) = rx.recv().await {
            let response: Result<Response<T>, Response<T>> = match event {
                WebMethods::Create(client) => {
                    let room_id = rooms.len() as u64;
                    let room = Room {
                        id: room_id,
                        owner_id: client.id,
                        client_ids: HashSet::<u64>::new(),
                        status: RoomStatus::Public,
                    };
                    rooms.insert(room_id, room);
                    clients.insert(client.id, client);

                    current_target_room_id = Some(room_id);

                    Ok(Response::RoomCreated(room_id))
                }
                WebMethods::Join(client, room_id, password) => {
                    let mut room = rooms.get_mut(&room_id).ok_or_else(|| {
                        Response::NotFound("Room not found".to_string())
                    })?;
                    room.client_ids.insert(client.id);
                    clients.insert(client.id, client);

                    current_target_room_id = Some(room_id);
                    Ok(Response::RoomJoined(room_id))
                }
                WebMethods::Action(client_id, action) => {
                    let client = clients.get(&client_id).ok_or_else(|| {
                        Err(Response::NotFound("Client not found".to_string()))
                    })?;
                    self.dispatch(client_id, action).await;
                    current_target_room_id = client.room_id;
                    Ok(Response::Action(self.state.clone()))
                }
                WebMethods::Leave(client_id) => {
                    let client = clients.remove(&client_id).ok_or_else(|| {
                        Response::NotFound("Client not found".to_string())
                    })?;
                    let room_id = client.room_id.ok_or_else(|| {
                        Response::NotFound("Client not in room".to_string())
                    })?;
                    let room = rooms.get_mut(&room_id).ok_or_else(|| {
                        Response::NotFound("Room not found".to_string())
                    })?;
                    room.client_ids.remove(&client_id);
                    current_target_room_id = Some(room_id);
                    Ok(Response::RoomLeft(room_id))
                }
            };

            let room_id = current_target_room_id.ok_or_else(|| {
                Response::NotFound("No room selected".to_string())
            })?;

            let room = rooms.get(&room_id).ok_or_else(|| {
                Response::NotFound("Room not found".to_string())
            })?;

            // broadcast response state to all clients in room
            for client_id in room.client_ids.iter() {
                if let Some(client) = clients.get_mut(client_id) {
                    let _ = client
                        .conn
                        .send(Message::from(serde_json::to_string(&response).unwrap()))
                        .await;
                }
            }
        }

        Ok(())
    }
}

// TRYING TO WRITE THESE MACROS BELOW

// #[build_injoint(foo, bar, baz)]
// struct MyJoint;
