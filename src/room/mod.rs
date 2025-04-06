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
