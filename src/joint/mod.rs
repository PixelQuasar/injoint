use crate::broadcaster::Broadcaster;
use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use async_trait::async_trait;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    pub fn new() -> Self {
        AbstractJoint {
            broadcaster: Broadcaster::new(),
            reducer: Arc::new(Mutex::new(R::default())),
        }
    }

    // Dispatches developer-defined action (performed by user) to joint reducer
    pub async fn dispatch(
        &mut self,
        client_id: u64,
        action: R::Action,
    ) -> Result<R::Response, String> {
        let mut reducer = self.reducer.lock().await;
        reducer.dispatch(client_id, action).await
    }

    // handles new abstract split sink
    pub async fn handle_stream<S>(&self, receiver: &mut S, sender: Sink)
    where
        S: StreamAdapter + Unpin + Send + Sync,
    {
        let new_client_id = rand::rng().random::<u64>();
        println!("new client: {}", new_client_id);

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
        println!("client {} disconnected", new_client_id);
    }
}

#[async_trait]
pub trait Joint {
    async fn listen(&mut self);
}
