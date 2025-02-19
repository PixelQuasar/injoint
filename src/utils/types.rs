use std::fmt::Debug;
use serde::Serialize;
use crate::Client;

pub trait Broadcastable: Serialize + Debug {
    fn new() -> Self;
}

pub enum WebMethods<T> {
    Create(Client),                    // client who creates the room
    Join(Client, u64, Option<String>), // client, room, password
    Action(u64, T),                    // client id, action data
    Leave(u64),                        // client id
}
