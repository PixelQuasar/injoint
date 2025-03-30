use injoint::codegen::{reducer_actions, Broadcastable};
use injoint::joint::ws::WebsocketJoint;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Debug, Clone, Broadcastable)]
struct Message {
    pub author: u64,
    pub content: String,
}

#[derive(Serialize, Debug, Default, Clone, Broadcastable)]
struct State {
    users: HashMap<u64, String>,
    messages: Vec<Message>,
}

#[derive(Default, Serialize, Broadcastable)]
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

#[reducer_actions(State)]
impl Reducer {
    async fn identify_user(&mut self, client_id: u64, name: String) -> Result<String, String> {
        if self.state.users.contains_key(&client_id) {
            return Err("User already identified".to_string());
        }
        self.state.users.insert(client_id, name.clone());
        Ok(name)
    }

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
    joint.listen().await;
}
