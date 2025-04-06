#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{JointMessage, JointMessageMethod};
    use serde_json::{json, Value};

    #[test]
    fn test_joint_message_creation() {
        let message = JointMessage::new(JointMessageMethod::Create, "client123".to_string());
        assert_eq!(message.client_token, "client123");
        assert!(matches!(message.message, JointMessageMethod::Create));

        let room_id = 42;
        let message = JointMessage::new(JointMessageMethod::Join(room_id), "client456".to_string());
        assert_eq!(message.client_token, "client456");
        if let JointMessageMethod::Join(id) = message.message {
            assert_eq!(id, room_id);
        } else {
            panic!("Expected Join message");
        }

        let message = JointMessage::new(JointMessageMethod::Leave, "client789".to_string());
        assert_eq!(message.client_token, "client789");
        assert!(matches!(message.message, JointMessageMethod::Leave));

        let action_data = r#"{"command":"increment"}"#.to_string();
        let message = JointMessage::new(
            JointMessageMethod::Action(action_data.clone()),
            "client101".to_string(),
        );
        assert_eq!(message.client_token, "client101");
        if let JointMessageMethod::Action(data) = &message.message {
            assert_eq!(data, &action_data);
        } else {
            panic!("Expected Action message");
        }
    }

    #[test]
    fn test_joint_message_deserialization() {
        let json_str = r#"
        {
            "message": {
                "type": "Create"
            },
            "client_token": "client123"
        }
        "#;
        let message: JointMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(message.client_token, "client123");
        assert!(matches!(message.message, JointMessageMethod::Create));

        let json_str = r#"
        {
            "message": {
                "type": "Join",
                "data": 42
            },
            "client_token": "client456"
        }
        "#;
        let message: JointMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(message.client_token, "client456");
        if let JointMessageMethod::Join(room_id) = message.message {
            assert_eq!(room_id, 42);
        } else {
            panic!("Expected Join message");
        }

        let json_str = r#"
        {
            "message": {
                "type": "Leave"
            },
            "client_token": "client789"
        }
        "#;
        let message: JointMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(message.client_token, "client789");
        assert!(matches!(message.message, JointMessageMethod::Leave));

        let json_str = r#"
        {
            "message": {
                "type": "Action",
                "data": "{\"command\":\"increment\"}"
            },
            "client_token": "client101"
        }
        "#;
        let message: JointMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(message.client_token, "client101");
        if let JointMessageMethod::Action(action_data) = &message.message {
            assert_eq!(action_data, r#"{"command":"increment"}"#);
        } else {
            panic!("Expected Action message");
        }
    }

    #[test]
    fn test_joint_message_clone() {
        let original = JointMessage::new(
            JointMessageMethod::Action("test".to_string()),
            "client".to_string(),
        );
        let cloned = original.clone();

        assert_eq!(original.client_token, cloned.client_token);
        if let JointMessageMethod::Action(original_data) = &original.message {
            if let JointMessageMethod::Action(cloned_data) = &cloned.message {
                assert_eq!(original_data, cloned_data);
            } else {
                panic!("Expected Action message in clone");
            }
        } else {
            panic!("Expected Action message in original");
        }
    }
}
