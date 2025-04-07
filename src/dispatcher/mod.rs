/// This module contains the core functionality for dispatching actions to reducers.
mod test;

use crate::utils::types::{Broadcastable, Receivable};
use serde::{Deserialize, Serialize};
use std::future::Future;

/// Response structure for actions dispatched to the reducer.
///
/// This structure contains the status of the action, the current state of the reducer,
#[derive(Serialize, Deserialize, Debug)]
pub struct ActionResponse<S: Serialize> {
    pub status: String,
    pub state: S,
    pub author: u64,
    pub data: String,
}

/// Core trait for dispatching actions to a reducer.
///
/// Required to be implemented by any reducer that will be used in `Joint`.
///
/// Auto-generated for target struct by `#[reducer_actions]` macro in
/// `injoint::codegen` submodule.
///
/// This trait defines the core functionality of a reducer, including the ability to
/// dispatch actions, handle external dispatches, and retrieve the current state.
///
/// # example
///
/// ```rust
/// use injoint::dispatcher::{ActionResponse, Dispatchable};
/// use injoint::utils::types::{Broadcastable, Receivable};
/// use serde::{Deserialize, Serialize};
/// use std::future::Future;
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// enum TestAction {
///    Increment,
///   Add(i32),
/// }
///
/// impl Receivable for TestAction {}
/// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
/// struct TestState {
///   counter: i32,
/// }
///
/// impl Broadcastable for TestState {}
/// #[derive(Clone, Default)]
/// struct TestReducer {
///   state: TestState,
/// }
///
/// impl Dispatchable for TestReducer {
///     type Action = TestAction;
///     type State = TestState;
///
///     async fn dispatch(
///         &mut self,
///         client_id: u64,
///         action: Self::Action,
///     ) -> Result<ActionResponse<Self::State>, String> {
///         match action {
///             TestAction::Increment => {
///                 self.state.counter += 1;
///             }
///             TestAction::Add(value) => {
///                 self.state.counter += value;
///             }
///         }
///
///         Ok(ActionResponse {
///             status: "success".to_string(),
///             state: self.state.clone(),
///             author: client_id,
///             data: "".to_string(),
///          })
///     }
///
///     async fn extern_dispatch(
///         &mut self,
///         client_id: u64,
///         action: &str,
///     ) -> Result<ActionResponse<Self::State>, String> {
///         let action: TestAction = serde_json::from_str(action).unwrap();
///         self.dispatch(client_id, action).await
///     }
///
///     fn get_state(&self) -> Self::State {
///         self.state.clone()
///     }
/// }
/// ```
///
pub trait Dispatchable: Send + Sync + Clone {
    type Action: Receivable + Send;
    type State: Broadcastable;

    /// Dispatches an action to the reducer.
    ///
    /// This method is responsible for handling the action and updating the state of the reducer.
    /// It takes the client ID and the action as parameters and returns a `Future` that resolves to an `ActionResponse`.
    ///
    fn dispatch(
        &mut self,
        client_id: u64,
        action: Self::Action,
    ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send;

    /// Handles external dispatches to the reducer.
    ///
    /// This method is responsible for handling external actions that are not directly tied to the reducer's state.
    /// It takes the client ID and the action as parameters and returns a `Future` that resolves to an `ActionResponse`.
    ///
    fn extern_dispatch(
        &mut self,
        client_id: u64,
        action: &str,
    ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send;

    /// Retrieves the current state of the reducer.
    fn get_state(&self) -> Self::State;
}
