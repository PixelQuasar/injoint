use futures::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

pub type WSSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type WSStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
