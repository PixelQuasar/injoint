use crate::broadcaster::Broadcaster;
use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use rand::Rng;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod axum;
pub mod mpsc;
mod test;
pub mod ws;

// Root abstract struct that provides all publish-subscribe functionality
pub struct AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin,
    R: Dispatchable + Send,
{
    broadcaster: Broadcaster<Sink>,
    reducer: Arc<Mutex<R>>,
}

impl<R, Sink> AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin,
    R: Dispatchable + Send,
{
    pub fn new(default_reducer: R) -> Self {
        AbstractJoint {
            broadcaster: Broadcaster::new(),
            reducer: Arc::new(Mutex::new(default_reducer)),
        }
    }

    // Dispatches developer-defined action (performed by user) to joint reducer
    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::Response>, String> {
        let mut reducer = self.reducer.lock().await;
        reducer.extern_dispatch(client_id, action).await
    }

    // handles new abstract split sink
    pub async fn handle_stream<S>(&self, receiver: &mut S, sender: Sink)
    where
        S: StreamAdapter + Unpin + Send + Sync,
    {
        let new_client_id = rand::rng().random::<u64>();

        self.broadcaster
            .add_client_connection(
                Client::new(new_client_id, None, String::new(), String::new()),
                sender,
            )
            .await;

        self.broadcaster
            .handle_rx(new_client_id, receiver, self.reducer.clone())
            .await;

        self.broadcaster
            .remove_client_connection(new_client_id)
            .await;
    }
}
