/// Provides a joint implementation using Tokio MPSC (Multi-Producer, Single-Consumer) channels.
///
/// This is useful for in-process communication or testing where network connections are not required.
mod test;

use crate::connection::{SinkAdapter, StreamAdapter};
use crate::dispatcher::{ActionResponse, Dispatchable};
use crate::joint::AbstractJoint;
use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// An implementation of [`SinkAdapter`] that sends responses over a `tokio::sync::mpsc::Sender`.
#[derive(Clone)]
pub struct MPSCSink {
    sender: Sender<Response>,
}

#[async_trait]
impl SinkAdapter for MPSCSink {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sender.send(response).await.map_err(|e| {
            Box::new(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("Failed to send response: {}", e),
            )) as _
        })
    }
}

/// An implementation of [`StreamAdapter`] that receives messages from a `tokio::sync::mpsc::Receiver`.
pub struct MPSCStream {
    receiver: Receiver<JointMessage>,
}

#[async_trait]
impl StreamAdapter for MPSCStream {
    async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>> {
        self.receiver.recv().await.ok_or_else(|| {
            Box::new(io::Error::new(io::ErrorKind::BrokenPipe, "Channel closed")) as _
        })
    }
}

/// An `injoint` joint that uses Tokio MPSC channels for communication.
///
/// This allows clients to connect and interact with the joint by sending `JointMessage`s
/// and receiving `Response`s through channels, without involving networking.
/// Useful for testing or embedding the joint logic within a single process.
///
/// `R` represents the application-specific `Dispatchable` state/reducer.
pub struct MPSCJoint<R: Dispatchable + Send + 'static> {
    joint: Arc<AbstractJoint<R, MPSCSink>>,
}

// Add Clone bound here if AbstractJoint requires R: Clone
impl<R: Dispatchable + Send + Clone + 'static> MPSCJoint<R> {
    /// Creates a new `MPSCJoint`.
    ///
    /// Requires a default instance of the application's `Dispatchable` reducer.
    pub fn new(default_reducer: R) -> Self {
        MPSCJoint {
            joint: Arc::new(AbstractJoint::new(default_reducer)),
        }
    }

    /// Connects a new client via MPSC channels.
    ///
    /// Creates a pair of channels: one for the client to send messages (`Sender<JointMessage>`)
    /// and one for the client to receive responses (`Receiver<Response>`).
    ///
    /// A background task is spawned to handle the communication between these channels
    /// and the underlying `AbstractJoint`.
    ///
    /// # Arguments
    /// * `buffer_size` - The buffer size for the created MPSC channels.
    ///
    /// # Returns
    /// A tuple containing the sender for client messages and the receiver for server responses.
    pub fn connect(&self, buffer_size: usize) -> (Sender<JointMessage>, Receiver<Response>) {
        let (msg_tx, msg_rx) = mpsc::channel(buffer_size);
        let (resp_tx, resp_rx) = mpsc::channel(buffer_size);

        let joint = self.joint.clone();

        tokio::spawn(async move {
            let mut stream = MPSCStream { receiver: msg_rx };
            let sink = MPSCSink { sender: resp_tx };

            joint.handle_stream(&mut stream, sink).await;
        });

        (msg_tx, resp_rx)
    }

    /// Allows dispatching an action directly to the joint's reducer.
    ///
    /// Useful for controlling the joint state from outside the MPSC client connections.
    ///
    /// # Arguments
    /// * `client_id` - The ID of the client on whose behalf the action is dispatched.
    ///                 Note: The client must exist and be in a room for the dispatch to succeed.
    /// * `action` - A string slice representing the action to be dispatched (must be JSON serializable
    ///              according to the `Dispatchable::Action` type).
    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::State>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
