use injoint::codegen::reducer_actions;
use injoint::dispatcher::{ActionResponse, Dispatchable};
use injoint::joint::ws::WebsocketJoint;
use injoint::utils::types::{Broadcastable, Receivable};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Debug, Clone)]
struct Message {
    pub author: u64,
    pub content: String,
}

#[derive(Serialize, Debug, Clone)]
struct State {
    users: HashMap<u64, String>,
    messages: Vec<Message>,
}
impl Broadcastable for State {}

struct Reducer {
    state: State,
}

impl Reducer {
    pub fn new() -> Self {
        Reducer {
            state: State {
                users: HashMap::new(),
                messages: Vec::new(),
            },
        }
    }
}

enum Actions {
    IdentifyUser(String),
    SendMessage(String),
}

impl Reducer {
    fn dispatch(&mut self, client_id: u64, action: Actions) -> Result<String, String> {
        match action {
            Actions::IdentifyUser(name) => {
                if self.state.users.contains_key(&client_id) {
                    return Err("User already identified".to_string());
                }
                self.state.users.insert(client_id, name.clone());
                Ok(name)
            }
            Actions::SendMessage(text) => {
                if !self.state.users.contains_key(&client_id) {
                    return Err("User not identified".to_string());
                }
                self.state.messages.push(Message {
                    author: client_id,
                    content: text.clone(),
                });
                Ok(text)
            }
        }
    }
}

#[reducer_actions(State, Actions)]
impl Reducer {
    #[reducer_action(Actions::IdentifyUser)]
    async fn identify_user(&mut self, client_id: u64, name: String) -> Result<String, String> {
        if self.state.users.contains_key(&client_id) {
            return Err("User already identified".to_string());
        }
        self.state.users.insert(client_id, name.clone());
        Ok(name)
    }

    #[reducer_action(Actions::IdentifyUser)]
    async fn send_message(&mut self, client_id: u64, text: String) -> Result<String, String> {
        if !self.state.users.contains_key(&client_id) {
            return Err("User not identified".to_string());
        }
        self.state.messages.push(Message {
            author: client_id,
            content: text.clone(),
        });
        Ok(text)
    }
}

#[tokio::main]
async fn main() {
    let reducer = Reducer::new();
    let mut joint = WebsocketJoint::<Reducer>::new(reducer);
    joint.bind_addr("127.0.0.1:3000").await;
}
