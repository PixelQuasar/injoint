#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::get_id;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        name: String,
    }

    #[test]
    fn test_get_id() {
        let id1 = get_id();
        let id2 = get_id();
        let id3 = get_id();

        assert!(id1 < id2);
        assert!(id2 < id3);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_web_state_trait_implementation() {
        let state = TestState {
            value: 42,
            name: "test".to_string(),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        assert!(!serialized.is_empty());

        let deserialized: TestState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, state);

        let cloned = state.clone();
        assert_eq!(cloned, state);
    }

    #[test]
    fn test_broadcastable_and_receivable() {
        use crate::utils::types::{Broadcastable, Receivable};

        #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
        struct TestBroadcastable {
            message: String,
        }

        impl Broadcastable for TestBroadcastable {}

        #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
        struct TestReceivable {
            command: String,
        }

        impl Receivable for TestReceivable {}

        let broadcastable = TestBroadcastable {
            message: "Hello".to_string(),
        };
        let serialized = serde_json::to_string(&broadcastable).unwrap();
        assert!(!serialized.is_empty());

        let receivable_json = r#"{"command":"Start"}"#;
        let receivable: TestReceivable = serde_json::from_str(receivable_json).unwrap();
        assert_eq!(receivable.command, "Start");
    }
}
