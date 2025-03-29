#[cfg(test)]
mod mpsc_joint_tests {
    use super::*;
    use crate::client::Client;
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::joint::mpsc::MPSCJoint;
    use crate::message::{JointMessage, JointMessageMethod};
    use crate::response::{ErrorResponse, Response, RoomResponse};
    use crate::utils::types::Broadcastable;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::{sleep, Duration};

    // Test action type for our room-based tests
    #[derive(Debug, Clone, Deserialize, Serialize)]
    enum GameAction {
        UpdateScore { player_id: u64, points: i32 },
        SendMessage { content: String },
        UpdateGameState { state: String },
    }

    impl crate::utils::types::Receivable for GameAction {}

    // Test state type for our room-based tests
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct GameState {
        scores: std::collections::HashMap<u64, i32>,
        messages: Vec<String>,
        current_state: String,
    }

    impl Broadcastable for GameState {}

    // Test reducer
    #[derive(Default)]
    struct GameReducer {
        state: GameState,
    }

    impl Dispatchable for GameReducer {
        type Action = GameAction;
        type Response = GameState;

        async fn dispatch(
            &mut self,
            client_id: u64,
            action: GameAction,
        ) -> Result<ActionResponse<GameState>, String> {
            match action {
                GameAction::UpdateScore { player_id, points } => {
                    let current_score = self.state.scores.get(&player_id).cloned().unwrap_or(0);
                    self.state.scores.insert(player_id, current_score + points);

                    Ok(ActionResponse {
                        state: self.state.clone(),
                        author: client_id,
                        data: format!("Player {} score updated", player_id),
                    })
                }
                GameAction::SendMessage { content } => {
                    self.state.messages.push(content.clone());

                    Ok(ActionResponse {
                        state: self.state.clone(),
                        author: client_id,
                        data: format!("Message sent: {}", content),
                    })
                }
                GameAction::UpdateGameState { state } => {
                    self.state.current_state = state.clone();

                    Ok(ActionResponse {
                        state: self.state.clone(),
                        author: client_id,
                        data: format!("Game state updated to: {}", state),
                    })
                }
            }
        }

        async fn extern_dispatch(
            &mut self,
            client_id: u64,
            action: &str,
        ) -> Result<ActionResponse<GameState>, String> {
            let action: GameAction = serde_json::from_str(action).map_err(|e| e.to_string())?;
            self.dispatch(client_id, action).await
        }
    }

    // Helper functions for creating and handling messages
    fn create_message(method: JointMessageMethod) -> JointMessage {
        JointMessage {
            message: method,
            client_token: "0".to_string(),
        }
    }

    fn create_action_message(action: GameAction) -> JointMessage {
        let action_json = serde_json::to_string(&action).unwrap();
        create_message(JointMessageMethod::Action(action_json))
    }

    fn extract_room_response(message: &RoomResponse) -> Response {
        message.response.clone()
    }

    // Extract game state from an Action response
    fn extract_action_response(response: &Response) -> Option<ActionResponse<GameState>> {
        match &response {
            Response::Action(action_data) => {
                // If it's an action, deserialize the JSON data string to an ActionResponse
                serde_json::from_str(action_data).ok()
            }
            _ => None,
        }
    }

    // Extract error from a Response
    fn extract_error_response(response: &Response) -> Option<String> {
        match response {
            Response::ServerError(msg) => Some(msg.clone()),
            Response::ClientError(msg) => Some(msg.clone()),
            Response::NotFound(msg) => Some(msg.clone()),
            _ => None,
        }
    }

    #[tokio::test]
    async fn test_complete_client_flow() {
        // Create the joint
        let joint = MPSCJoint::<GameReducer>::new(GameReducer::default());

        // STEP 1: CONNECT
        let (tx, mut rx) = joint.connect(10);

        // STEP 2: CREATE ROOM
        tx.send(create_message(JointMessageMethod::Create))
            .await
            .expect("Failed to send create room message");

        // Wait for room creation response
        let response = rx.recv().await.expect("Failed to receive response");

        // STEP 3: PERFORM ACTIONS

        // Update score action
        let action1 = GameAction::UpdateScore {
            player_id: 1,
            points: 10,
        };
        tx.send(create_action_message(action1))
            .await
            .expect("Failed to send update score action");

        // Wait for action response
        let response = rx.recv().await.expect("Failed to receive action response");
        let action_response = extract_action_response(&response);

        // Verify action was processed
        assert!(action_response.is_some());
        let state = action_response.unwrap().state;
        assert_eq!(*state.scores.get(&1).unwrap(), 10);

        // Send message action
        let action2 = GameAction::SendMessage {
            content: "Hello from test!".to_string(),
        };
        tx.send(create_action_message(action2))
            .await
            .expect("Failed to send message action");

        // Wait for action response
        let response = rx.recv().await.expect("Failed to receive action response");
        let action_response = extract_action_response(&response);

        // Verify action was processed
        assert!(action_response.is_some());
        let state = action_response.unwrap().state;
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0], "Hello from test!");

        // Update game state action
        let action3 = GameAction::UpdateGameState {
            state: "RUNNING".to_string(),
        };
        tx.send(create_action_message(action3))
            .await
            .expect("Failed to send update state action");

        // Wait for action response
        let response = rx.recv().await.expect("Failed to receive action response");
        let action_response = extract_action_response(&response);

        // Verify action was processed
        assert!(action_response.is_some());
        let state = action_response.unwrap().state;
        assert_eq!(state.current_state, "RUNNING");

        // STEP 4: LEAVE ROOM
        tx.send(create_message(JointMessageMethod::Leave))
            .await
            .expect("Failed to send leave room message");

        // STEP 5: DISCONNECT (by dropping the channel)
        drop(tx);

        // Give time for the disconnect to propagate
        sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_multiple_clients_room_interaction() {
        // Create the joint
        let joint = MPSCJoint::<GameReducer>::new(GameReducer::default());

        // Connect client 1
        let (tx1, mut rx1) = joint.connect(10);

        // Client 1 creates a room
        tx1.send(create_message(JointMessageMethod::Create))
            .await
            .expect("Failed to send create room message");

        // Get room ID
        let response = rx1.recv().await.expect("Failed to receive create response");
        let room_id = match response {
            Response::RoomCreated(room_response) => room_response,
            _ => panic!("Expected room response"),
        };

        // Connect client 2
        let (tx2, mut rx2) = joint.connect(10);

        // Client 2 joins the room
        tx2.send(create_message(JointMessageMethod::Join(room_id)))
            .await
            .expect("Failed to send join room message");

        // Client 2 gets join confirmation
        let response = rx2
            .recv()
            .await
            .expect("Client 2 failed to receive join response");

        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 1 also receives notification that client 2 joined
        let response = rx1
            .recv()
            .await
            .expect("Client 1 failed to receive join notification");
        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 1 sends a message
        let action = GameAction::SendMessage {
            content: "Hello from client 1!".to_string(),
        };
        tx1.send(create_action_message(action))
            .await
            .expect("Failed to send message action");

        // Both clients should receive the action response

        // Client 1 receives it
        let response1 = rx1
            .recv()
            .await
            .expect("Client 1 failed to receive action response");
        let action_response1 =
            extract_action_response(&response1).expect("Expected action response");

        // Client 2 receives it
        let response2 = rx2
            .recv()
            .await
            .expect("Client 2 failed to receive action response");
        let action_response2 =
            extract_action_response(&response2).expect("Expected action response");

        // Verify both clients received the same state update
        assert_eq!(action_response1.state, action_response2.state);
        assert_eq!(action_response1.state.messages[0], "Hello from client 1!");

        // Client 2 sends a message
        let action = GameAction::SendMessage {
            content: "Hello from client 2!".to_string(),
        };
        tx2.send(create_action_message(action))
            .await
            .expect("Failed to send message action");

        // Both clients should receive the action response

        // Client 1 receives it
        let response1 = rx1
            .recv()
            .await
            .expect("Client 1 failed to receive action response");
        let action_response1 =
            extract_action_response(&response1).expect("Expected action response");

        // Client 2 receives it
        let response2 = rx2
            .recv()
            .await
            .expect("Client 2 failed to receive action response");
        let action_response2 =
            extract_action_response(&response2).expect("Expected action response");

        // Verify both clients received the same state update
        assert_eq!(action_response1.state, action_response2.state);
        assert_eq!(action_response1.state.messages.len(), 2);
        assert_eq!(action_response1.state.messages[1], "Hello from client 2!");

        // Client 1 leaves the room
        tx1.send(create_message(JointMessageMethod::Leave))
            .await
            .expect("Failed to send leave room message");

        // Client 2 receives notification that client 1 left
        let response = rx2
            .recv()
            .await
            .expect("Client 2 failed to receive leave notification");
        match response {
            Response::RoomLeft(_) => (),
            _ => panic!("Expected room leave response"),
        }

        // Client 2 leaves the room
        tx2.send(create_message(JointMessageMethod::Leave))
            .await
            .expect("Failed to send leave room message");
    }

    #[tokio::test]
    async fn test_room_error_handling() {
        // Create the joint
        let joint = MPSCJoint::<GameReducer>::new(GameReducer::default());

        // Connect a client
        let (tx, mut rx) = joint.connect(10);

        // Try to perform an action without joining a room first
        let action = GameAction::UpdateScore {
            player_id: 1,
            points: 10,
        };
        tx.send(create_action_message(action))
            .await
            .expect("Failed to send action without room");

        // Should receive an error response
        let response = rx.recv().await.expect("Failed to receive error response");
        let error = extract_error_response(&response).expect("Expected error response");

        // Verify error message
        assert!(error.contains("Client not in room"));

        // Try to leave a room when not in one
        tx.send(create_message(JointMessageMethod::Leave))
            .await
            .expect("Failed to send leave message without being in room");

        // Should receive an error response
        let response = rx.recv().await.expect("Failed to receive error response");
        let error = extract_error_response(&response).expect("Expected error response");

        // Verify error message
        assert!(error.contains("Client not in room"));

        // Create a room now
        tx.send(create_message(JointMessageMethod::Create))
            .await
            .expect("Failed to send create room message");

        // Should receive success response
        let response = rx
            .recv()
            .await
            .expect("Failed to receive create room response");
        match response {
            Response::RoomCreated(_) => (),
            _ => panic!("Expected room create response"),
        }

        // Try to create another room while in one
        tx.send(create_message(JointMessageMethod::Create))
            .await
            .expect("Failed to send second create room message");

        // Should receive an error response
        let response = rx.recv().await.expect("Failed to receive error response");
        let error = extract_error_response(&response).expect("Expected error response");

        // Verify error message
        assert!(error.contains("Leave current room"));
    }

    #[tokio::test]
    async fn test_room_reconnection() {
        // Create the joint
        let joint = MPSCJoint::<GameReducer>::new(GameReducer::default());

        // Connect first client
        let (tx1, mut rx1) = joint.connect(10);

        // Create a room
        tx1.send(create_message(JointMessageMethod::Create))
            .await
            .expect("Failed to send create room message");

        // Get room ID
        let response = rx1.recv().await.expect("Failed to receive create response");
        let room_id = match response {
            Response::RoomCreated(id) => id,
            _ => panic!("Expected room create response"),
        };

        // Send a message to have some state
        let action = GameAction::UpdateGameState {
            state: "WAITING".to_string(),
        };
        tx1.send(create_action_message(action))
            .await
            .expect("Failed to send update state action");

        // Wait for action confirmation
        let response = rx1.recv().await.expect("Failed to receive action response");
        extract_action_response(&response).expect("Expected action response");

        // Connect a second client
        let (tx2, mut rx2) = joint.connect(10);

        // Second client joins the room
        tx2.send(create_message(JointMessageMethod::Join(room_id)))
            .await
            .expect("Failed to send join room message");

        // Wait for join confirmation
        let response = rx2.recv().await.expect("Failed to receive join response");
        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 1 gets notification about client 2 joining
        let response = rx1
            .recv()
            .await
            .expect("Failed to receive join notification");
        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 2 updates the score
        let action = GameAction::UpdateScore {
            player_id: 2,
            points: 20,
        };
        tx2.send(create_action_message(action))
            .await
            .expect("Failed to send score update action");

        // Both clients receive the update
        let response1 = rx1
            .recv()
            .await
            .expect("Client 1 failed to receive action response");
        let response2 = rx2
            .recv()
            .await
            .expect("Client 2 failed to receive action response");

        let action_response1 =
            extract_action_response(&response1).expect("Expected action response");
        let action_response2 =
            extract_action_response(&response2).expect("Expected action response");

        // Verify both have the same state
        assert_eq!(action_response1.state, action_response2.state);
        assert_eq!(*action_response1.state.scores.get(&2).unwrap(), 20);

        // Client 2 disconnects
        drop(tx2);
        drop(rx2);

        // Wait for disconnect to process
        sleep(Duration::from_millis(100)).await;

        // Connect a new client (client 3)
        let (tx3, mut rx3) = joint.connect(10);

        // Client 3 joins the same room
        tx3.send(create_message(JointMessageMethod::Join(room_id)))
            .await
            .expect("Failed to send join room message");

        // Wait for join confirmation
        let response = rx3.recv().await.expect("Failed to receive join response");
        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 1 gets notification about client 3 joining
        let response = rx1
            .recv()
            .await
            .expect("Failed to receive join notification");
        match response {
            Response::RoomJoined(_) => (),
            _ => panic!("Expected room join response"),
        }

        // Client 3 sends a message
        let action = GameAction::SendMessage {
            content: "I'm back!".to_string(),
        };
        tx3.send(create_action_message(action))
            .await
            .expect("Failed to send message action");

        // Both clients receive the update
        let response1 = rx1
            .recv()
            .await
            .expect("Client 1 failed to receive action response");
        let response3 = rx3
            .recv()
            .await
            .expect("Client 3 failed to receive action response");

        let action_response1 =
            extract_action_response(&response1).expect("Expected action response");
        let action_response3 =
            extract_action_response(&response3).expect("Expected action response");

        // Verify the state includes both previous actions and the new message
        assert_eq!(action_response1.state, action_response3.state);
        assert_eq!(*action_response1.state.scores.get(&2).unwrap(), 20);
        assert_eq!(action_response1.state.current_state, "WAITING");
        assert_eq!(action_response1.state.messages[0], "I'm back!");
    }
}
