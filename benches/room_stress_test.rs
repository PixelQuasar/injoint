use criterion::{criterion_group, criterion_main, Criterion};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::{Barrier, Mutex};
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tungstenite::Utf8Bytes;
use url::Url;

use injoint::dispatcher::{ActionResponse, Dispatchable};
use injoint::joint::ws::WebsocketJoint;
use injoint::message::{JointMessage, JointMessageMethod};
use injoint::response::Response;
use injoint::utils::types::{Broadcastable, Receivable};

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
struct BenchState {
    counter: i32,
}

impl Broadcastable for BenchState {}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum BenchAction {
    Add(i32),
}

impl Receivable for BenchAction {}

#[derive(Clone, Default)]
struct BenchReducer {
    state: BenchState,
}

impl Dispatchable for BenchReducer {
    type Action = BenchAction;
    type State = BenchState;

    async fn dispatch(
        &mut self,
        client_id: u64,
        action: BenchAction,
    ) -> Result<ActionResponse<BenchState>, String> {
        match action {
            BenchAction::Add(value) => {
                self.state.counter += value;
                Ok(ActionResponse {
                    status: "success".into(),
                    state: self.state.clone(),
                    author: client_id,
                    data: self.state.counter.to_string(),
                })
            }
        }
    }

    async fn extern_dispatch(
        &mut self,
        client_id: u64,
        action_str: &str,
    ) -> Result<ActionResponse<BenchState>, String> {
        let action: BenchAction = serde_json::from_str(action_str)
            .map_err(|e| format!("Failed to parse action: {}", e))?;
        self.dispatch(client_id, action).await
    }

    fn get_state(&self) -> BenchState {
        self.state.clone()
    }
}

fn create_message(method: JointMessageMethod) -> JointMessage {
    JointMessage {
        client_token: "benchmark-token".to_string(),
        message: method,
    }
}

fn create_action_message(action: BenchAction) -> JointMessage {
    let action_json = serde_json::to_string(&action).unwrap();
    create_message(JointMessageMethod::Action(action_json))
}

async fn run_websocket_benchmark(num_clients: usize, actions_per_client: usize) -> f64 {
    // Using a barrier to ensure server is ready before clients connect
    let server_ready = Arc::new(Barrier::new(2)); // Server + this task
    let server_ready_clone = server_ready.clone();
    let local_addr_arc = Arc::new(Mutex::new(None::<SocketAddr>)); // To share the actual address
    let local_addr_arc_clone = local_addr_arc.clone();

    // Spawn the server
    let server_handle = tokio::spawn(async move {
        let mut joint = WebsocketJoint::<BenchReducer>::new(BenchReducer::default());
        joint
            .bind_addr("127.0.0.1:0")
            .await
            .expect("Failed to bind to port 0"); // Bind to port 0

        // Store the actual address for clients
        let actual_addr = joint
            .local_addr()
            .expect("Failed to get local address after bind");
        *local_addr_arc_clone.lock().await = Some(actual_addr);

        // Signal that the server is ready
        server_ready_clone.wait().await;

        // Listen for connections (will run until the benchmark is done)
        joint.listen().await;
    });

    // Wait for server to be ready and get the address
    server_ready.wait().await;
    let actual_addr = {
        let mut addr_opt = local_addr_arc.lock().await;
        loop {
            if addr_opt.is_some() {
                break addr_opt.take().unwrap();
            }
            // Release lock and sleep briefly if address not set yet
            drop(addr_opt);
            tokio::time::sleep(Duration::from_millis(10)).await;
            addr_opt = local_addr_arc.lock().await;
        }
    };

    // Shared room_id to be discovered by the first client
    let room_id = Arc::new(Mutex::new(None::<u64>));

    // Connect clients using the actual address
    let url = format!("ws://{}", actual_addr);
    let url = Url::parse(&url).unwrap();

    let start_time = Instant::now();

    let room_id_clone = room_id.clone();
    let url_str = url.to_string();

    let first_client = tokio::spawn(async move {
        let (ws_stream, _) = connect_async(url_str).await.expect("Failed to connect");
        let (mut write, mut read) = ws_stream.split();

        let create_msg = create_message(JointMessageMethod::Create);
        let json = serde_json::to_string(&create_msg).unwrap();
        write
            .send(Message::Text(Utf8Bytes::from(&json)))
            .await
            .unwrap();

        let mut room_created = false;
        while !room_created {
            if let Some(msg) = read.next().await {
                let msg = msg.unwrap();
                if let Message::Text(text) = msg {
                    //println!("Received message: {}", text);
                    let response: Response = serde_json::from_str(&text).unwrap();
                    if let Response::RoomCreated(id) = response {
                        let mut room_id_guard = room_id_clone.lock().await;
                        *room_id_guard = Some(id);
                        room_created = true;
                    }
                }
            }
        }

        let mut actions_completed = 0;
        while actions_completed < actions_per_client {
            let action_msg = create_action_message(BenchAction::Add(1));
            let json = serde_json::to_string(&action_msg).unwrap();
            write
                .send(Message::Text(Utf8Bytes::from(&json)))
                .await
                .unwrap();

            while let Ok(Some(msg)) =
                tokio::time::timeout(Duration::from_millis(100), read.next()).await
            {
                let msg = msg.unwrap();
                if let Message::Text(text) = msg {
                    let response: Response = serde_json::from_str(&text).unwrap();
                    match response {
                        Response::Action(_) => {
                            actions_completed += 1;
                            break; // Break after receiving this action's response
                        }
                        Response::StateSent(_) => {
                            continue;
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            }
        }

        let leave_msg = create_message(JointMessageMethod::Leave);
        let json = serde_json::to_string(&leave_msg).unwrap();
        write
            .send(Message::Text(Utf8Bytes::from(&json)))
            .await
            .unwrap();
    });

    let mut room = None;
    while room.is_none() {
        let id = room_id.lock().await.clone();
        if id.is_some() {
            room = id;
        }
    }

    let room_id = room.unwrap();

    let mut client_handles = vec![first_client];

    for _ in 1..num_clients {
        let url_clone = url.to_string();
        let handle = tokio::spawn(async move {
            let (ws_stream, _) = connect_async(&url_clone).await.expect("Failed to connect");
            let (mut write, mut read) = ws_stream.split();

            let join_msg = create_message(JointMessageMethod::Join(room_id));
            let json = serde_json::to_string(&join_msg).unwrap();
            write
                .send(Message::Text(Utf8Bytes::from(&json)))
                .await
                .unwrap();

            let mut joined = false;
            while !joined {
                if let Some(msg) = read.next().await {
                    let msg = msg.unwrap();
                    if let Message::Text(text) = msg {
                        let response: Response = serde_json::from_str(&text).unwrap();
                        match response {
                            Response::RoomJoined(_) => joined = true,
                            Response::StateSent(_) => joined = true,
                            _ => {}
                        }
                    }
                }
            }

            let mut actions_completed = 0;
            while actions_completed < actions_per_client {
                let action_msg = create_action_message(BenchAction::Add(1));
                let json = serde_json::to_string(&action_msg).unwrap();
                write
                    .send(Message::Text(Utf8Bytes::from(&json)))
                    .await
                    .unwrap();

                while let Ok(Some(msg)) =
                    tokio::time::timeout(Duration::from_millis(100), read.next()).await
                {
                    let msg = msg.unwrap();
                    if let Message::Text(text) = msg {
                        let response: Response = serde_json::from_str(&text).unwrap();
                        match response {
                            Response::Action(_) => {
                                actions_completed += 1;
                                break;
                            }
                            Response::StateSent(_) => {
                                continue;
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
            }

            let leave_msg = create_message(JointMessageMethod::Leave);
            let json = serde_json::to_string(&leave_msg).unwrap();
            write
                .send(Message::Text(Utf8Bytes::from(&json)))
                .await
                .unwrap();
        });

        client_handles.push(handle);
    }

    for handle in client_handles {
        handle.await.unwrap();
    }

    let duration = start_time.elapsed();

    server_handle.abort();

    duration.as_secs_f64()
}

fn websocket_joint_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let configs = vec![
        (1, 100),    // 1 client, 100 actions
        (5, 100),    // 5 clients, 100 actions each
        (10, 100),   // 10 clients, 100 actions each
        (1, 1000),   // 1 client, 1000 actions each
        (5, 1000),   // 5 clients, 1000 actions each
        (10, 10000), // 10 clients, 1000 actions each
        (20, 50),    // 20 clients, 50 actions each
    ];

    let mut group = c.benchmark_group("WebSocket Joint Performance");

    for (clients, actions) in configs {
        let id = format!("clients={}_actions={}", clients, actions);

        group.bench_function(id, |b| {
            b.iter(|| rt.block_on(run_websocket_benchmark(clients, actions)));
        });
    }

    group.finish();
}

criterion_group!(benches, websocket_joint_benchmark);
criterion_main!(benches);
