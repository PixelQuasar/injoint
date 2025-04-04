use crate::utils::types::{Broadcastable, Receivable};
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Serialize, Deserialize)]
pub struct ActionResponse<S: Serialize> {
    pub status: String,
    pub state: S,
    pub author: u64,
    pub data: String,
}

pub trait Dispatchable: Send + Default {
    type Action: Receivable + Send;
    type Response: Broadcastable;
    fn dispatch(
        &mut self,
        client_id: u64,
        action: Self::Action,
    ) -> impl Future<Output = Result<ActionResponse<Self::Response>, String>> + Send;

    fn extern_dispatch(
        &mut self,
        client_id: u64,
        action: &str,
    ) -> impl Future<Output = Result<ActionResponse<Self::Response>, String>> + Send;
}
