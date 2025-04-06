#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::joint::mpsc::MPSCJoint;
    use crate::message::{JointMessage, JointMessageMethod};
    use crate::response::Response;
    use crate::utils::types::{Broadcastable, Receivable};
    use serde::{Deserialize, Serialize};
    use std::sync::mpsc::Receiver;
    use std::time::Duration;
    use tokio::time::sleep;

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

    fn create_message(method: JointMessageMethod) -> JointMessage {
        JointMessage {
            client_token: "test-token".to_string(),
            message: method,
        }
    }

    fn create_action_message(action: TestAction) -> JointMessage {
        let action_json = serde_json::to_string(&action).unwrap();
        create_message(JointMessageMethod::Action(action_json))
    }


    #[tokio::test]
    async fn test_basic_connection() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());
        let (tx, mut rx) = joint.connect(10);

        assert!(tx.capacity() >= 10);

        drop(tx);
        drop(rx);
    }

    #[tokio::test]
    async fn test_create_room_flow() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());
        let (tx, mut rx) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut room_id: Option<u64> = None;
        while let Some(response) = rx.recv().await {
            match response {
                Response::RoomCreated(id) => {
                    room_id = Some(id);
                    break;
                }
                Response::StateSent(_) => {
                }
                other => {
                    panic!("Unexpected response: {:?}", other);
                }
            }
        }

        assert!(room_id.is_some(), "Room ID should be received");

        drop(tx);
        drop(rx);
    }

    #[tokio::test]
    async fn test_complete_client_flow() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());
        let (tx, mut rx) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut room_id: Option<u64> = None;
        let mut initial_state: Option<TestState> = None;

        while room_id.is_none() || initial_state.is_none() {
            if let Some(response) = rx.recv().await {
                match response {
                    Response::RoomCreated(id) => {
                        room_id = Some(id);
                    }
                    Response::StateSent(state_json) => {
                        let state: TestState =
                            serde_json::from_str(&state_json).expect("Failed to parse state JSON");
                        initial_state = Some(state);
                    }
                    other => {
                        println!("Received other response: {:?}", other);
                    }
                }
            }
        }

        let state = initial_state.unwrap();
        assert_eq!(state.counter, 0);
        assert_eq!(state.messages.len(), 0);

        let action_msg = create_action_message(TestAction::Add(5));
        tx.send(action_msg)
            .await
            .expect("Failed to send action message");

        let mut updated_state: Option<TestState> = None;
        while updated_state.is_none() {
            if let Some(response) = rx.recv().await {
                match response {
                    Response::Action(action_json) => {
                        let action_response: ActionResponse<TestState> =
                            serde_json::from_str(&action_json)
                                .expect("Failed to parse action response");
                        updated_state = Some(action_response.state);
                    }
                    other => {
                        println!("Received other response: {:?}", other);
                    }
                }
            }
        }

        let state = updated_state.unwrap();
        assert_eq!(state.counter, 5);

        let action_msg = create_action_message(TestAction::Message("Hello MPSC".to_string()));
        tx.send(action_msg)
            .await
            .expect("Failed to send message action");

        let mut updated_state2: Option<TestState> = None;
        while updated_state2.is_none() {
            if let Some(response) = rx.recv().await {
                match response {
                    Response::Action(action_json) => {
                        let action_response: ActionResponse<TestState> =
                            serde_json::from_str(&action_json)
                                .expect("Failed to parse action response");
                        updated_state2 = Some(action_response.state);
                    }
                    other => {
                        println!("Received other response: {:?}", other);
                    }
                }
            }
        }

        let state = updated_state2.unwrap();
        assert_eq!(state.counter, 5);
        assert_eq!(state.messages, vec!["Hello MPSC"]);

        let leave_msg = create_message(JointMessageMethod::Leave);
        tx.send(leave_msg)
            .await
            .expect("Failed to send leave message");

        drop(tx);
        drop(rx);
    }

    #[tokio::test]
    async fn test_join_existing_room() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());

        let (tx1, mut rx1) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx1.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut room_id: Option<u64> = None;
        while room_id.is_none() {
            if let Some(response) = rx1.recv().await {
                if let Response::RoomCreated(id) = response {
                    room_id = Some(id);
                }
            }
        }

        let action_msg = create_action_message(TestAction::Add(10));
        tx1.send(action_msg)
            .await
            .expect("Failed to send action message");

        let mut action_processed = false;
        while !action_processed {
            if let Some(response) = rx1.recv().await {
                if let Response::Action(_) = response {
                    action_processed = true;
                }
            }
        }

        let (tx2, mut rx2) = joint.connect(10);

        let join_msg = create_message(JointMessageMethod::Join(room_id.unwrap()));
        tx2.send(join_msg)
            .await
            .expect("Failed to send join message");

        let mut join_confirmed = false;
        let mut received_state: Option<TestState> = None;

        while !join_confirmed || received_state.is_none() {
            if let Some(response) = rx2.recv().await {
                match response {
                    Response::RoomJoined(_) => {
                        join_confirmed = true;
                    }
                    Response::StateSent(state_json) => {
                        let state: TestState =
                            serde_json::from_str(&state_json).expect("Failed to parse state JSON");
                        received_state = Some(state);
                    }
                    other => {
                        println!("Received other response: {:?}", other);
                    }
                }
            }
        }

        let state = received_state.unwrap();
        assert_eq!(state.counter, 10);

        drop(tx1);
        drop(rx1);
        drop(tx2);
        drop(rx2);
    }

    #[tokio::test]
    async fn test_multiple_clients_interaction() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());

        let (tx1, mut rx1) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx1.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut room_id: Option<u64> = None;
        while room_id.is_none() {
            if let Some(response) = rx1.recv().await {
                if let Response::RoomCreated(id) = response {
                    room_id = Some(id);
                }
            }
        }

        let (tx2, mut rx2) = joint.connect(10);

        let join_msg = create_message(JointMessageMethod::Join(room_id.unwrap()));
        tx2.send(join_msg)
            .await
            .expect("Failed to send join message");

        let mut join_confirmed = false;
        while !join_confirmed {
            if let Some(response) = rx2.recv().await {
                if let Response::RoomJoined(_) = response {
                    join_confirmed = true;
                }
            }
        }

        let mut join_notified = false;
        while !join_notified {
            if let Some(response) = rx1.recv().await {
                if let Response::RoomJoined(_) = response {
                    join_notified = true;
                }
            }
        }

        let action_msg = create_action_message(TestAction::Add(7));
        tx1.send(action_msg)
            .await
            .expect("Failed to send action message");

        let mut client1_updated = false;
        let mut client2_updated = false;
        let mut state1: Option<TestState> = None;
        let mut state2: Option<TestState> = None;

        async fn process_action_response(
            rx: &mut tokio::sync::mpsc::Receiver<Response>,
            updated: &mut bool,
            state: &mut Option<TestState>,
        ) {
            while !*updated {
                if let Some(response) = rx.recv().await {
                    if let Response::Action(action_json) = response {
                        let action_response: ActionResponse<TestState> =
                            serde_json::from_str(&action_json)
                                .expect("Failed to parse action response");
                        *state = Some(action_response.state.clone());
                        *updated = true;
                    }
                } else {
                    break;
                }
            }
        }

        let timeout = sleep(Duration::from_millis(500));
        tokio::pin!(timeout);

        process_action_response(&mut rx1, &mut client1_updated, &mut state1).await;
        process_action_response(&mut rx2, &mut client2_updated, &mut state2).await;

        assert!(client1_updated, "Client 1 should receive action update");
        assert!(client2_updated, "Client 2 should receive action update");

        assert_eq!(state1, state2);
        assert_eq!(state1.unwrap().counter, 7);

        drop(tx1);
        drop(rx1);
        drop(tx2);
        drop(rx2);
    }

    #[tokio::test]
    async fn test_direct_dispatch() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());
        let (tx, mut rx) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut client_id: Option<u64> = None;

        while client_id.is_none() {
            if let Some(response) = rx.recv().await {
                match response {
                    Response::RoomCreated(_) => {
                        let clients = joint.joint.broadcaster.get_clients().clone();
                        client_id = Some(*clients.lock().await.iter().next().unwrap().0);
                    }
                    _ => {}
                }
            }
        }

        let action_json = r#"{"Add":15}"#;
        let result = joint.dispatch(client_id.unwrap(), action_json).await;

        assert!(result.is_ok(), "Dispatch should succeed");
        let response = result.unwrap();
        assert_eq!(response.state.counter, 15);

        drop(tx);
        drop(rx);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());
        let (tx, mut rx) = joint.connect(10);

        let action_msg = create_action_message(TestAction::Add(5));
        tx.send(action_msg)
            .await
            .expect("Failed to send action message");

        let mut received_error = false;
        while !received_error {
            if let Some(response) = rx.recv().await {
                if let Response::NotFound(_) = response {
                    received_error = true;
                }
            }
        }

        assert!(
            received_error,
            "Should receive error for action without room"
        );

        let leave_msg = create_message(JointMessageMethod::Leave);
        tx.send(leave_msg)
            .await
            .expect("Failed to send leave message");

        let mut received_error = false;
        while !received_error {
            if let Some(response) = rx.recv().await {
                if let Response::NotFound(_) = response {
                    received_error = true;
                }
            }
        }

        assert!(
            received_error,
            "Should receive error for leave without room"
        );

        drop(tx);
        drop(rx);
    }

    #[tokio::test]
    async fn test_channel_closing() {
        let joint = MPSCJoint::<TestReducer>::new(TestReducer::default());

        let (tx, rx) = joint.connect(10);
        drop(tx);
        drop(rx);

        let (tx2, mut rx2) = joint.connect(10);

        let create_msg = create_message(JointMessageMethod::Create);
        tx2.send(create_msg)
            .await
            .expect("Failed to send create message");

        let mut room_created = false;
        while !room_created {
            if let Some(response) = rx2.recv().await {
                if let Response::RoomCreated(_) = response {
                    room_created = true;
                }
            }
        }

        assert!(
            room_created,
            "Should be able to create room after previous channel closed"
        );

        drop(tx2);
        drop(rx2);
    }
}
