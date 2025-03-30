use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

#[derive(Serialize, Debug)]
struct ChatRequest {
    message: RequestMessage,
    client_token: String,
}

#[derive(Serialize, Debug)]
struct RequestMessage {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    status: String,
    message: Value,
}

#[derive(Deserialize, Debug)]
struct ChatMessage {
    author: u64,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatState {
    messages: Vec<ChatMessage>,
    users: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct ActionResponse {
    status: String,
    data: String,
    author: u64,
    state: ChatState,
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = "ws://127.0.0.1:3000";

    println!("Connecting to WebSocket server at {}", url);
    let (ws_stream, _) = connect_async(url).await?;
    println!("Connected! Starting chat client...");

    let (writer, reader) = ws_stream.split();
    let writer = Arc::new(Mutex::new(writer));
    let reader = Arc::new(Mutex::new(reader));

    join_room(Arc::clone(&writer)).await?;

    println!("Chat client started.");
    println!("Commands:");
    println!("  /name <username> - Set your username");
    println!("  /quit - Exit the chat");
    println!("  Just type anything else to send a message");

    let reader_handle = {
        let writer_clone = Arc::clone(&writer);
        tokio::spawn(async move {
            let mut reader = reader.lock().await;
            while let Some(msg) = reader.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = handle_server_message(&text, &writer_clone).await {
                            eprintln!("Error handling message: {}", e);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        })
    };

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();

        if line.starts_with("/quit") {
            leave_room(Arc::clone(&writer)).await?;
            println!("Goodbye!");
            break;
        } else if line.starts_with("/name ") {
            let name = line.trim_start_matches("/name ").trim();
            if !name.is_empty() {
                identify_user(Arc::clone(&writer), name).await?;
            }
        } else if !line.is_empty() {
            send_message(Arc::clone(&writer), line).await?;
        }
    }

    reader_handle.abort();

    Ok(())
}

async fn join_room(
    writer: Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
) -> Result<()> {
    let join_request = ChatRequest {
        message: RequestMessage {
            msg_type: "Create".to_string(),
            data: None,
        },
        client_token: "0".to_string(),
    };

    let join_json = serde_json::to_string(&join_request)?;
    writer
        .lock()
        .await
        .send(Message::Text(join_json.into()))
        .await?;

    Ok(())
}
async fn create_room(
    writer: Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
) -> Result<()> {
    let create_request = ChatRequest {
        message: RequestMessage {
            msg_type: "Create".to_string(),
            data: None,
        },
        client_token: "0".to_string(),
    };

    let create_json = serde_json::to_string(&create_request)?;
    writer
        .lock()
        .await
        .send(Message::Text(create_json.into()))
        .await?;

    Ok(())
}

async fn identify_user(
    writer: Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
    username: &str,
) -> Result<()> {
    let action_data = serde_json::json!({
        "type": "ActionIdentifyUser",
        "data": username
    })
    .to_string();

    let identify_request = ChatRequest {
        message: RequestMessage {
            msg_type: "Action".to_string(),
            data: Some(Value::String(action_data)),
        },
        client_token: "0".to_string(),
    };

    let identify_json = serde_json::to_string(&identify_request)?;
    writer
        .lock()
        .await
        .send(Message::Text(identify_json.into()))
        .await?;

    Ok(())
}

async fn send_message(
    writer: Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
    message: &str,
) -> Result<()> {
    let action_data = serde_json::json!({
        "type": "ActionSendMessage",
        "data": message
    })
    .to_string();

    let message_request = ChatRequest {
        message: RequestMessage {
            msg_type: "Action".to_string(),
            data: Some(Value::String(action_data)),
        },
        client_token: "0".to_string(),
    };

    let message_json = serde_json::to_string(&message_request)?;
    writer
        .lock()
        .await
        .send(Message::Text(message_json.into()))
        .await?;

    Ok(())
}

async fn leave_room(
    writer: Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
) -> Result<()> {
    let leave_request = ChatRequest {
        message: RequestMessage {
            msg_type: "Leave".to_string(),
            data: None,
        },
        client_token: "0".to_string(),
    };

    let leave_json = serde_json::to_string(&leave_request)?;
    writer
        .lock()
        .await
        .send(Message::Text(leave_json.into()))
        .await?;

    Ok(())
}

async fn handle_server_message(
    message: &str,
    writer: &Arc<
        Mutex<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    >,
) -> Result<()> {
    let response: ChatResponse = match serde_json::from_str(message) {
        Ok(r) => r,
        Err(e) => return Err(anyhow!("Failed to parse server response: {}", e)),
    };

    match response.status.as_str() {
        "RoomCreated" => {
            println!("Room created successfully");
            join_room(Arc::clone(writer)).await?;
        }
        "RoomJoined" => {
            println!(
                "Joined room successfully! New user ID: {}",
                response.message
            );
        }
        "RoomLeft" => {
            println!("User {} left the room", response.message);
        }
        "Action" => {
            match serde_json::from_value::<ActionResponse>(response.message) {
                Ok(action) => {
                    match action.status.as_str() {
                        "ActionIdentifyUser" => {
                            println!("User {} identified as '{}'", action.author, action.data);
                            // Print current users
                            let users = action.state.users;
                            println!(
                                "Currently in room: {}",
                                users
                                    .values()
                                    .map(|s| s.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        "ActionSendMessage" => {
                            let author_id = action.author.to_string();
                            let username = action
                                .state
                                .users
                                .get(&author_id)
                                .cloned()
                                .unwrap_or_else(|| format!("User {}", author_id));

                            println!("{}: {}", username, action.data);
                        }
                        _ => println!("Unknown action: {}", action.status),
                    }
                }
                Err(e) => println!("Error parsing action response: {}", e),
            }
        }
        "NotFound" => {
            println!("Room not found, creating a new room...");
            create_room(Arc::clone(writer)).await?;
        }
        "ClientError" => {
            println!("Client error: {}", response.message);
        }
        "ServerError" => {
            println!("Server error: {}", response.message);
        }
        _ => {
            println!("Unknown response status: {}", response.status);
        }
    }

    Ok(())
}
