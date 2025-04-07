/// This module contains the `JointMessage` struct and the `JointMessageMethod` enum.
mod test;

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Message method enum that represents messages receiving by `Joint`.
///
/// This enum is used to represent the different types of messages that can be sent
/// to the joint. It is used in the `JointMessage` struct to determine the type of message.
/// Implements `Deserialize`, `Serialize`, and `Clone` traits.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum JointMessageMethod {
    /// Create a new room (triggering RoomCreated response)
    Create,
    /// Join an existing room by id (triggering RoomJoined and StateSent responses)
    Join(u64),
    /// Leave the current room (triggering RoomLeft response)
    Leave,
    /// Send a message to the room (triggering Action response)
    Action(String), // maybe this should be a generic type that deserializable?
}

/// JointMessage struct that represents a message received by the `Joint`.
///
/// This struct is used to encapsulate the message method and the client token.
/// It is used to send messages to the joint and to receive messages from the joint.
/// It implements the `Deserialize`, `Serialize`, and `Clone` traits.
///
/// # examples
///
/// ```rust
/// use injoint::message::JointMessageMethod;
/// use injoint::message::JointMessage;
///
/// let message = JointMessage::new(JointMessageMethod::Create, String::new());
///
/// let json = serde_json::to_string(&message).unwrap();
///
/// println!("Serialized message: {}", json);
///
/// let deserialized: JointMessage = serde_json::from_str(&json).unwrap();
///
/// println!("Deserialized message: {:?}", deserialized);
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JointMessage {
    /// The message method that represents the type of message.
    pub message: JointMessageMethod,
    /// The client token that is used to identify the client.
    /// (usage is developer-defined, not used for anything yet)
    pub client_token: String,
}

impl JointMessage {
    /// Creates a new `JointMessage` instance.
    pub fn new(message: JointMessageMethod, client_token: String) -> Self {
        JointMessage {
            message,
            client_token,
        }
    }
}
