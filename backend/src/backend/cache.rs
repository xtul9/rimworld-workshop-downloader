use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Simple in-memory cache with TTL (Time To Live)
pub struct Cache<T> {
    cache: HashMap<String, CacheEntry<T>>,
    default_ttl: Duration,
}

struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

impl<T> Cache<T> {
    /// Create a new cache with default TTL
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            default_ttl,
        }
    }

    /// Set a value in the cache with optional custom TTL
    pub fn set(&mut self, key: String, data: T, ttl: Option<Duration>) {
        let expires_at = Instant::now() + ttl.unwrap_or(self.default_ttl);
        self.cache.insert(key, CacheEntry { data, expires_at });
    }

    /// Get a value from the cache, returning None if expired or not found
    pub fn get(&mut self, key: &str) -> Option<&T> {
        let is_expired = self.cache.get(key)
            .map(|entry| Instant::now() > entry.expires_at)
            .unwrap_or(true);
        
        if is_expired {
            self.cache.remove(key);
            return None;
        }
        
        self.cache.get(key).map(|entry| &entry.data)
    }

    /// Check if a key exists and is not expired
    pub fn has(&mut self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Clean up expired entries
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        self.cache.retain(|_, entry| entry.expires_at > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_and_get() {
        let mut cache = Cache::new(Duration::from_secs(60));
        cache.set("key1".to_string(), "value1", None);
        
        assert_eq!(cache.get("key1"), Some(&"value1"));
    }

    #[test]
    fn test_cache_expiration() {
        let mut cache = Cache::new(Duration::from_millis(100));
        cache.set("key1".to_string(), "value1", None);
        
        assert_eq!(cache.get("key1"), Some(&"value1"));
        
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(cache.get("key1"), None);
    }

    #[test]
    fn test_cache_custom_ttl() {
        let mut cache = Cache::new(Duration::from_secs(60));
        cache.set("key1".to_string(), "value1", Some(Duration::from_millis(100)));
        
        assert_eq!(cache.get("key1"), Some(&"value1"));
        
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(cache.get("key1"), None);
    }

    #[test]
    fn test_cache_cleanup() {
        let mut cache = Cache::new(Duration::from_millis(100));
        cache.set("key1".to_string(), "value1", None);
        cache.set("key2".to_string(), "value2", None);
        
        std::thread::sleep(Duration::from_millis(150));
        cache.cleanup();
        
        assert_eq!(cache.get("key1"), None);
        assert_eq!(cache.get("key2"), None);
    }
}

