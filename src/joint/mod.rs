use crate::broadcaster::Broadcaster;
use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::Dispatchable;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::Mutex;

// Root abstract struct that provides all publish-subscribe functionality
pub struct AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin + Send + Sync,
    R: Dispatchable + Send + Sync,
{
    broadcaster: Broadcaster<Sink>,
    reducer: Arc<Mutex<R>>,
}

impl<R, Sink> AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin + Send + Sync,
    R: Dispatchable + Send + Sync,
{
    pub fn new() -> Self {
        AbstractJoint {
            broadcaster: Broadcaster::new(),
            reducer: Arc::new(Mutex::new(R::new())),
        }
    }

    // Dispatches developer-defined action (performed by user) to joint reducer
    pub async fn dispatch(
        &mut self,
        client_id: u64,
        action: R::Action,
    ) -> Result<R::Response, String> {
        let mut reducer_guard = self.reducer.lock().await;
        reducer_guard.dispatch(client_id, action).await
    }

    // handles new abstract split sink
    pub async fn handle_stream<S>(&mut self, receiver: &mut S, sender: Sink)
    where
        S: StreamAdapter + Unpin + Send + Sync,
    {
        let new_client_id = rand::rng().random::<u64>();

        self.broadcaster.add_client_connection(
            Client::new(new_client_id, None, String::new(), String::new()),
            sender,
        );

        self.broadcaster
            .handle_rx(new_client_id, receiver, self.reducer.clone())
            .await;

        self.broadcaster.remove_client_connection(new_client_id);
    }

    // pub fn get_client(&self, id: u64) -> Option<&Box<dyn Client<Connection = C>>> {
    //     self.broadcaster.get_client(id)
    // }
    //
    // pub fn add_client(&mut self, client: Box<dyn Client<Connection = C>>) {
    //     self.broadcaster.add_client(client);
    // }
    //
    // pub fn remove_client(&mut self, client_id: u64) {
    //     self.broadcaster.remove_client(client_id);
    // }
}
