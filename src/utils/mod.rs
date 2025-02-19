pub mod types;

use serde::{de::DeserializeOwned, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait WebStateTrait: DeserializeOwned + Serialize + Clone + Sync + Send {}

pub fn get_id() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
