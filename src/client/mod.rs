// use async_trait::async_trait;
//
// #[async_trait]
// pub trait Client {
//     type Connection;
//
//     fn get_id(&self) -> u64;
//
//     fn set_id(&mut self, id: u64);
//
//     fn get_room_id(&self) -> Option<u64>;
//
//     fn set_room_id(&mut self, room_id: u64);
//
//     fn set_connection(&mut self, conn: Self::Connection);
//
//     fn get_connection(&self) -> &Self::Connection;
//
//     async fn send_message(&mut self, msg: String);
// }

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
