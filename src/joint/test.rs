#[cfg(test)]
mod abstract_joint_tests {
    use super::*;
    use crate::broadcaster::Broadcaster;
    use crate::client::Client;
    use crate::connection::{SinkAdapter, StreamAdapter};
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::joint::AbstractJoint;
    use crate::message::{JointMessage, JointMessageMethod};
    use crate::response::Response;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::sync::Mutex;
    use tokio::time::Duration;

    // Mock implementation for SinkAdapter
    struct MockSink {
        responses: Arc<StdMutex<Vec<Response>>>,
    }

    #[async_trait]
    impl SinkAdapter for MockSink {
        async fn send(
            &mut self,
            response: Response,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.responses.lock().unwrap().push(response);
            Ok(())
        }
    }

    impl Unpin for MockSink {}

    // Mock implementation for StreamAdapter
    struct MockStream {
        messages: Vec<JointMessage>,
        index: usize,
    }

    #[async_trait]
    impl StreamAdapter for MockStream {
        async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>> {
            if self.index < self.messages.len() {
                let message = self.messages[self.index].clone();
                self.index += 1;
                Ok(message)
            } else {
                // Return an error to stop the processing loop
                Err("No more messages".into())
            }
        }
    }

    impl Unpin for MockStream {}

    // Test action and state
    #[derive(Debug, Clone, Deserialize, Serialize)]
    enum TestAction {
        Add(i32),
        Echo(String),
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestState {
        value: i32,
        messages: Vec<String>,
    }

    impl crate::utils::types::Broadcastable for TestState {}
    impl crate::utils::types::Receivable for TestAction {}

    // Test reducer
    #[derive(Default)]
    struct TestReducer {
        state: TestState,
    }

    impl Dispatchable for TestReducer {
        type Action = TestAction;
        type Response = TestState;

        async fn dispatch(
            &mut self,
            client_id: u64,
            action: TestAction,
        ) -> Result<ActionResponse<TestState>, String> {
            match action {
                TestAction::Add(value) => {
                    self.state.value += value;
                    Ok(ActionResponse {
                        state: self.state.clone(),
                        author: client_id,
                        data: format!("Added {}", value),
                    })
                }
                TestAction::Echo(message) => {
                    self.state.messages.push(message.clone());
                    Ok(ActionResponse {
                        state: self.state.clone(),
                        author: client_id,
                        data: message,
                    })
                }
            }
        }
    }

    #[tokio::test]
    async fn test_joint_initialization() {
        // Simply test that the joint can be created without errors
        let joint = AbstractJoint::<TestReducer, MockSink>::new();

        // Verify the reducer is initialized with default state
        let reducer = joint.reducer.lock().await;
        assert_eq!(reducer.state.value, 0);
        assert_eq!(reducer.state.messages.len(), 0);
    }

    #[tokio::test]
    async fn test_joint_dispatch() {
        // Create a joint
        let joint = AbstractJoint::<TestReducer, MockSink>::new();

        // Dispatch an Add action
        let result = joint.dispatch(1, TestAction::Add(42)).await;

        // Verify the action was processed successfully
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.state.value, 42);
        assert_eq!(response.author, 1);
        assert_eq!(response.data, "Added 42");

        // Dispatch an Echo action
        let result = joint
            .dispatch(2, TestAction::Echo("Hello Joint".to_string()))
            .await;

        // Verify the action was processed successfully
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.state.value, 42); // Value should remain
        assert_eq!(response.state.messages.len(), 1);
        assert_eq!(response.state.messages[0], "Hello Joint");
        assert_eq!(response.author, 2);
        assert_eq!(response.data, "Hello Joint");
    }

    #[tokio::test]
    async fn test_handle_stream() {
        // This test will focus on the client lifecycle management
        // Create a joint
        let joint = Arc::new(AbstractJoint::<TestReducer, MockSink>::new());

        // Create a mock stream with room creation and action messages
        let create_message = JointMessage {
            client_token: "0".to_string(),
            message: JointMessageMethod::Create,
        };

        let action = TestAction::Add(10);
        let action_json = serde_json::to_string(&action).unwrap();

        let action_message = JointMessage {
            client_token: "0".to_string(),
            message: JointMessageMethod::Action(action_json),
        };

        let messages = vec![create_message, action_message];
        let mut stream = MockStream { messages, index: 0 };

        // Create a response collector
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        // Handle the stream (this runs until stream.next() returns an error)
        let joint_clone = joint.clone();
        let handle = tokio::spawn(async move {
            joint_clone.handle_stream(&mut stream, sink).await;
        });

        // Allow time for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Here we would verify the responses, but they'll vary based on random client ID
        // Instead verify that the stream was processed by checking response count
        let response_count = responses.lock().unwrap().len();
        assert!(response_count > 0, "No responses received");

        // Verify the handle completed (meaning stream was fully processed)
        let _ = tokio::time::timeout(Duration::from_millis(500), handle)
            .await
            .expect("handle_stream did not complete in time");
    }

    #[tokio::test]
    async fn test_client_lifecycle() {
        // This test verifies that clients are added and removed properly
        let joint = AbstractJoint::<TestReducer, MockSink>::new();

        // Create an empty stream that will immediately return an error
        let mut stream = MockStream {
            messages: vec![],
            index: 0,
        };
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        // Get initial client count
        let initial_client_count = joint.broadcaster.get_clients().lock().await.len();

        // Run handle_stream which should add a client and then remove it
        joint.handle_stream(&mut stream, sink).await;

        // Verify client count is back to initial
        let final_client_count = joint.broadcaster.get_clients().lock().await.len();
        assert_eq!(
            final_client_count, initial_client_count,
            "Client was not properly removed"
        );
    }

    #[tokio::test]
    async fn test_integration_with_broadcaster() {
        // This test verifies the integration between the Joint and Broadcaster
        let joint = AbstractJoint::<TestReducer, MockSink>::new();

        // Get a reference to the broadcaster
        let broadcaster = &joint.broadcaster;

        // Verify joint initialized the broadcaster
        assert_eq!(broadcaster.get_clients().lock().await.len(), 0);
        assert_eq!(broadcaster.get_rooms().lock().await.len(), 0);

        // Create a client
        let client_id = 1;
        let client = Client::new(client_id, None, "Test User".to_string(), "".to_string());
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        // Add client to broadcaster
        broadcaster.add_client_connection(client, sink).await;

        // Verify client was added
        assert_eq!(broadcaster.get_clients().lock().await.len(), 1);

        // Dispatch action through the joint to verify the integration
        let result = joint.dispatch(client_id, TestAction::Add(5)).await;
        assert!(result.is_ok());

        // Clean up
        broadcaster.remove_client_connection(client_id).await;
    }
}
