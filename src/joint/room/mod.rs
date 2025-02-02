use std::sync::Arc;

use crate::joint::utils::WebStateTrait;

#[derive(Clone)]
pub struct Room<T>
where
    T: WebStateTrait,
{
    id: usize,
    owner: usize,
    clients: Vec<usize>,
    state: Arc<Option<T>>,
}

impl<T> Room<T>
where
    T: WebStateTrait,
{
    pub fn new(owner: usize, id: usize, state: Arc<Option<T>>) -> Room<T> {
        Room {
            id,
            owner,
            clients: vec![],
            state,
        }
    }
}
