#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::{ClientResponse, Response, RoomResponse};
    use serde_json::{json, Value};

    #[test]
    fn test_response_serialization() {
        // Test RoomCreated response
        let response = Response::RoomCreated(123);
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "status": "RoomCreated",
                "message": 123
            })
        );

        // Test RoomJoined response
        let response = Response::RoomJoined(456);
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "status": "RoomJoined",
                "message": 456
            })
        );

        // Test StateSent response
        let state_json = r#"{"value": 42, "name": "test"}"#;
        let response = Response::StateSent(state_json.to_string());
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "status": "StateSent",
                "message": {"value": 42, "name": "test"}
            })
        );

        // Test Action response
        let action_json = r#"{"type": "increment", "value": 5}"#;
        let response = Response::Action(action_json.to_string());
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "status": "Action",
                "message": {"type": "increment", "value": 5}
            })
        );

        // Test error responses
        let response = Response::ServerError("Server error".to_string());
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "status": "ServerError",
                "message": "Server error"
            })
        );
    }

    #[test]
    fn test_room_response() {
        // Test create_room factory method
        let room_id = 123;
        let response = RoomResponse::create_room(room_id);
        assert_eq!(response.room, room_id);
        if let Response::RoomCreated(id) = response.response {
            assert_eq!(id, room_id);
        } else {
            panic!("Expected RoomCreated response");
        }

        // Test serialization
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "room": room_id,
                "payload": {
                    "status": "RoomCreated",
                    "message": room_id
                }
            })
        );

        // Test join_room
        let client_id = 456;
        let response = RoomResponse::join_room(room_id, client_id);
        if let Response::RoomJoined(id) = response.response {
            assert_eq!(id, client_id);
        } else {
            panic!("Expected RoomJoined response");
        }

        // Test action
        let payload = r#"{"value": 42}"#.to_string();
        let response = RoomResponse::action(room_id, payload.clone());
        if let Response::Action(p) = &response.response {
            assert_eq!(p, &payload);
        } else {
            panic!("Expected Action response");
        }

        // Test leave_room
        let response = RoomResponse::leave_room(room_id, client_id);
        if let Response::RoomLeft(id) = response.response {
            assert_eq!(id, client_id);
        } else {
            panic!("Expected RoomLeft response");
        }
    }

    #[test]
    fn test_client_response() {
        // Test server_error factory method
        let client_id = 123;
        let message = "Server error message".to_string();
        let response = ClientResponse::server_error(client_id, message.clone());
        assert_eq!(response.client, client_id);
        if let Response::ServerError(msg) = &response.response {
            assert_eq!(msg, &message);
        } else {
            panic!("Expected ServerError response");
        }

        // Test serialization
        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(
            serialized,
            json!({
                "client": client_id,
                "error": {
                    "status": "ServerError",
                    "message": message
                }
            })
        );

        // Test client_error
        let message = "Client error message".to_string();
        let response = ClientResponse::client_error(client_id, message.clone());
        if let Response::ClientError(msg) = &response.response {
            assert_eq!(msg, &message);
        } else {
            panic!("Expected ClientError response");
        }

        // Test not_found
        let message = "Not found message".to_string();
        let response = ClientResponse::not_found(client_id, message.clone());
        if let Response::NotFound(msg) = &response.response {
            assert_eq!(msg, &message);
        } else {
            panic!("Expected NotFound response");
        }
    }
}
