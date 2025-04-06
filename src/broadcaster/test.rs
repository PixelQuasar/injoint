#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcaster::Broadcaster;
    use crate::client::Client;
    use crate::connection::{SinkAdapter, StreamAdapter};
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::message::{JointMessage, JointMessageMethod};
    use crate::response::{ClientResponse, Response, RoomResponse};
    use crate::room::{Room, RoomStatus};
    use crate::utils::types::{Broadcastable, Receivable};
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::sync::Mutex;

    // Mock SinkAdapter for testing
    #[derive(Clone)]
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

    // Mock StreamAdapter for testing
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
                Err("End of stream".into())
            }
        }
    }

    impl Unpin for MockStream {}

    // Test action and state
    #[derive(Debug, Clone, Deserialize, Serialize)]
    enum TestAction {
        Increment,
        Add(i32),
        Message(String),
    }

    impl Receivable for TestAction {}

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestState {
        counter: i32,
        messages: Vec<String>,
    }

    impl Broadcastable for TestState {}

    // Test reducer implementing Dispatchable
    #[derive(Clone, Default)]
    struct TestReducer {
        state: TestState,
    }

    impl Dispatchable for TestReducer {
        type Action = TestAction;
        type State = TestState;

        async fn dispatch(
            &mut self,
            client_id: u64,
            action: TestAction,
        ) -> Result<ActionResponse<TestState>, String> {
            match action {
                TestAction::Increment => {
                    self.state.counter += 1;
                    Ok(ActionResponse {
                        status: "success".into(),
                        state: self.state.clone(),
                        author: client_id,
                        data: self.state.counter.to_string(),
                    })
                }
                TestAction::Add(value) => {
                    self.state.counter += value;
                    Ok(ActionResponse {
                        status: "success".into(),
                        state: self.state.clone(),
                        author: client_id,
                        data: format!("Added {}", value),
                    })
                }
                TestAction::Message(text) => {
                    self.state.messages.push(text.clone());
                    Ok(ActionResponse {
                        status: "success".into(),
                        state: self.state.clone(),
                        author: client_id,
                        data: text,
                    })
                }
            }
        }

        async fn extern_dispatch(
            &mut self,
            client_id: u64,
            action_str: &str,
        ) -> Result<ActionResponse<TestState>, String> {
            let action: TestAction = serde_json::from_str(action_str)
                .map_err(|e| format!("Failed to parse action: {}", e))?;
            self.dispatch(client_id, action).await
        }

        fn get_state(&self) -> TestState {
            self.state.clone()
        }
    }

    // Helper functions
    fn create_client(id: u64) -> Client {
        Client::new(id, None, format!("User{}", id), String::new())
    }

    fn create_message(client_id: u64, method: JointMessageMethod) -> JointMessage {
        JointMessage {
            client_token: client_id.to_string(),
            message: method,
        }
    }

    fn create_action_message(client_id: u64, action: TestAction) -> JointMessage {
        let action_json = serde_json::to_string(&action).unwrap();
        create_message(client_id, JointMessageMethod::Action(action_json))
    }

    fn get_response_count(responses: &Arc<StdMutex<Vec<Response>>>) -> usize {
        responses.lock().unwrap().len()
    }

    fn get_last_response(responses: &Arc<StdMutex<Vec<Response>>>) -> Option<Response> {
        let responses = responses.lock().unwrap();
        responses.last().cloned()
    }

    // TESTS

    #[tokio::test]
    async fn test_broadcaster_creation() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Verify initial state
        assert_eq!(broadcaster.get_clients().clone().lock().await.len(), 0);
        assert_eq!(broadcaster.get_rooms().clone().lock().await.len(), 0);
        assert_eq!(broadcaster.get_connections().clone().lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_add_remove_client() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        // Add client
        broadcaster.add_client_connection(client, sink).await;

        // Verify client was added
        {
            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.len(), 1);
            assert!(clients.contains_key(&1));

            let connections = broadcaster.get_connections();
            let connections = connections.lock().await;
            assert_eq!(connections.len(), 1);
            assert!(connections.contains_key(&1));
        }

        // Remove client
        broadcaster.remove_client_connection(1).await;

        // Verify client was removed
        {
            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.len(), 0);

            let connections = broadcaster.get_connections();
            let connections = connections.lock().await;
            assert_eq!(connections.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_handle_create() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        // Add client
        broadcaster.add_client_connection(client, sink).await;

        // Create room
        let result = broadcaster.handle_create(1).await;

        // Verify success
        assert!(result.is_ok());
        let room_response = result.unwrap();
        assert!(matches!(room_response.response, Response::RoomCreated(_)));

        // Verify room was created with reducer
        {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            assert_eq!(rooms.len(), 1);
            let room = rooms.get(&0).unwrap();
            assert_eq!(room.id, 0);
            assert_eq!(room.owner_id, 1);
            assert!(room.client_ids.contains(&1));

            // Verify client is in room
            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            let client = clients.get(&1).unwrap();
            assert_eq!(client.room_id, Some(0));
        }
    }

    #[tokio::test]
    async fn test_handle_join() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add two clients
        let client1 = create_client(1);
        let responses1 = Arc::new(StdMutex::new(Vec::new()));
        let sink1 = MockSink {
            responses: responses1.clone(),
        };

        let client2 = create_client(2);
        let responses2 = Arc::new(StdMutex::new(Vec::new()));
        let sink2 = MockSink {
            responses: responses2.clone(),
        };

        broadcaster.add_client_connection(client1, sink1).await;
        broadcaster.add_client_connection(client2, sink2).await;

        // Client 1 creates room
        let create_result = broadcaster.handle_create(1).await;
        assert!(create_result.is_ok());
        let room_id = match create_result.unwrap().response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected RoomCreated response"),
        };

        // Clear response buffer
        responses1.lock().unwrap().clear();

        // Client 2 joins room
        let join_result = broadcaster.handle_join(2, room_id).await;

        // Verify success
        assert!(join_result.is_ok());
        let room_response = join_result.unwrap();
        assert!(matches!(room_response.response, Response::RoomJoined(_)));

        // Verify both clients are in room
        {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            let room = rooms.get(&room_id).unwrap();
            assert_eq!(room.client_ids.len(), 2);
            assert!(room.client_ids.contains(&1));
            assert!(room.client_ids.contains(&2));

            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.get(&1).unwrap().room_id, Some(room_id));
            assert_eq!(clients.get(&2).unwrap().room_id, Some(room_id));
        }
    }

    #[tokio::test]
    async fn test_handle_action() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add client
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        broadcaster.add_client_connection(client, sink).await;

        // Create room
        let create_result = broadcaster.handle_create(1).await;
        assert!(create_result.is_ok());
        let room_id = match create_result.unwrap().response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected RoomCreated response"),
        };

        // Clear response buffer
        responses.lock().unwrap().clear();

        // Get room reducer
        let room_reducer = {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            rooms.get(&room_id).unwrap().reducer.clone()
        };

        // Perform action
        let action = TestAction::Add(5);
        let action_result = broadcaster
            .handle_action(1, action, room_reducer.clone())
            .await;

        // Verify success
        assert!(action_result.is_ok());
        let room_response = action_result.unwrap();
        assert!(matches!(room_response.response, Response::Action(_)));

        // Verify action changed state
        {
            let reducer = room_reducer.lock().await;
            assert_eq!(reducer.get_state().counter, 5);
        }

        // Another action
        let action = TestAction::Message("Hello".to_string());
        let action_result = broadcaster
            .handle_action(1, action, room_reducer.clone())
            .await;
        assert!(action_result.is_ok());

        // Verify state
        {
            let reducer = room_reducer.lock().await;
            let state = reducer.get_state();
            assert_eq!(state.counter, 5);
            assert_eq!(state.messages, vec!["Hello"]);
        }
    }

    #[tokio::test]
    async fn test_handle_leave() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add two clients
        let client1 = create_client(1);
        let responses1 = Arc::new(StdMutex::new(Vec::new()));
        let sink1 = MockSink {
            responses: responses1.clone(),
        };

        let client2 = create_client(2);
        let responses2 = Arc::new(StdMutex::new(Vec::new()));
        let sink2 = MockSink {
            responses: responses2.clone(),
        };

        broadcaster.add_client_connection(client1, sink1).await;
        broadcaster.add_client_connection(client2, sink2).await;

        // Client 1 creates room
        let create_result = broadcaster.handle_create(1).await;
        assert!(create_result.is_ok());
        let room_id = match create_result.unwrap().response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected RoomCreated response"),
        };

        // Client 2 joins room
        let join_result = broadcaster.handle_join(2, room_id).await;
        assert!(join_result.is_ok());

        // Clear response buffers
        responses1.lock().unwrap().clear();
        responses2.lock().unwrap().clear();

        // Client 1 leaves room
        let leave_result = broadcaster.handle_leave(1).await;

        // Verify success
        assert!(leave_result.is_ok());
        let room_response = leave_result.unwrap();
        assert!(matches!(room_response.response, Response::RoomLeft(_)));

        // Verify client 1 left room but client 2 remains
        {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            let room = rooms.get(&room_id).unwrap();
            assert_eq!(room.client_ids.len(), 1);
            assert!(!room.client_ids.contains(&1));
            assert!(room.client_ids.contains(&2));

            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.get(&1).unwrap().room_id, None);
            assert_eq!(clients.get(&2).unwrap().room_id, Some(room_id));
        }
    }

    #[tokio::test]
    async fn test_process_event() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add client
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        broadcaster.add_client_connection(client, sink).await;

        // Process create room event
        let create_event = create_message(1, JointMessageMethod::Create);
        let result = broadcaster.process_event(1, create_event).await;

        // Verify success
        assert!(result.is_ok());
        let room_id = match result.unwrap().response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected RoomCreated response"),
        };

        let action = TestAction::Add(10);
        let action_event = create_action_message(1, action);

        let result = broadcaster.process_event(1, action_event).await;

        assert!(result.is_ok());

        // Verify state changed
        {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            let room = rooms.get(&room_id).unwrap();
            let state = room.reducer.lock().await.get_state();
            assert_eq!(state.counter, 10);
        }

        // Process leave event
        let leave_event = create_message(1, JointMessageMethod::Leave);
        let result = broadcaster.process_event(1, leave_event).await;

        // Verify success
        assert!(result.is_ok());

        // Verify client left room
        {
            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.get(&1).unwrap().room_id, None);
        }
    }

    #[tokio::test]
    async fn test_extern_dispatch() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add client
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        broadcaster.add_client_connection(client, sink).await;

        // Create room
        let create_result = broadcaster.handle_create(1).await;
        assert!(create_result.is_ok());

        // Dispatch action using extern_dispatch
        let action_json = r#"{"Increment":null}"#;
        let result = broadcaster.extern_dispatch(1, action_json).await;

        // Verify success
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.state.counter, 1);

        // Another action
        let action_json = r#"{"Add":5}"#;
        let result = broadcaster.extern_dispatch(1, action_json).await;

        // Verify success
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.state.counter, 6);
    }

    #[tokio::test]
    async fn test_insert_client_to_room() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add two clients
        let client1 = create_client(1);
        let responses1 = Arc::new(StdMutex::new(Vec::new()));
        let sink1 = MockSink {
            responses: responses1.clone(),
        };

        let client2 = create_client(2);
        let responses2 = Arc::new(StdMutex::new(Vec::new()));
        let sink2 = MockSink {
            responses: responses2.clone(),
        };

        broadcaster.add_client_connection(client1, sink1).await;
        broadcaster.add_client_connection(client2, sink2).await;

        // Client 1 creates room
        let create_result = broadcaster.handle_create(1).await;
        assert!(create_result.is_ok());
        let room_id = match create_result.unwrap().response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected RoomCreated response"),
        };

        // Modify room state
        {
            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            let room = rooms.get(&room_id).unwrap();
            let mut reducer = room.reducer.lock().await;
            reducer.state.counter = 42;
            reducer.state.messages.push("Initial".to_string());
        }

        // Clear responses
        responses2.lock().unwrap().clear();

        // Directly insert client 2 to room
        let result = broadcaster.insert_client_to_room(2, room_id).await;

        // Verify success
        assert!(result.is_ok());

        // Verify client 2 is in room
        {
            let clients = broadcaster.get_clients();
            let clients = clients.lock().await;
            assert_eq!(clients.get(&2).unwrap().room_id, Some(room_id));

            let rooms = broadcaster.get_rooms();
            let rooms = rooms.lock().await;
            let room = rooms.get(&room_id).unwrap();
            assert!(room.client_ids.contains(&2));
        }

        // Verify client 2 received state update
        assert!(get_response_count(&responses2) > 0);
        if let Some(Response::StateSent(state_json)) = get_last_response(&responses2) {
            let state: TestState = serde_json::from_str(&state_json).unwrap();
            assert_eq!(state.counter, 42);
            assert_eq!(state.messages, vec!["Initial"]);
        } else {
            panic!("Expected StateSent response");
        }
    }

    #[tokio::test]
    async fn test_handle_rx() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add client
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        broadcaster.add_client_connection(client, sink).await;

        // Create a stream with multiple messages
        let messages = vec![
            create_message(1, JointMessageMethod::Create),
            create_action_message(1, TestAction::Add(7)),
            create_action_message(1, TestAction::Message("Test".to_string())),
            create_message(1, JointMessageMethod::Leave),
        ];

        let mut stream = MockStream { messages, index: 0 };

        // Handle the stream
        let handle = tokio::spawn(async move {
            broadcaster.handle_rx(1, &mut stream).await;
        });

        // Wait for completion (stream runs out of messages)
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), handle)
            .await
            .expect("handle_rx did not complete");

        // Verify final state of responses
        assert!(get_response_count(&responses) > 0);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let reducer = TestReducer::default();
        let broadcaster = Broadcaster::<MockSink, TestReducer>::new(reducer);

        // Add client
        let client = create_client(1);
        let responses = Arc::new(StdMutex::new(Vec::new()));
        let sink = MockSink {
            responses: responses.clone(),
        };

        broadcaster.add_client_connection(client, sink).await;

        // Try to join non-existent room
        let join_event = create_message(1, JointMessageMethod::Join(999));
        let result = broadcaster.process_event(1, join_event).await;

        // Verify error
        assert!(result.is_err());
        match result.err().unwrap().response {
            Response::NotFound(_) => {}
            _ => panic!("Expected NotFound response"),
        }

        // Try invalid action format
        let invalid_action = r#"{"Invalid":null}"#;
        let result = broadcaster.extern_dispatch(1, invalid_action).await;

        // Verify error
        assert!(result.is_err());

        // Try to perform action without being in room
        let action = TestAction::Add(5);
        let action_event = create_action_message(1, action);
        let result = broadcaster.process_event(1, action_event).await;

        // Verify error
        assert!(result.is_err());
        match result.err().unwrap().response {
            Response::NotFound(_) => {}
            _ => panic!("Expected NotFound response"),
        }
    }
}
