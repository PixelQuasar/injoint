use crate::utils::types::{Broadcastable, Receivable};
use std::future::Future;

pub trait Dispatchable: Send + Sync + Default {
    type Action: Receivable + Send + Sync;
    type Response: Broadcastable + Send + Sync;
    fn dispatch(
        &mut self,
        client_id: u64,
        action: Self::Action,
    ) -> impl Future<Output = Result<Self::Response, String>> + Send;
}
