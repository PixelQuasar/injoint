use futures::stream::SplitSink;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::Message;

#[derive(Debug)]
pub struct Connection {
    pub id: usize,
    pub con: SplitSink<WebSocketStream<TcpStream>, Message>,
}

