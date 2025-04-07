#![allow(unused)]
/// This module defines the `Client` struct, which represents a participant in a room.

/// A client is a participant in a room.
///
/// Each client has a unique ID, an optional room ID, a label (username), and a token.
pub struct Client {
    pub id: u64,
    pub room_id: Option<u64>,
    pub label: String,
    pub token: String,
}

impl Client {
    /// Creates a new client with the given ID, room ID, label, and token.
    pub fn new(id: u64, room_id: Option<u64>, label: String, token: String) -> Self {
        Client {
            id,
            room_id,
            label,
            token,
        }
    }
}
