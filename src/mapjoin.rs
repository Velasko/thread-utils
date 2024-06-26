use std::{
    cell::RefCell,
    collections::hash_map::HashMap,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub struct MapJoin<S> {
    map: Arc<Mutex<RefCell<HashMap<usize, S>>>>,
    args_size: usize,
}

impl<S> MapJoin<S> {
    pub fn new(map: Arc<Mutex<RefCell<HashMap<usize, S>>>>, args_size: usize) -> Self {
        Self {
            map,
            args_size
        }
    }

    pub fn join(&self) -> Vec<S> {
        loop {
            match self.map.lock() {
                Err(_) => panic!("Poison Error! Either thread pool panic or panic::catch_unwind did not catch user's function panic."),
                Ok(return_map) => {
                    if return_map.deref().borrow().len() >= self.args_size {
                        let mut rmap = return_map.take();
                        return (0..self.args_size)
                            .map(|key| rmap.remove(&key).unwrap())
                            .collect::<Vec<S>>();
                    };
                }
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        match self.map.try_lock() {
            Ok(return_map) => return_map.deref().borrow().len() == self.args_size,
            Err(_) => false,
        }
    }
}
