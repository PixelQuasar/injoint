/// This module contains utility functions and types for the injoint library.
mod test;
pub mod types;

use serde::{de::DeserializeOwned, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

/// `get_id` generates a unique ID for each call.
/// It uses an atomic counter to ensure thread safety.
pub fn get_id() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
