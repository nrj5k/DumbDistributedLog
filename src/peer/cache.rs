use dashmap::DashMap;
use std::time::{Duration, Instant};

pub struct CacheEntry<V> {
    pub value: V,
    pub expires_at: Instant,
}

pub struct PeerCache<V: Clone> {
    entries: DashMap<String, CacheEntry<V>>,
    default_ttl: Duration,
    max_entries: usize,
}

impl<V: Clone> PeerCache<V> {
    pub fn new(default_ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            entries: DashMap::new(),
            default_ttl: Duration::from_secs(default_ttl_secs),
            max_entries,
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        if let Some(entry) = self.entries.get(key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.value.clone());
            }
        }
        self.entries.remove(key);
        None
    }

    pub fn set(&self, key: String, value: V, ttl: Option<Duration>) {
        // Evict if at capacity (simple random eviction)
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(&key) {
            let keys: Vec<_> = self
                .entries
                .iter()
                .take(1)
                .map(|r| r.key().clone())
                .collect();
            if let Some(k) = keys.first() {
                self.entries.remove(k);
            }
        }
        let expires_at = Instant::now() + ttl.unwrap_or(self.default_ttl);
        self.entries.insert(key, CacheEntry { value, expires_at });
    }

    pub fn set_with_default_ttl(&self, key: String, value: V) {
        self.set(key, value, None);
    }
    pub fn invalidate(&self, key: &str) -> Option<V> {
        self.entries.remove(key).map(|(_, e)| e.value)
    }

    pub fn clear_expired(&self) -> usize {
        let now = Instant::now();
        let mut count = 0;
        self.entries.retain(|_, entry| {
            if entry.expires_at > now {
                true
            } else {
                count += 1;
                false
            }
        });
        count
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_set_get_returns_value() {
        let cache = PeerCache::<String>::new(60, 10);
        cache.set("k1".into(), "v1".into(), Some(Duration::from_secs(60)));
        assert_eq!(cache.get("k1"), Some("v1".into()));
    }
    #[test]
    fn test_expired_entry_returns_none() {
        let cache = PeerCache::<String>::new(60, 10);
        cache.set("k1".into(), "v1".into(), Some(Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(cache.get("k1"), None);
    }
    #[test]
    fn test_max_entries_eviction() {
        let cache = PeerCache::<String>::new(60, 3);
        cache.set("k1".into(), "v1".into(), None);
        cache.set("k2".into(), "v2".into(), None);
        cache.set("k3".into(), "v3".into(), None);
        assert_eq!(cache.len(), 3);
        cache.set("k4".into(), "v4".into(), None);
        assert_eq!(cache.len(), 3);
        assert!(cache.get("k4").is_some());
    }
}
