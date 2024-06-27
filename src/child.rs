use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use crate::pool::Pool;

thread_local! {
    static POOL: RefCell<Arc<Pool>> = RefCell::new(Pool::new(0));
}

pub fn get_thread_pool() -> Arc<Pool> {
    POOL.with_borrow(|pool| pool.clone())
}

pub fn thread_operation(pool: Arc<Pool>) {
    POOL.set(pool.clone());

    loop {
        if pool.any_thread_waking_up() {
            if pool.restart_thread() {
                pool.remove_self();
                return;
            }
        }

        let func = pool.fetch_task();
        let _ = func();
    }
}
