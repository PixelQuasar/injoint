use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use Option;
use serde::ser::SerializeStruct;
use serde;
use crate::Client;

#[derive(Debug, Deserialize, Serialize)]
pub enum BroadcastMessageMethod {
    Create,
    Join,
    Leave,
    Delete,
    Action,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BroadcastMessage<T> {
    method: BroadcastMessageMethod,
    client_token: String,
    payload: Option<T>,
}

impl<T> BroadcastMessage<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new(
        method: BroadcastMessageMethod,
        client_token: String,
        payload: Option<T>,
    ) -> BroadcastMessage<T> {
        BroadcastMessage {
            method,
            client_token,
            payload,
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ResponseStatus {
    RoomCreated,
    RoomJoined,
    Action,
    RoomLeft,
    ServerError,
    ClientError,
    NotFound,
}

#[derive(Debug, Serialize)]
struct CreateRoomPayload {
    room_id: u64,
}

#[derive(Debug, Serialize)]
struct JoinRoomPayload {
    client_id: u64,
}

#[derive(Debug, Serialize)]
struct LeaveRoomPayload {
    client_id: u64,
}

#[derive(Debug)]
pub enum Response<T> where T : Serialize {
    RoomCreated(u64),
    RoomJoined(u64),
    Action(T),
    RoomLeft(u64),
    ServerError(String),
    ClientError(String),
    NotFound(String),
}

const RESPONSE_STR: &str = "Response";
const STATUS_STR: &str = "status";
const MESSAGE_STR: &str = "message";
impl<T> serde::ser::Serialize for Response<T>
where
    T: serde::ser::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut s = serializer.serialize_struct(RESPONSE_STR, 2)?;
        match self {
            Response::RoomCreated(room_id) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::RoomCreated)?;
                s.serialize_field(MESSAGE_STR, room_id)?;
            }
            Response::RoomJoined(client_id) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::RoomJoined)?;
                s.serialize_field(MESSAGE_STR, client_id)?;
            }
            Response::Action(payload) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::Action)?;
                s.serialize_field(MESSAGE_STR, payload)?;
            }
            Response::RoomLeft(client_id) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::RoomLeft)?;
                s.serialize_field(MESSAGE_STR, client_id)?;
            }
            Response::ServerError(message) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::ServerError)?;
                s.serialize_field(MESSAGE_STR, message)?;
            }
            Response::ClientError(message) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::ClientError)?;
                s.serialize_field(MESSAGE_STR, message)?;
            }
            Response::NotFound(message) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::NotFound)?;
                s.serialize_field(MESSAGE_STR, message)?;
            }
        }
        s.end()
    }
}