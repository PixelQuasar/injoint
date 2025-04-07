//! Injoint is a library for creating and managing WebSocket connections in declarative, inspired by state reducer approach.
//!
//! # About Injoint
//! Core concept of injoint is `Joint` structure - a wrapper for all multithreaded asynchronous functionality
//! that is necessary to run a WebSocket server. It is responsible for managing the state of the application,
//! handling incoming messages, and dispatching them to the appropriate reducers for each room individually.
//!
//! # Joint implementations
//! `Joint` structure is heuristically abstract and need to be implemented around some real-life conception,
//! for example, websockets or mpsc. `injoint` library currently provides these implementations for `AbstractJoint`:
//! - [`WebsocketJoint`](joint::ws::WebsocketJoint) - common implementation around asynchronous websocket connection
//! using `tokio` and `tungstenite` libraries.
//! - [`AxumWSJoint`](joint::axum::AxumWSJoint) - another implementation around websocket that can be integrated
//! into `axum` router.
//! - [`MPSCJoint`](joint::mpsc::MPSCJoint) - implementation around `tokio::sync::mpsc` channels.
//!
//! # Usage
//! Example of minimalistic websocket chat server taken from [GitHub repository](https://github.com/PixelQuasar/injoint):
//!
//! ```rust
//! use injoint::codegen::{reducer_actions, Broadcastable};
//! use injoint::joint::ws::WebsocketJoint;
//! use serde::Serialize;
//! use std::collections::HashMap;
//!
//! // message struct, used in State
//! #[derive(Serialize, Debug, Clone, Broadcastable)]
//! struct Message {
//!     pub author: u64,
//!     pub content: String,
//! }
//!
//! // chat state struct
//! #[derive(Serialize, Debug, Default, Clone, Broadcastable)]
//! struct State {
//!     users: HashMap<u64, String>,
//!     messages: Vec<Message>,
//! }
//!
//! // state reducer, statically injected to `WebsocketJoint`
//! #[derive(Default, Serialize, Clone, Broadcastable)]
//! struct Reducer {
//!     state: State,
//! }
//!
//! impl Reducer {
//!     pub fn new() -> Self {
//!         Reducer {
//!             state: State {
//!                 users: HashMap::new(),
//!                 messages: Vec::new(),
//!             },
//!         }
//!     }
//! }
//!
//! // using `reducer_actions` macro to generate boilerplate
//! // code implementing actions and their dispatching
//! #[reducer_actions(State)]
//! impl Reducer {
//!     async fn identify_user(&mut self, client_id: u64, name: String) -> Result<String, String> {
//!         if self.state.users.contains_key(&client_id) {
//!             return Err("User already identified".to_string());
//!         }
//!         self.state.users.insert(client_id, name.clone());
//!         Ok(name)
//!     }
//!
//!     async fn send_message(&mut self, client_id: u64, text: String) -> Result<String, String> {
//!         if !self.state.users.contains_key(&client_id) {
//!             return Err("User not identified".to_string());
//!         }
//!         self.state.messages.push(Message {
//!             author: client_id,
//!             content: text.clone(),
//!         });
//!         Ok(text)
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // initialize default reducer state
//!     let reducer = Reducer::new();
//!
//!     // create root websocket joint instance
//!     let mut joint = WebsocketJoint::<Reducer>::new(reducer);
//!
//!     // bind address to listen on
//!     joint.bind_addr("127.0.0.1:3000").await.unwrap();
//!
//!    // start listening loop handling incoming connections
//!     joint.listen().await;
//! }
//! ```
//!
//! #### And then just build and run it with
//! ```bash
//! cargo run
//! ```
//!
//! #### As websocket client, you may send one of four types of methods:
//! - `Create` - create a new room
//! example:
//! ```json
//! {
//! "message": {
//!     "type": "Create",
//! },
//! "client_token": "" // doesn't affect builtin logic, you can use this token to identify your client
//! }
//! ```
//! - `Join` - join an existing room by id
//! example:
//! ```json
//! {
//! "message": {
//!     "type": "Join",
//! 	"data": 0 // room id
//! },
//! "client_token": ""
//! }
//! ```
//! - `Action` - perform one of actions defined in your `Reducer`
//! example:
//! ```json
//! {
//! "message": {
//!     "type": "Action",
//! 	"data": "{\"type\":\"ActionIdentifyUser\",\"data\":\"quasarity\"}" // action payload
//!  },
//!  "client_token": ""
//!  }
//! ```
//! - `Leave` - leave current room
//! example:
//! ```json
//! {
//! "message": {
//!     "type": "Jeave",
//! },
//! "client_token": ""
//! }
//! ```
//!
//! #### And server will respond with one of four types of messages:
//! - `RoomCreated` - room created successfully
//! example:
//! ```json
//! {
//! "status": "RoomCreated",
//! "message": 0 // room id
//! }
//! ```
//! - `RoomJoined` - joined existing room successfully
//! example:
//! ```json
//! {
//! "status": "RoomJoined",
//! "message": 0 // client id
//! }
//! ```
//! - `StateSent` - state sent successfully, sent to each client individually
//! example:
//! ```json
//! {
//! "status": "StateSent",
//! "message": "{
//!        "users": {
//!            "0": "quasarity"
//!        },
//!        "messages": [
//!             {
//!                 "author": 0,
//!                 "content": "Hello, world!"
//!             }
//!         ]}"
//!     }
//! }
//! ```
//! - `Action` - action performed successfully, sent to each client in room
//! example:
//! ```json
//! {
//! 	"status": "Action",
//! 	"message": {
//! 		"author": 0,
//! 		"data": "quasarity",
//! 		"state": {
//! 			"messages": [],
//! 			"users": {
//! 				"0": "quasarity",
//! 			}
//! 		},
//! 		"status": "ActionIdentifyUser"
//! 	}
//! }
//! ```
//! - `RoomLeft` - left current room successfully
//! example:
//! ```json
//! {
//! "status": "RoomLeft",
//! "message": 0 // client id
//! }
//!
/// Broadcaster is core structure responsible handling room-split communication and multiple reducers.
mod broadcaster;

/// Client is a structure that represents a client connected to the server.
mod client;

/// Connection is a structure that represents a connection to a client.
pub mod connection;

/// Dispatcher is a structure that handles incoming messages and dispatches them to the appropriate reducer.
pub mod dispatcher;

/// Joint is a structure that represents a joint implementation for real-time communication.
pub mod joint;

/// Message is a structure that represents a message sent over the WebSocket connection.
pub mod message;

/// Response is a structure that represents a response sent back to the client.
pub mod response;

/// Room is a structure that represents a room in which clients can join and communicate.
mod room;

/// State is a structure that represents the state of the application.
pub mod utils;

/// This module contains the code generation macros for injoint.
pub mod codegen {
    /// This module contains the code generation macros for injoint.
    pub use injoint_macros::*;
}
