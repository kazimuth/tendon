use std::thread;
use std::time::Duration;

/// Exponenial sleep backoff with a max-wait of .25 seconds.
pub struct Backoff(u32, u32);

impl Backoff {
    pub fn new(max: u32) -> Backoff {
        Backoff(1, max)
    }
    pub fn wait(&mut self) {
        thread::sleep(Duration::from_millis(self.0 as u64));
        self.0 = (self.0 * 2) % self.1;
    }
    pub fn reset(&mut self) {
        self.0 = 1;
    }
}
