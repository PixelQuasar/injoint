#![allow(unused)]
/// This module defines the `Room` struct and its associated types.
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A room status can be either public or private.
pub enum RoomStatus {
    /// Public room, accessible to all clients.
    Public,
    /// Private room, accessible only to the owner and invited clients.
    Private(String),
}

/// A room is a container for clients and their shared state.
///
/// R is the type of the reducer that manages the state of the room.
/// The reducer is responsible for handling actions and updating the state.
///
pub struct Room<R> {
    /// The ID of the room.
    pub id: u64,
    /// The ID of the owner of the room.
    pub owner_id: u64,
    /// The status of the room, either public or private.
    pub status: RoomStatus,
    /// The set of client IDs that are currently in the room.
    pub client_ids: HashSet<u64>,
    /// The reducer that manages the state of the room.
    pub reducer: Arc<Mutex<R>>,
}

impl<R> Room<R> {
    /// Creates a new room with the given ID, owner ID, client IDs, status, and reducer.
    pub fn new(
        id: u64,
        owner_id: u64,
        client_ids: HashSet<u64>,
        status: RoomStatus,
        reducer: Arc<Mutex<R>>,
    ) -> Self {
        Room {
            id,
            owner_id,
            client_ids,
            status,
            reducer,
        }
    }
}
