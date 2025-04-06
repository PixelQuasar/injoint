#![allow(unused)]

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum RoomStatus {
    Public,
    Private(String),
}

pub struct Room<R> {
    pub id: u64,
    pub owner_id: u64,
    pub client_ids: HashSet<u64>,
    pub status: RoomStatus,
    pub reducer: Arc<Mutex<R>>,
}

impl<R> Room<R> {
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
