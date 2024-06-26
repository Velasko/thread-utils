use std::sync::{Mutex, Condvar};

#[derive(Default)]
pub struct Alarm {
	lock: Mutex<bool>,
	buzzer: Condvar
}

impl Alarm {
	pub fn set(&self) {
		unimplemented!();
	}

	pub fn buzz(&self) {
		unimplemented!();
	}
}
