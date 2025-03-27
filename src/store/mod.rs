// use std::sync::Arc;
//
// use dashmap::DashMap;
// use serde::{Deserialize, Serialize};
//
// use crate::joint::room::Room;
//
// use super::utils::{get_id, WebStateTrait};
//
// #[derive(Clone, Deserialize, Serialize)]
// struct Client {
//     token: usize,
//     address: String,
//     foreign_id: String,
// }
//
// joint_impl Client {
//     pub fn new(address: String, foreign_id: String) -> Self {
//         Client {
//             token: get_id(),
//             address,
//             foreign_id,
//         }
//     }
//
//     pub fn get_token(&self) -> usize {
//         self.token
//     }
// }
//
// struct Store<T>
// where
//     T: WebStateTrait,
// {
//     active_clients: Arc<DashMap<usize, Client>>,
//     rooms: Arc<DashMap<usize, Room<T>>>,
// }
//
// joint_impl<T> Store<T>
// where
//     T: WebStateTrait,
// {
//     pub fn new() -> Self {
//         Store {
//             active_clients: Arc::new(DashMap::new()),
//             rooms: Arc::new(DashMap::new()),
//         }
//     }
//
//     pub fn add_room(&self, owner: usize) -> usize {
//         let id = crate::joint::utils::get_id();
//         self.rooms.insert(id, Room::new(owner, id, Arc::new(None)));
//         id
//     }
//
//     pub fn remove_room(&self, id: usize) {
//         self.rooms.remove(&id);
//     }
//
//     pub fn get_room(&self, id: usize) -> Option<Room<T>> {
//         self.rooms.get(&id).map(|room| room.clone())
//     }
//
//     pub fn add_client<S>(&self, address: String, foreign_id: String) -> usize {
//         let client = Client::new(address, foreign_id);
//         self.active_clients
//             .insert(client.get_token(), client.clone());
//         return client.get_token();
//     }
//
//     pub fn remove_client(&self, token: usize) {
//         self.active_clients.remove(&token);
//     }
//
//     pub fn get_client(&self, token: usize) -> Option<Client> {
//         self.active_clients.get(&token).map(|client| client.clone())
//     }
// }
