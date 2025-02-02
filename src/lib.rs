// use tokio::{net::TcpListener, net::TcpStream, sync::mpsc};
// use tokio_tungstenite::{accept_async, WebSocketStream};
// use injoint_macros::build_injoint;
// use paste;
mod joint;

use crate::joint::message::Response;
use dashmap::DashMap;
use futures::SinkExt;
use futures_util::stream::SplitSink;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use tokio::net::TcpStream;
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tungstenite::{client, Utf8Bytes};
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

    pub async fn broadcast(&mut self, mut rx: UnboundedReceiver<WebMethods<Actions>>) {
        // map of all active client connections
        let mut clients: HashMap<u64, Client> = HashMap::<u64, Client>::new();

        // map of rooms
        let mut rooms: HashMap<u64, Room> = HashMap::<u64, Room>::new();

        let mut current_target_room_id: Option<u64> = None;

        while let Some(event) = rx.recv().await {
            let response: Response = match event {
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

                    Response::room_created(self.state.clone())
                }
                WebMethods::Join(client, room_id, password) => {
                    if let Some(mut room) = rooms.get_mut(&room_id) {
                        room.client_ids.insert(client.id);
                        clients.insert(client.id, client);

                        current_target_room_id = Some(room_id);
                        Response::ok(self.state.clone())
                    } else {
                        Response::not_found("Room not found".to_string())
                    }
                }
                WebMethods::Action(client_id, action) => {
                    if let Some(client) = clients.get(&client_id) {
                        self.dispatch(client_id, action).await;

                        current_target_room_id = client.room_id;
                        Response::ok(self.state.clone())
                    } else {
                        Response::not_found("Client not found".to_string())
                    }
                }
                WebMethods::Leave(client_id) => {
                    if let Some(client) = clients.remove(&client_id) {
                        if let Some(room) = rooms.get_mut(&client.room_id.unwrap()) {
                            room.client_ids.remove(&client_id);
                        }
                        current_target_room_id = client.room_id;
                        Response::ok(self.state.clone())
                    } else {
                        Response::not_found("Client not found".to_string())
                    }
                }
            };

            if let Some(room_id) = current_target_room_id {
                if let Some(room) = rooms.get(&room_id) {
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
            }
        }
    }
}

// TRYING TO WRITE THESE MACROS BELOW

// #[build_injoint(foo, bar, baz)]
// struct MyJoint;
