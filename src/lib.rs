/* про клонирование стейта на каждом действии:
 * из-за того что вся многопоточка на мьютексах, нормально мутировать состояние на каждом экшне
 * не получится, следовательно, взаимодействуем с мьютексом только в диспатчере, а все остальные
 * мутации придется положить на модель "возвращаем новое состояние вместо мутирования старого"
 *
 * как потом можно пофиксить: убрать мьютексы, убрать лишние &mut self и перенести
 * хэндлинг многопоточки на mpsc каналы
 */

pub mod broadcaster;
mod client;
pub mod connection;
pub mod controller;
pub mod dispatcher;
pub mod joint;
pub mod joint_impl;
pub mod message;
pub mod reducer;
mod response;
pub mod room;
pub mod store;
pub mod tcp_handler;
pub mod utils;

use crate::dispatcher::Dispatchable;
use crate::joint_impl::websocket_joint::WebsocketJoint;
use crate::utils::types::{Broadcastable, Receivable};
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use tokio::net::TcpListener;
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
// joint_impl MyJoint {
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

#[derive(Serialize, Debug, Clone, Default)]
struct DemoState {
    messages: Vec<TextMessage>,
    user_names: HashMap<u64, String>,
}
impl Broadcastable for DemoState {}

struct WebMessage {
    sender: u64,
    data: String,
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
#[derive(Deserialize, Debug)]
enum Actions {
    IdentifyUser(String), // client name
    SendMessage(String),  // message content
    DeleteMessage(u64),   // message id
    PinMessage(u64),      // message id
}
impl Receivable for Actions {}

struct MyJointReducer {
    state: DemoState,
}

impl MyJointReducer {
    async fn action_identify_user(
        &mut self,
        //state: &mut DemoState,
        client_id: u64,
        name: String,
    ) -> Result<(), String> {
        let mut x = self.state.borrow_mut();
        self.state.user_names.insert(client_id, name);
        Ok(())
    }

    async fn action_send_message(
        &mut self,
        //state: &mut DemoState,
        client_id: u64,
        content: String,
    ) -> Result<(), String> {
        let message = TextMessage {
            id: 0,
            content,
            author: client_id,
            pinned: false,
        };
        self.state.messages.push(message);
        Ok(())
    }

    async fn action_delete_message(
        &mut self,
        //state: &mut DemoState,
        client_id: u64,
        message_id: u64,
    ) -> Result<(), String> {
        self.state.messages.retain(|msg| msg.id != message_id);
        Ok(())
    }

    async fn action_pin_message(
        &mut self,
        //state: &mut DemoState,
        client_id: u64,
        message_id: u64,
    ) -> Result<(), String> {
        for msg in self.state.messages.iter_mut() {
            if msg.id == message_id {
                msg.pinned = true;
            }
        }
        Ok(())
    }
}

impl Dispatchable for MyJointReducer {
    type Action = Actions;
    type Response = DemoState;
    fn new() -> Self {
        MyJointReducer {
            state: DemoState::default(),
        }
    }

    async fn dispatch(&mut self, client_id: u64, action: Actions) -> Result<DemoState, String> {
        match action {
            Actions::IdentifyUser(name) => {
                self.action_identify_user(client_id, name).await?;
            }
            Actions::SendMessage(content) => {
                self.action_send_message(client_id, content).await?;
            }
            Actions::DeleteMessage(id) => {
                self.action_delete_message(client_id, id).await?;
            }
            Actions::PinMessage(id) => {
                self.action_pin_message(client_id, id).await?;
            }
        }

        Ok(self.state.clone())
    }
}

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let mut joint = WebsocketJoint::<MyJointReducer>::new();

    joint.poll(server).await;
}

// TRYING TO WRITE THESE MACROS BELOW

// #[build_injoint(foo, bar, baz)]
// struct MyJoint;
