#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{get_id, WebStateTrait};
    use serde::{Deserialize, Serialize};

    // Test implementation of WebStateTrait
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        name: String,
    }

    impl WebStateTrait for TestState {}

    #[test]
    fn test_get_id() {
        // Test that IDs are unique and increasing
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
        // Create a test state
        let state = TestState {
            value: 42,
            name: "test".to_string(),
        };

        // Test serialization
        let serialized = serde_json::to_string(&state).unwrap();
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized: TestState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, state);

        // Test cloning
        let cloned = state.clone();
        assert_eq!(cloned, state);
    }

    #[test]
    fn test_broadcastable_and_receivable() {
        use crate::utils::types::{Broadcastable, Receivable};

        // Define test types
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

        // Test Broadcastable
        let broadcastable = TestBroadcastable {
            message: "Hello".to_string(),
        };
        let serialized = serde_json::to_string(&broadcastable).unwrap();
        assert!(!serialized.is_empty());

        // Test Receivable
        let receivable_json = r#"{"command":"Start"}"#;
        let receivable: TestReceivable = serde_json::from_str(receivable_json).unwrap();
        assert_eq!(receivable.command, "Start");
    }
}
