pub mod broadcaster;
pub mod connection;
pub mod controller;
pub mod message;
pub mod reducer;
pub mod room;
pub mod store;
pub mod tcp_handler;
pub mod utils;
pub mod dispatcher;
pub mod joint;

use std::sync::Arc;

use futures_util::stream::SplitSink;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use serde::Serialize;
use tokio::net::TcpStream;
use crate::dispatcher::Dispatchable;
use crate::joint::Joint;
use crate::utils::types::Broadcastable;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

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

#[derive(Serialize, Debug, Clone)]
struct TextMessage {
    id: u64,
    author: u64,
    content: String,
    pinned: bool,
}

#[derive(Serialize, Debug, Clone)]
struct DemoState {
    messages: Vec<TextMessage>,
    user_names: HashMap<u64, String>,
}

impl Broadcastable for DemoState {
    fn new() -> Self {
        DemoState {
            messages: Vec::new(),
            user_names: HashMap::new(),
        }
    }
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

// THIS IS AN EXAMPLE OF WHAT JOINT MACRO WOULD GENERATE.
// AND ALSO MY SANDBOX CURRENTLY.
enum Actions {
    IdentifyUser(String), // client name
    SendMessage(String),  // message content
    DeleteMessage(u64),   // message id
    PinMessage(u64),      // message id
}

struct MyJointReducer {
    state: DemoState,
}

impl MyJointReducer {
    pub fn new() -> Self {
        MyJointReducer {
            state: DemoState::new()
        }
    }

    async fn action_impl_identify_user(
        &mut self, client_id: u64, name: String
    ) -> Result<DemoState, String> {
        let mut x = self.state.borrow_mut();
        self.state.user_names.insert(client_id, name);
        Ok(self.state.clone())
    }

    async fn action_send_message(&mut self, client_id: u64, content: String) -> Result<DemoState, String> {
        let message = TextMessage {
            id: 0,
            content,
            author: client_id,
            pinned: false,
        };
        self.state.messages.push(message);
        Ok(self.state.clone())
    }

    async fn action_delete_message(&mut self, client_id: u64, message_id: u64) -> Result<DemoState, String> {
        self.state.messages.retain(|msg| msg.id != message_id);
        Ok(self.state.clone())
    }

    async fn action_pin_message(&mut self, client_id: u64, message_id: u64) -> Result<DemoState, String> {
        for msg in self.state.messages.iter_mut() {
            if msg.id == message_id {
                msg.pinned = true;
            }
        }
        Ok(self.state.clone())
    }
}

impl Dispatchable<Actions, DemoState> for MyJointReducer {
    fn new() -> Self {
        MyJointReducer {
            state: DemoState::new()
        }
    }

    async fn dispatch(&mut self, client_id: u64, action: Actions) -> Result<DemoState, String> {
        let new_state = match action {
            Actions::IdentifyUser(name) => self.action_impl_identify_user(client_id, name).await?,
            Actions::SendMessage(content) => self.action_send_message(client_id, content).await?,
            Actions::DeleteMessage(id) => self.action_delete_message(client_id, id).await?,
            Actions::PinMessage(id) => self.action_pin_message(client_id, id).await?,
        };
        Ok(new_state)
    }
}


fn main () {
    let mut joint = Joint::<Actions, DemoState, MyJointReducer>::new();

}




// TRYING TO WRITE THESE MACROS BELOW

// #[build_injoint(foo, bar, baz)]
// struct MyJoint;
