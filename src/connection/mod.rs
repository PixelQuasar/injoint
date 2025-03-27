use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;

#[async_trait]
pub trait SinkAdapter {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
pub trait StreamAdapter {
    async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>>;
}
