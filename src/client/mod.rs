#![allow(unused)]

pub struct Client {
    pub id: u64,
    pub room_id: Option<u64>,
    pub label: String,
    pub token: String,
}

impl Client {
    pub fn new(id: u64, room_id: Option<u64>, label: String, token: String) -> Self {
        Client {
            id,
            room_id,
            label,
            token,
        }
    }
}
