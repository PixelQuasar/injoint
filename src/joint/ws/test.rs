#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::joint::ws::WebsocketJoint;
    use crate::room::{Room, RoomStatus};
    use crate::utils::types::{Broadcastable, Receivable};
    use futures_util::{SinkExt, StreamExt};
    use serde::de::Unexpected::Str;
    use serde::{Deserialize, Serialize};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::{future::Future, net::SocketAddr};
    use tokio::io::join;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tokio_tungstenite::tungstenite::Message;

    // Simple test action and state for testing
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestAction {
        Increment,
        Add(i32),
    }

    impl Receivable for TestAction {}

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestState {
        counter: i32,
    }

    impl Broadcastable for TestState {}

    #[derive(Clone, Default)]
    struct TestReducer {
        state: TestState,
    }

    impl Dispatchable for TestReducer {
        type Action = TestAction;
        type State = TestState;

        fn dispatch(
            &mut self,
            client_id: u64,
            action: Self::Action,
        ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send {
            async move {
                match action {
                    TestAction::Increment => {
                        self.state.counter += 1;
                    }
                    TestAction::Add(value) => {
                        self.state.counter += value;
                    }
                }

                Ok(ActionResponse {
                    status: "success".to_string(),
                    state: self.state.clone(),
                    author: client_id,
                    data: "".to_string(),
                })
            }
        }

        fn extern_dispatch(
            &mut self,
            client_id: u64,
            action: &str,
        ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send {
            async move {
                if action == "create_room" {
                    return Ok(ActionResponse {
                        status: "success".to_string(),
                        state: self.state.clone(),
                        author: client_id,
                        data: "".to_string(),
                    });
                }

                let action: TestAction = serde_json::from_str(action)
                    .map_err(|e| format!("Failed to deserialize action: {}", e))?;
                self.dispatch(client_id, action).await
            }
        }

        fn get_state(&self) -> Self::State {
            self.state.clone()
        }
    }

    // Helper function to create a test listener on an available port
    async fn setup_test_listener() -> TcpListener {
        TcpListener::bind("127.0.0.1:0").await.unwrap()
    }

    #[tokio::test]
    async fn test_websocket_joint_creation() {
        // Create a joint
        let reducer = TestReducer::default();
        let joint = WebsocketJoint::new(reducer);

        // Verify it was created correctly
        assert!(joint.tcp_listener.is_none());
    }

    #[tokio::test]
    async fn test_websocket_joint_bind() {
        // Create a joint
        let reducer = TestReducer::default();
        let mut joint = WebsocketJoint::new(reducer);

        // Create a listener
        let listener = setup_test_listener().await;
        let addr = listener.local_addr().unwrap();

        // Bind the joint to the listener
        joint.bind_listener(listener).await;

        // Verify it was bound correctly
        assert!(joint.tcp_listener.is_some());
    }

    #[tokio::test]
    async fn test_websocket_joint_dispatch() {
        // Create a joint
        let reducer = TestReducer::default();
        let joint = WebsocketJoint::new(reducer);

        let rooms = joint.joint.broadcaster.get_rooms().clone();
        let mut rooms = rooms.lock().await;
        rooms.insert(
            1,
            Room::new(
                1,
                0,
                HashSet::new(),
                RoomStatus::Public,
                Arc::new(Mutex::new(TestReducer::default())),
            ),
        );
        drop(rooms);

        // Test the dispatch method
        let clients = joint.joint.broadcaster.get_clients().clone();
        let mut clients = clients.lock().await;
        clients.insert(1, Client::new(1, Some(1), String::new(), String::new()));
        drop(clients);

        let client_id = 1;
        let action = r#"{"Increment":null}"#;
        let result = joint.dispatch(client_id, action).await;

        // Verify the result
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status, "success");
        assert_eq!(response.author, client_id);
        assert_eq!(response.state.counter, 1);

        // Test adding a value
        let action = r#"{"Add":5}"#;
        let result = joint.dispatch(client_id, action).await;

        // Verify the result
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.state.counter, 6);
    }
}
