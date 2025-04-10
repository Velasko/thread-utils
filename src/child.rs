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
    static POOL: RefCell<Option<Weak<Pool>>> = RefCell::new(None);
}

pub fn get_thread_pool() -> Option<Arc<Pool>> {
    POOL.with_borrow(|opt| opt.clone().map(|pool| pool.upgrade()))
        .flatten()
}

pub fn thread_operation(pool: Weak<Pool>) {
    POOL.set(Some(pool.clone()));

    while let None = pool.upgrade() {} // todo!("Improve the thread waiting for the pool to fully
                                       // initialize")

    while let Some(p) = pool.upgrade() {
        let task = p.fetch_task(); // sleep here makes the pool to never be dropped. Maybe copy the
                                   // Q ?
        thread::sleep(Duration::from_millis(0));
    }
}
