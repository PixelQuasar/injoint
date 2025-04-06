#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::joint::axum::AxumWSJoint;
    use crate::room::{Room, RoomStatus};
    use crate::utils::types::{Broadcastable, Receivable};
    use axum::{
        body::Body,
        extract::ws::{Message, WebSocket},
        http::{Request, StatusCode},
        Router,
    };
    use futures_util::{SinkExt, StreamExt};
    use serde::{Deserialize, Serialize};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::{future::Future, net::SocketAddr};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

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

    // Helper function to create a test router with WebSocket endpoint
    async fn setup_test_router() -> (Router, SocketAddr) {
        let joint = AxumWSJoint::new(TestReducer::default());
        let router = joint.attach_router("/ws", Router::new());

        // Find available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        (router, addr)
    }

    #[tokio::test]
    async fn test_websocket_upgrade() {
        let (app, _addr) = setup_test_router().await;

        // Create a WebSocket upgrade request
        let request = Request::builder()
            .uri("/ws")
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();

        // Call the application with our request
        let response = app.oneshot(request).await.unwrap();

        // Check that we got a successful upgrade response
        assert_eq!(response.status(), 426);
    }

    #[tokio::test]
    async fn test_joint_creation_and_dispatch() {
        // Create a joint
        let reducer = TestReducer::default();
        let joint = AxumWSJoint::new(reducer);

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

        // Test the dispatch method
        let client_id = 1;
        let action = r#"{"Increment":null}"#;
        let result = joint.dispatch(client_id, action).await;

        // Verify the result
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status, "success");
        assert_eq!(response.author, client_id);
        assert_eq!(response.state.counter, 1);
    }
}
