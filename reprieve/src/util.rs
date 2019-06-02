use lazy_static::lazy_static;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use std::cell::RefCell;
use std::thread_local;
use std::time::Instant;

lazy_static! {
    static ref START: Instant = Instant::now();
}

thread_local! {
    static RNG: RefCell<XorShiftRng> = RefCell::new(XorShiftRng::seed_from_u64(
        START.elapsed().subsec_nanos() as u64,
    ));
}

#[allow(unused)]
pub(crate) fn random<T>() -> T
where
    rand::distributions::Standard: rand::distributions::Distribution<T>,
{
    RNG.with(|rng| rng.borrow_mut().gen())
}
