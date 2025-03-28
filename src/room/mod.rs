#![allow(unused)]

use std::collections::HashSet;

#[derive(Debug)]
pub enum RoomStatus {
    Public,
    Private(String),
}

#[derive(Debug)]
pub struct Room {
    pub id: u64,
    pub owner_id: u64,
    pub client_ids: HashSet<u64>,
    pub status: RoomStatus,
}
