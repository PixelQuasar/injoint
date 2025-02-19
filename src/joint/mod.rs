use crate::broadcaster::Broadcaster;
use crate::dispatcher::Dispatchable;
use crate::utils::types::Broadcastable;
use crate::utils::types::WebMethods;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use tokio::sync::mpsc::UnboundedReceiver;

pub struct Joint<A, T: Broadcastable, R: Dispatchable<A, T>> {
    broadcaster: Broadcaster,
    reducer: Rc<RefCell<R>>,
    phantom_a: PhantomData<A>,
    phantom_t: PhantomData<T>
}

impl<A, T: Broadcastable, R: Dispatchable<A, T>> Joint<A, T, R> {
    pub async fn new() -> Self {
        Joint {
            broadcaster: Broadcaster::new(),
            reducer: Rc::new(RefCell::new(R::new())),
            phantom_a: PhantomData,
            phantom_t: PhantomData
        }
    }

    pub async fn dispatch(&mut self, client_id: u64, action: A) -> Result<T, String> {
        self.reducer.borrow_mut().dispatch(client_id, action).await
    }

    pub async fn handle_rx(&mut self, rx: UnboundedReceiver<WebMethods<A>>) {
        self.broadcaster.handle_rx(rx, self.reducer.clone()).await;
    }
}
