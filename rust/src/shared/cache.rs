/// Generic typed LRU cache with thread-safe access.
///
/// Replaces the duplicated `Lazy<Mutex<LruCache<K, V>>>` pattern
/// used for metadata and poster caches in metadata.rs.

use lru::LruCache;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::sync::Mutex;

pub struct TypedCache<K: Hash + Eq, V: Clone> {
    inner: Mutex<LruCache<K, V>>,
}

impl<K: Hash + Eq, V: Clone> TypedCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).expect("cache capacity must be > 0"),
            )),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    pub fn put(&self, key: K, value: V) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .put(key, value);
    }
}
