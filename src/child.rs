use std::{
    borrow::Borrow,
    cell::RefCell,
    future::Future,
    pin::Pin,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex, Weak,
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

use crate::pool::Pool;

thread_local! {
    static POOL: RefCell<Weak<Pool>> = RefCell::new(Arc::downgrade(&Pool::new(0)));
}

pub fn get_thread_pool() -> Option<Arc<Pool>> {
    POOL.with_borrow(|pool| pool.upgrade())
}

pub fn thread_operation(pool: Weak<Pool>) {
    POOL.set(pool.clone());

    while let Some(_p) = pool.upgrade() {
        // let task = p.fetch_task();
        thread::sleep(Duration::from_millis(100));
    }
}
