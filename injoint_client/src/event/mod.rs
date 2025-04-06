use serde::Deserialize;

#[derive(Deserialize)]
pub struct ActionPayload<A: Send, T: Send> {
    #[serde(flatten)]
    pub action: A,
    pub client_id: u64,
    pub state: T,
}

#[derive(Deserialize)]
#[serde(tag = "status", content = "message")]
pub enum Event<A: Send, T: Send> {
    RoomCreated(u64),
    RoomJoined(u64),
    StateSent(T),
    Action(ActionPayload<A, T>),
    RoomLeft(u64),
    ServerError(String),
    ClientError(String),
    NotFound(String),
}
