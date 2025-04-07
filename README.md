# Injoint-rs

Injoint is a library for creating and managing WebSocket connections in declarative, inspired by state reducer approach.

## About Injoint

Core concept of injoint is `Joint` structure - a wrapper for all multithreaded asynchronous functionality
that is necessary to run a WebSocket server. It is responsible for managing the state of the application,
handling incoming messages, and dispatching them to the appropriate reducers for each room individually.

## Joint implementations

`Joint` structure is heuristically abstract and need to be implemented around some real-life conception,
for example, websockets or mpsc. `injoint` library currently provides these implementations for `AbstractJoint`:

- [`WebsocketJoint`](https://docs.rs/injoint/latest/injoint/joint/ws/struct.WebsocketJoint.html) - common implementation
  around asynchronous websocket connection
  using `tokio` and `tungstenite` libraries.
- [`AxumWSJoint`](https://docs.rs/injoint/latest/injoint/joint/ws/struct.AxumWSJoint.html) - another implementation
  around websocket that can be integrated
  into `axum` router.
- [`MPSCJoint`](https://docs.rs/injoint/latest/injoint/joint/ws/struct.MPSCJoint.html) - implementation around
  `tokio::sync::mpsc` channels.

## Usage

To use injoint, add this to your `Cargo.toml`:

```toml
[dependencies]
injoint = "0.1.0"
```

Example of minimalistic websocket chat server:

 ```rust
 use injoint::codegen::{reducer_actions, Broadcastable};
use injoint::joint::ws::WebsocketJoint;
use serde::Serialize;
use std::collections::HashMap;

// message struct, used in State
#[derive(Serialize, Debug, Clone, Broadcastable)]
struct Message {
    pub author: u64,
    pub content: String,
}

// chat state struct
#[derive(Serialize, Debug, Default, Clone, Broadcastable)]
struct State {
    users: HashMap<u64, String>,
    messages: Vec<Message>,
}

// state reducer, statically injected to `WebsocketJoint`
#[derive(Default, Serialize, Clone, Broadcastable)]
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

// using `reducer_actions` macro to generate boilerplate
// code implementing actions and their dispatching
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
    // initialize default reducer state
    let reducer = Reducer::new();

    // create root websocket joint instance
    let mut joint = WebsocketJoint::<Reducer>::new(reducer);

    // bind address to listen on
    joint.bind_addr("127.0.0.1:3000").await.unwrap();

    // start listening loop handling incoming connections
    joint.listen().await;
}
 ```

### Full documentation: https://docs.rs/injoint/latest/injoint/
