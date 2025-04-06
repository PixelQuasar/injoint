use serde::ser::SerializeStruct;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Debug;

#[derive(Debug, Serialize)]
pub enum ResponseStatus {
    RoomCreated,
    RoomJoined,
    StateSent,
    Action,
    RoomLeft,
    ServerError,
    ClientError,
    NotFound,
}

#[derive(Debug, Clone)]
pub enum Response {
    RoomCreated(u64),
    RoomJoined(u64),
    StateSent(String),
    Action(String), // maybe this should be a generic type that serializable?
    RoomLeft(u64),
    ServerError(String),
    ClientError(String),
    NotFound(String),
}

const ROOM_STR: &str = "room";
const PAYLOAD_STR: &str = "payload";
const RESPONSE_STR: &str = "response";
const STATUS_STR: &str = "status";
const MESSAGE_STR: &str = "message";
const CLIENT_STR: &str = "client";
const ERROR_STR: &str = "error";
impl serde::ser::Serialize for Response {
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
            Response::StateSent(payload) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::StateSent)?;
                match serde_json::from_str::<Value>(payload) {
                    Ok(json_value) => s.serialize_field("message", &json_value)?,
                    Err(_) => {
                        s.serialize_field("message", payload)?;
                    }
                }
            }
            Response::Action(payload) => {
                s.serialize_field(STATUS_STR, &ResponseStatus::Action)?;
                match serde_json::from_str::<Value>(payload) {
                    Ok(json_value) => s.serialize_field("message", &json_value)?,
                    Err(_) => {
                        s.serialize_field("message", payload)?;
                    }
                }
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

#[derive(Debug)]
pub struct RoomResponse {
    pub room: u64,
    pub response: Response,
}

impl RoomResponse {
    pub fn create_room(room: u64) -> Self {
        RoomResponse {
            room,
            response: Response::RoomCreated(room),
        }
    }

    pub fn join_room(room: u64, client: u64) -> Self {
        RoomResponse {
            room,
            response: Response::RoomJoined(client),
        }
    }

    pub fn action(room: u64, payload: String) -> Self {
        RoomResponse {
            room,
            response: Response::Action(payload),
        }
    }

    pub fn leave_room(room: u64, client: u64) -> Self {
        RoomResponse {
            room,
            response: Response::RoomLeft(client),
        }
    }
}

impl serde::ser::Serialize for RoomResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut s = serializer.serialize_struct(RESPONSE_STR, 2)?;
        s.serialize_field(ROOM_STR, &self.room)?;
        s.serialize_field(PAYLOAD_STR, &self.response)?;
        s.end()
    }
}

#[derive(Debug)]
pub struct ClientResponse {
    pub client: u64,
    pub response: Response,
}

impl ClientResponse {
    pub fn server_error(client: u64, message: String) -> Self {
        ClientResponse {
            client,
            response: Response::ServerError(message),
        }
    }

    pub fn client_error(client: u64, message: String) -> Self {
        ClientResponse {
            client,
            response: Response::ClientError(message),
        }
    }

    pub fn not_found(client: u64, message: String) -> Self {
        ClientResponse {
            client,
            response: Response::NotFound(message),
        }
    }
}

impl serde::ser::Serialize for ClientResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut s = serializer.serialize_struct(RESPONSE_STR, 2)?;
        s.serialize_field(CLIENT_STR, &self.client)?;
        s.serialize_field(ERROR_STR, &self.response)?;
        s.end()
    }
}
