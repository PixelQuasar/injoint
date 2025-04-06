#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{ActionResponse, Dispatchable};
    use crate::utils::types::{Broadcastable, Receivable};
    use serde::{Deserialize, Serialize};
    use std::future::Future;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestAction {
        Increment,
        Add(i32),
        Echo(String),
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
                    TestAction::Echo(message) => {
                        self.state.messages.push(message);
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
                let action: TestAction = serde_json::from_str(action)
                    .map_err(|e| format!("Failed to deserialize action: {}", e))?;
                self.dispatch(client_id, action).await
            }
        }

        fn get_state(&self) -> Self::State {
            self.state.clone()
        }
    }

    #[tokio::test]
    async fn test_action_response_serialization() {
        let state = TestState {
            counter: 42,
            messages: vec!["hello".to_string()],
        };

        let response = ActionResponse {
            status: "success".to_string(),
            state: state.clone(),
            author: 123,
            data: "Test data".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();

        let deserialized: ActionResponse<TestState> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "success");
        assert_eq!(deserialized.state, state);
        assert_eq!(deserialized.author, 123);
        assert_eq!(deserialized.data, "Test data");
    }

    #[tokio::test]
    async fn test_dispatchable_dispatch() {
        let mut reducer = TestReducer::default();

        assert_eq!(reducer.get_state().counter, 0);
        assert_eq!(reducer.get_state().messages.len(), 0);

        let client_id = 1;
        let result = reducer.dispatch(client_id, TestAction::Increment).await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().counter, 1);

        let result = reducer.dispatch(client_id, TestAction::Add(10)).await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().counter, 11);

        let message = "Hello, world!".to_string();
        let result = reducer
            .dispatch(client_id, TestAction::Echo(message.clone()))
            .await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().messages.len(), 1);
        assert_eq!(reducer.get_state().messages[0], message);

        let response = result.unwrap();
        assert_eq!(response.status, "success");
        assert_eq!(response.author, client_id);
        assert_eq!(response.state.counter, 11);
        assert_eq!(response.state.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_dispatchable_extern_dispatch() {
        let mut reducer = TestReducer::default();

        let client_id = 2;
        let action_json = r#"{"Increment":null}"#;
        let result = reducer.extern_dispatch(client_id, action_json).await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().counter, 1);

        let action_json = r#"{"Add":5}"#;
        let result = reducer.extern_dispatch(client_id, action_json).await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().counter, 6);

        let action_json = r#"{"Echo":"Hello from JSON"}"#;
        let result = reducer.extern_dispatch(client_id, action_json).await;
        assert!(result.is_ok());
        assert_eq!(reducer.get_state().messages.len(), 1);
        assert_eq!(reducer.get_state().messages[0], "Hello from JSON");

        let invalid_json = r#"{"InvalidAction":null}"#;
        let result = reducer.extern_dispatch(client_id, invalid_json).await;
        assert!(result.is_err());
    }
}
