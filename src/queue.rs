use std::{
    cell::RefCell,
    collections::vec_deque::VecDeque,
    sync::{Arc, Condvar, Mutex, RwLock},
};

pub struct Queue<T> {
    data: RwLock<RefCell<VecDeque<T>>>,
    notifier: Condvar,
    pop_lock: Mutex<()>,
}

impl<T> Queue<T> {
    fn default() -> Arc<Self> {
        Arc::new(Self {
            data: RwLock::new(RefCell::new(VecDeque::new())),
            notifier: Condvar::new(),
            pop_lock: Mutex::new(()),
        })
    }

    pub fn push(&self, data: T) {
        match self.data.write() {
            Err(_) => unimplemented!("Queue poisoned lock"),
            Ok(mut guard) => {
                let queue = guard.get_mut();
                queue.push_back(data);
                self.notifier.notify_one();
            }
        }
    }

    pub fn pop(&self) -> T {
        let data = match self.pop_lock.lock() {
            Err(_) => unimplemented!("Queue poisoned lock"),
            Ok(mut pop_guard) => {
                // While empty, wait.
                // Notifier may randomly awake the thread.
                while self
                    .data
                    .read()
                    .map_or(true, |queue| (*queue).borrow().len() == 0)
                {
                    pop_guard = self
                        .notifier
                        .wait(pop_guard)
                        .unwrap_or_else(|err| err.into_inner());
                }

                // Can exit the while without issues because no other writer will be popping

                match self.data.write() {
                    Err(_) => unimplemented!("Queue poisoned lock"),
                    Ok(mut guard) => {
                        let queue = guard.get_mut();
                        queue
                            .pop_front()
                            .expect("Queue locks should've prevented the empty queue.")
                    }
                }
            }
        };

        self.notifier.notify_one();
        data
    }
}

unsafe impl<T> Sync for Queue<T> {}
unsafe impl<T> Send for Queue<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    #[test]
    fn queue_fifo() {
        let queue = Queue::default();
        let a = 0;
        let b = 1;
        queue.push(a);
        queue.push(b);

        assert_eq!(queue.pop(), a);
        assert_eq!(queue.pop(), b);
    }

    #[test]
    fn pop_empty() {
        let value: i32 = 3;

        let q: Arc<Queue<i32>> = Queue::default();

        let p = q.clone();
        let pop1 = thread::spawn(move || p.pop());

        thread::sleep(time::Duration::from_millis(100));

        let p = q.clone();
        let pop2 = thread::spawn(move || p.pop());

        q.push(value.clone());

        assert!(pop1.join().map_or(false, |tpop| tpop == value));
        assert!(!pop2.is_finished());
    }

    #[test]
    fn concurrent_popping() {
        let value: i32 = 3;

        let q: Arc<Queue<i32>> = Queue::default();
        q.push(value.clone());

        let p1 = q.clone();
        let p2 = q.clone();
        let pop1 = thread::spawn(move || p1.pop());
        let pop2 = thread::spawn(move || p2.pop());

        while !(pop1.is_finished() | pop2.is_finished()) {
            thread::sleep(time::Duration::from_millis(100));
        }

        assert!(
            pop1.is_finished() ^ pop2.is_finished(),
            "Exactly one thread must finished. that was not the case. T1: {}; T2: {}",
            pop1.is_finished(),
            pop2.is_finished()
        );
        let finished = if pop1.is_finished() { pop1 } else { pop2 };
        assert!(finished.join().map_or(false, |tpop| tpop == value));
    }
}
