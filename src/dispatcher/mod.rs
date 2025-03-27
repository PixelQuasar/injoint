use crate::utils::types::{Broadcastable, Receivable};
use std::future::Future;

pub trait Dispatchable: Send + Sync {
    type Action: Receivable + Send;
    type Response: Broadcastable;
    fn new() -> Self;
    fn dispatch(
        &mut self,
        client_id: u64,
        action: Self::Action,
    ) -> impl Future<Output = Result<Self::Response, String>> + Send;
}
