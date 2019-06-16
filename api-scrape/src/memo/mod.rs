// TODO: convert to crate
use ccl::dhashmap::DHashMap;
use std::hash::Hash;
use std::future::Future;

mod cell;
pub use cell::{Memo, MemoResult};

/// A memoizing, thread-safe dictionary.
pub struct MemoDict<K: Hash + Eq, V>(DHashMap<K, Memo<dyn Future<Output = V>>>);

impl <K, V> MemoDict<K, V>
    where K: Hash + Eq + Send + Sync, V: Send + Sync {
    
    pub fn new() -> MemoDict<K, V> {
        MemoDict(Default::default())
    }
}
