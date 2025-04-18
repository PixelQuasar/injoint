use crate::broadcaster::Broadcaster;
use crate::client::Client;
use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use rand::Rng;

#[cfg(not(tarpaulin))]
pub mod axum;
pub mod mpsc;
mod test;
#[cfg(not(tarpaulin))]
pub mod ws;

/// Root abstract struct that provides all publish-subscribe functionality
///
/// This struct is generic over the reducer type `R` and the sink type `Sink`.
///
/// `R` must implement the `Dispatchable` trait, which defines how actions are dispatched to the reducer.
/// `Sink` must implement the `SinkAdapter` trait, which defines how messages are sent to clients.
pub struct AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin + Clone,
    R: Dispatchable + Send + Clone,
{
    /// Broadcaster instance that handles the communication between clients and the reducer.
    pub(crate) broadcaster: Broadcaster<Sink, R>,
}

impl<R, Sink> AbstractJoint<R, Sink>
where
    Sink: SinkAdapter + Unpin + Clone,
    R: Dispatchable + Send + Clone,
{
    /// Creates a new instance of `AbstractJoint`.
    pub fn new(default_reducer: R) -> Self {
        AbstractJoint {
            broadcaster: Broadcaster::new(default_reducer),
        }
    }

    /// Dispatches developer-defined action (performed by user) to joint reducer
    ///
    /// This method takes a `client_id` and an `action` string as parameters.
    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.broadcaster.extern_dispatch(client_id, action).await
    }

    /// handles new abstract split sink
    ///
    /// This method takes a mutable reference to a `StreamAdapter` and a `Sink` as parameters.
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

        self.broadcaster.handle_rx(new_client_id, receiver).await;

        self.broadcaster
            .remove_client_connection(new_client_id)
            .await;
    }

    /// Returns a reference to the `Broadcaster` instance.
    #[allow(dead_code)] // used in tests
    pub(crate) fn get_broadcaster(&self) -> &Broadcaster<Sink, R> {
        &self.broadcaster
    }
}
