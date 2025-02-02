use std::sync::Arc;
use tokio::sync::Mutex;

// type ActionType<T, A> = Box<dyn FnMut(Arc<Mutex<T, A>>)>;

// pub struct Reducer<T, A> {
//     route: &'static str,
//     action: Arc<ActionType<T, A>>,
// }

// impl<T> Reducer<T, A> {
//     pub fn new(route: &str, action: ActionType<T, A>) -> Self {
//         Reducer {
//             route,
//             action: Arc::new(Box::new(action)),
//         }
//     }
// }

// struct Store<T> {
//     state: Arc<Mutex<T>>,
//     reducers: Vec<Reducer<T>>,
// }

// impl<T> Store<T> {
//     pub fn new(state: T, reducers: Vec<Reducer<T>>) -> Self {
//         Store {
//             state: Arc::new(Mutex::new(state)),
//             reducers: Arc::new(reducers),
//         }
//     }

//     pub fn add_reducer(&mut self, reducer: Reducer<T>) {
//         self.reducers.push(reducer);
//     }

//     pub async fn dispatch(&self, route: &str) {
//         for reducer in self.reducers.iter() {
//             if reducer.route == route {
//                 let mut action = reducer.action.clone();
//                 action(self.state.clone()).await;
//             }
//         }
//     }
// }

struct DemoState {
    count: i32,
    users: Vec<String>,
}

impl DemoState {
    pub fn new() -> Self {
        DemoState {
            count: 0,
            users: vec![],
        }
    }
}

fn test_main() {
    let store = Store::<DemoState>::new(
        DemoState::new(),
        vec![Reducer::new("/route/add-user", |state, name: String| {
            let mut state = state.lock().await;
            state.users.push(name);
            state.count += 1;
        })],
    );

    store.dispatch("/route/add-user").await;
}

enum Actions {
    AddUser(String),
    RemoveUser(String),
    RemoveAll(),
    getNthUser(i32),
}

struct MyStore {
    state: Arc<Mutex<DemoState>>,
}

struct MyJoint {
    state: DemoState,
}
