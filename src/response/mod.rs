/// This module contains the response types used in the injoint library.
mod test;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fmt::Debug;

/// Enum representing the status of a response, available in joint.
///
/// This enum is used to categorize the type of response being sent back to the client.
///
/// # example
/// ```
/// use injoint::response::ResponseStatus;
///
/// let status = ResponseStatus::RoomCreated;
/// assert_eq!(format!("{:?}", status), "RoomCreated");
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    /// Indicates that a room has been created successfully. Per-room response.
    RoomCreated,
    /// Indicates that a client has joined a room successfully. Per-room response.
    RoomJoined,
    /// Each new user in room receives a state object individually when they join. Per-client response.
    StateSent,
    /// Indicates that an action has been sent to the room. Per-room response.
    Action,
    /// Indicates that a client has left the room. Per-room response.
    RoomLeft,
    /// Indicates a server error occurred. Per-client response.
    ServerError,
    /// Indicates a client error occurred. Per-client response.
    ClientError,
    /// Indicates that a room was not found. Per-client response.
    NotFound,
}

/// Enum representing the response sent back to the client, available in joint.
///
/// This enum is used to encapsulate the different types of responses that can be sent back to the client.
/// Implements `Serialize` and `Deserialize` traits from `serde`.
///
/// # example
/// ```
/// use injoint::response::Response;
/// use serde_json;
///
/// let response = Response::RoomCreated(123);
/// let json = serde_json::to_string(&response).unwrap();
/// assert_eq!(json, r#"{"status":"RoomCreated","message":123}"#);
/// ```
///
#[derive(Debug, Clone)]
pub enum Response {
    /// Indicates that a room has been created successfully. Per-room response.
    RoomCreated(u64),
    /// Indicates that a client has joined a room successfully. Per-room response.
    RoomJoined(u64),
    /// Each new user in room receives a state object individually when they join. Per-client response.
    StateSent(String),
    /// Indicates that an action has been sent to the room. Per-room response.
    Action(String), // maybe this should be a generic type that serializable?
    /// Indicates that a client has left the room. Per-room response.
    RoomLeft(u64),
    /// Indicates a server error occurred. Per-client response.
    ServerError(String),
    /// Indicates a client error occurred. Per-client response.
    ClientError(String),
    /// Indicates that a room was not found. Per-client response.
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

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        enum Field {
            Status,
            Message,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`status` or `message`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            STATUS_STR => Ok(Field::Status),
                            MESSAGE_STR => Ok(Field::Message),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ResponseVisitor;

        impl<'de> Visitor<'de> for ResponseVisitor {
            type Value = Response;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Response")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Response, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut status: Option<ResponseStatus> = None;
                // Use Value initially for message to handle different types
                let mut message_value: Option<Value> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Status => {
                            if status.is_some() {
                                return Err(de::Error::duplicate_field(STATUS_STR));
                            }
                            // Deserialize status directly into ResponseStatus enum
                            status = Some(map.next_value()?);
                        }
                        Field::Message => {
                            if message_value.is_some() {
                                return Err(de::Error::duplicate_field(MESSAGE_STR));
                            }
                            // Deserialize message as a generic Value first
                            message_value = Some(map.next_value()?);
                        }
                    }
                }

                let status = status.ok_or_else(|| de::Error::missing_field(STATUS_STR))?;
                let message_value =
                    message_value.ok_or_else(|| de::Error::missing_field(MESSAGE_STR))?;

                // Now, based on status, parse message_value into the specific type
                match status {
                    ResponseStatus::RoomCreated
                    | ResponseStatus::RoomJoined
                    | ResponseStatus::RoomLeft => {
                        let id = message_value.as_u64().ok_or_else(|| {
                            de::Error::invalid_type(
                                de::Unexpected::Other("non-u64 value"),
                                &"an unsigned 64-bit integer",
                            )
                        })?;
                        match status {
                            ResponseStatus::RoomCreated => Ok(Response::RoomCreated(id)),
                            ResponseStatus::RoomJoined => Ok(Response::RoomJoined(id)),
                            ResponseStatus::RoomLeft => Ok(Response::RoomLeft(id)),
                            _ => unreachable!(), // Should not happen due to outer match
                        }
                    }
                    ResponseStatus::StateSent | ResponseStatus::Action => {
                        // For StateSent and Action, we expect the payload as a string (could be stringified JSON or plain string)
                        // We store it as a string in the enum variant.
                        let payload_str = message_value.to_string(); // Convert the Value back to string representation
                                                                     // If it was originally a string, remove quotes added by to_string()
                        let payload_str = if message_value.is_string() {
                            message_value.as_str().unwrap_or(&payload_str).to_string()
                        } else {
                            payload_str
                        };

                        match status {
                            ResponseStatus::StateSent => Ok(Response::StateSent(payload_str)),
                            ResponseStatus::Action => Ok(Response::Action(payload_str)),
                            _ => unreachable!(),
                        }
                    }
                    ResponseStatus::ServerError
                    | ResponseStatus::ClientError
                    | ResponseStatus::NotFound => {
                        let msg = message_value
                            .as_str()
                            .ok_or_else(|| {
                                de::Error::invalid_type(
                                    de::Unexpected::Other("non-string value"),
                                    &"a string",
                                )
                            })?
                            .to_string();
                        match status {
                            ResponseStatus::ServerError => Ok(Response::ServerError(msg)),
                            ResponseStatus::ClientError => Ok(Response::ClientError(msg)),
                            ResponseStatus::NotFound => Ok(Response::NotFound(msg)),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }

        const FIELDS: &'static [&'static str] = &[STATUS_STR, MESSAGE_STR];
        deserializer.deserialize_struct(RESPONSE_STR, FIELDS, ResponseVisitor)
    }
}

/// Struct representing per-room responses.
///
/// This struct is used to encapsulate the room ID and the response type.
#[derive(Debug)]
pub(crate) struct RoomResponse {
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

/// Struct representing per-client responses.
///
/// This struct is used to encapsulate the client ID and the response type.
#[derive(Debug)]
pub(crate) struct ClientResponse {
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
