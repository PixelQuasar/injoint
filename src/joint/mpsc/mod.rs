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

pub struct MPSCJoint<R: Dispatchable + Send + 'static> {
    joint: Arc<AbstractJoint<R, MPSCSink>>,
}

impl<R: Dispatchable + Send + 'static> MPSCJoint<R> {
    pub fn new(default_reducer: R) -> Self {
        MPSCJoint {
            joint: Arc::new(AbstractJoint::new(default_reducer)),
        }
    }

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

    pub async fn dispatch(
        &self,
        client_id: u64,
        action: &str,
    ) -> Result<ActionResponse<R::Response>, String> {
        self.joint.dispatch(client_id, action).await
    }
}
