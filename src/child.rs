use std::{
    cell::RefCell,
    sync::{Arc, Weak},
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

    loop {
        pool.upgrade().map(|p| (p.fetch_task())());
    }
}
