use serde::de::DeserializeOwned;
use serde::Serialize;

pub trait Broadcastable: Serialize {}

pub trait Receivable: DeserializeOwned {}
