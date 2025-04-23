#![allow(dead_code)]
#![allow(path_statements)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod child;
mod pool;
mod queue;
mod task;

mod prelude;

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn coroutine_support() {
        // test couroutines can be added to the pool
        let pool = Pool::new(1);
        let corout = async |arg| {
            arg;
        };
        pool.insert_task(corout(true));
    }
}
