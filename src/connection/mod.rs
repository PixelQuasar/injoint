/// This module defines the `SinkAdapter` and `StreamAdapter` traits.
mod test;

use crate::message::JointMessage;
use crate::response::Response;
use async_trait::async_trait;

/// `SinkAdapter` is a trait that defines the interface for sending messages.
///
/// It is implemented by different types of sinks, such as WebSocket or MPSC.
/// It can be used as sink adapter in implementing custom Joint struct.
///
/// # example
///
/// ```rust
/// use injoint::connection::SinkAdapter;
/// use tungstenite::Message;
/// use async_trait::async_trait;
/// use injoint::joint::mpsc;
/// use injoint::response::Response;
///
/// #[derive(Clone)]
/// struct WSSink {
///     tx: tokio::sync::mpsc::Sender<Result<Message, tungstenite::Error>>,
/// }
///
/// #[async_trait]
/// impl SinkAdapter for WSSink {
///     async fn send(
///         &mut self,
///         response: Response,
///     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///         let message_text = serde_json::to_string(&response)?;
///         self.tx
///             .send(Ok(Message::Text(message_text.into())))
///             .await
///             .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
///         Ok(())
///     }
/// }
/// ```
///
#[async_trait]
pub trait SinkAdapter {
    async fn send(
        &mut self,
        response: Response,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// `StreamAdapter` is a trait that defines the interface for receiving messages.
///
/// It is implemented by different types of streams, such as WebSocket or MPSC.
/// It can be used as stream adapter in implementing custom Joint struct.
///
/// # example
///
/// ```rust
/// use injoint::connection::StreamAdapter;
/// use injoint::message::JointMessage;
/// use async_trait::async_trait;
/// use futures_util::stream::SplitStream;
/// use futures_util::{io, StreamExt};
/// use tokio::net::TcpStream;
/// use tokio_tungstenite::WebSocketStream;
/// use tungstenite::Message;
/// use injoint::joint::mpsc;
/// use injoint::response::Response;
///
/// struct WSStream {
///     stream: SplitStream<WebSocketStream<TcpStream>>,
/// }
///
/// #[async_trait]
/// impl StreamAdapter for WSStream {
///     async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>> {
///         let message = self.stream.next().await.unwrap()?;
///         let message = match message {
///             Message::Text(text) => text,
///             _ => {
///                 return Err(Box::new(io::Error::new(
///                     io::ErrorKind::InvalidData,
///                     "Invalid data",
///                 )))
///             }
///         };
///         let message = serde_json::from_slice((&message).as_ref())?;
///         Ok(message)
///     }
/// }
/// ```
///
#[async_trait]
pub trait StreamAdapter {
    async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>>;
}
