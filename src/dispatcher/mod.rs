mod test;

use crate::utils::types::{Broadcastable, Receivable};
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionResponse<S: Serialize> {
    pub status: String,
    pub state: S,
    pub author: u64,
    pub data: String,
}

pub trait Dispatchable: Send + Sync + Clone {
    type Action: Receivable + Send;
    type State: Broadcastable;

    fn dispatch(
        &mut self,
        client_id: u64,
        action: Self::Action,
    ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send;

    fn extern_dispatch(
        &mut self,
        client_id: u64,
        action: &str,
    ) -> impl Future<Output = Result<ActionResponse<Self::State>, String>> + Send;

    fn get_state(&self) -> Self::State;
}
