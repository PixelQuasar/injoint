pub mod broadcaster;
pub mod connection;
pub mod controller;
pub mod message;
pub mod reducer;
pub mod room;
pub mod store;
pub mod tcp_handler;
pub mod utils;

use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use utils::get_id;

#[derive(Serialize, Deserialize, Clone)]
struct DemoModel {
    id: usize,
    name: String,
}

impl DemoModel {
    pub fn new(name: String) -> Self {
        DemoModel { id: get_id(), name }
    }
}

type CollectionNameType = String;

type JointQueryName = String;

struct Joint<T> {
    id: usize,
    reducers: Arc<DashMap<JointQueryName, Operation<T>>>,
}

impl<T> Joint<T> {
    pub async fn new() -> Self {
        Joint {
            id: get_id(),
            reducers: Arc::new(DashMap::new()),
        }
    }
}
