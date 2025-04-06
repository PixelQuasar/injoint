use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
pub enum JointMessage<T>
where
    T: Serialize + Send + Sync,
{
    Create,
    Join(u64),
    Action(T),
    Leave,
}

#[derive(Serialize)]
pub struct MessageRequest<T>
where
    T: Serialize + Send + Sync,
{
    pub message: JointMessage<T>,
    client_token: String,
}

impl<T> MessageRequest<T>
where
    T: Serialize + Send + Sync,
{
    pub fn new(message: JointMessage<T>) -> Self {
        MessageRequest {
            message,
            client_token: "0".to_string(),
        }
    }

    pub fn create_room() -> Self {
        MessageRequest::new(JointMessage::Create)
    }

    pub fn join_room(room_id: u64) -> Self {
        MessageRequest::new(JointMessage::Join(room_id))
    }

    pub fn leave_room() -> Self {
        MessageRequest::new(JointMessage::Leave)
    }

    pub fn action(action: T) -> Self {
        MessageRequest::new(JointMessage::Action(action))
    }
}
