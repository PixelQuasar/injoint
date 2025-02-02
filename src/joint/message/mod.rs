use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt::Debug;
use Option;

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

#[derive(Debug, Serialize)]
pub struct Response {
    status: ResponseStatus,
    message: String,
}

impl Response {
    pub fn new(status: ResponseStatus, message: String) -> Response {
        Response { status, message }
    }

    pub fn action<T>(state: T) -> Response
    where
        T: Serialize,
    {
        Response {
            status: ResponseStatus::Action,
            message: serde_json::to_string(&state).unwrap(),
        }
    }

    pub fn room_created(room_id: u64) -> Response {
        let payload = CreateRoomPayload { room_id };
        Response {
            status: ResponseStatus::RoomCreated,
            message: serde_json::to_string(&payload).unwrap(),
        }
    }

    pub fn room_joined(client_id: u64) -> Response {
        let payload = JoinRoomPayload { client_id };
        Response {
            status: ResponseStatus::RoomJoined,
            message: serde_json::to_string(&payload).unwrap(),
        }
    }

    pub fn room_left(client_id: u64) -> Response {
        let payload = LeaveRoomPayload { client_id };
        Response {
            status: ResponseStatus::RoomLeft,
            message: serde_json::to_string(&payload).unwrap(),
        }
    }

    pub fn server_error(message: String) -> Response {
        Response {
            status: ResponseStatus::ServerError,
            message,
        }
    }

    pub fn client_error(message: String) -> Response {
        Response {
            status: ResponseStatus::ClientError,
            message,
        }
    }

    pub fn not_found(message: String) -> Response {
        Response {
            status: ResponseStatus::NotFound,
            message,
        }
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Default for Response {
    fn default() -> Self {
        Response {
            status: ResponseStatus::ServerError,
            message: "Server error".to_string(),
        }
    }
}
