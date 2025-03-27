use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

pub trait Broadcastable: Serialize + Debug + Default {}

pub trait Receivable: DeserializeOwned + Debug {}
