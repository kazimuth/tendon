// TODO: convert to crate
use ccl::dhashmap::DHashMap;
use std::future::Future;
use std::hash::Hash;

mod cell;
pub use cell::{Memo, MemoResult};

type Entry<V> = Memo<dyn Future<Output = V> + Send + Sync + 'static>;

/// A memoizing, thread-safe dictionary.
pub struct MemoDict<K: Hash + Eq, V>(DHashMap<K, Entry<V>>);

impl<K, V> MemoDict<K, V>
where
    K: Clone + Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    pub fn new() -> MemoDict<K, V> {
        MemoDict(Default::default())
    }

    pub fn memo<C, F>(&self, k: &K, c: C) -> impl Future<Output = MemoResult<V>>
    where
        C: FnOnce() -> F,
        F: Future<Output = V> + Send + Sync + 'static,
    {
        if let Some(cur) = self.0.get(k) {
            (&*cur).clone()
        } else {
            let result: Entry<V> = Memo::new_dyn(c);
            self.0.insert(k.clone(), result.clone());
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoDict;

    #[runtime::test]
    async fn dict() {
        let d = MemoDict::new();

        let f1 = d.memo(&"hello", || async { 1u8 });
        let f2 = d.memo(&"hello", || async { 2u8 });
        let r2 = f2.await;
        let r1 = f1.await;

        assert_eq!(*r1, 1, "first future created should set result");
        assert_eq!(*r1, *r2);
    }
}
