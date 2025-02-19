use crate::utils::types::Broadcastable;

pub trait Dispatchable<A, T: Broadcastable> {
    fn new() -> Self;
    async fn dispatch(&mut self, client_id: u64, action: A) -> Result<T, String>;
}
