mod test;

use serde::Deserialize;
use std::fmt::Debug;

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum JointMessageMethod {
    Create,
    Join(u64),
    Leave,
    Action(String), // maybe this should be a generic type that deserializable?
}

#[derive(Debug, Deserialize, Clone)]
pub struct JointMessage {
    pub message: JointMessageMethod,
    pub client_token: String,
}

impl JointMessage {
    pub fn new(message: JointMessageMethod, client_token: String) -> Self {
        JointMessage {
            message,
            client_token,
        }
    }
}
