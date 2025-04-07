use serde::de::DeserializeOwned;
use serde::Serialize;

/// `Broadcastable` trait is used to mark a struct as broadcastable.
pub trait Broadcastable: Serialize {}

/// `Receivable` trait is used to mark a struct as receivable.
pub trait Receivable: DeserializeOwned {}
