use super::expiration_heap::ExpiryItem;
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::PoisonError;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub enum StoreError {
    LockError(String),
}

impl<T> From<PoisonError<T>> for StoreError {
    fn from(err: PoisonError<T>) -> Self {
        StoreError::LockError(err.to_string())
    }
}

struct Entry {
    value: Vec<u8>,
    expires_at: Option<DateTime<Utc>>,
}

impl Entry {
    pub fn new(value: Vec<u8>, expires_at: Option<DateTime<Utc>>) -> Self {
        Self { value, expires_at }
    }

    pub fn is_expired_by(&self, item: &ExpiryItem) -> bool {
        self.expires_at == Some(item.expires_at)
    }
}

struct KvState {
    elements: BTreeMap<Vec<u8>, Entry>,
    expiration_heap: BinaryHeap<ExpiryItem>,
}

impl KvState {
    pub fn new() -> Self {
        Self {
            elements: BTreeMap::<Vec<u8>, Entry>::new(),
            expiration_heap: BinaryHeap::<ExpiryItem>::new(),
        }
    }
}

pub struct Store {
    state: Arc<RwLock<KvState>>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(KvState::new())),
        }
    }

    pub fn put(
        &self,
        key: Vec<u8>,
        value: Vec<u8>,
        ttl_seconds: Option<u64>,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.write()?;
        let duration = ttl_seconds.map(|sec| Utc::now() + std::time::Duration::from_secs(sec));
        if let Some(expires_at) = duration {
            guard
                .expiration_heap
                .push(ExpiryItem::new(expires_at, key.clone()));
        }
        let entry = Entry::new(value, duration);
        guard.elements.insert(key, entry);
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        let guard = self.state.read()?;
        let result = guard
            .elements
            .get(key)
            .filter(|node| match node.expires_at {
                Some(expires) => expires > Utc::now(),
                None => true,
            })
            .map(|node| node.value.clone());

        Ok(result)
    }

    pub fn delete(&self, key: &[u8]) -> Result<bool, StoreError> {
        let mut guard = self.state.write()?;
        Ok(guard.elements.remove(key).is_some())
    }

    pub fn exists(&self, key: &[u8]) -> Result<bool, StoreError> {
        let guard = self.state.read()?;
        let is_valid = guard
            .elements
            .get(key)
            .is_some_and(|node| match node.expires_at {
                Some(expires) => expires > Utc::now(),
                None => true,
            });

        Ok(is_valid)
    }

    pub fn cleanup_expired(&self) -> Result<usize, StoreError> {
        let mut guard = self.state.write()?;
        let now = Utc::now();
        let mut removed_count = 0;
        while guard
            .expiration_heap
            .peek()
            .is_some_and(|item| item.expires_at <= now)
        {
            let Some(item) = guard.expiration_heap.pop() else {
                continue;
            };

            if guard
                .elements
                .get(&item.key)
                .is_some_and(|e| e.is_expired_by(&item))
            {
                guard.elements.remove(&item.key);
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_store_put() {
        let store = Store::new();

        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(5))
            .unwrap();

        let result = store.get(b"one").unwrap();
        assert_eq!(result, Some(b"one".to_vec()));
    }

    #[test]
    fn test_store_put_override() {
        let store = Store::new();

        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(5))
            .unwrap();

        assert_eq!(store.get(b"one").unwrap(), Some(b"one".to_vec()));

        store
            .put(b"one".to_vec(), b"two".to_vec(), Some(5))
            .unwrap();

        assert_eq!(store.get(b"one").unwrap(), Some(b"two".to_vec()));
    }

    #[test]
    fn test_store_get_ttl_expiration() {
        let store = Store::new();

        store
            .put(b"one".to_vec(), b"two".to_vec(), Some(1))
            .unwrap();

        assert_eq!(store.get(b"one").unwrap(), Some(b"two".to_vec()));
        sleep(Duration::from_millis(1100));
        assert_eq!(store.get(b"one").unwrap(), None);
    }

    #[test]
    fn test_non_existing_key() {
        let store = Store::new();
        assert_eq!(store.get(b"one").unwrap(), None);
    }

    #[test]
    fn test_delete_element() {
        let store = Store::new();
        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(5))
            .unwrap();

        assert_eq!(store.get(b"one").unwrap(), Some(b"one".to_vec()));
        store.delete(b"one").unwrap();
        assert_eq!(store.get(b"one").unwrap(), None);
    }

    #[test]
    fn test_delete_expired_element() {
        let store = Store::new();
        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(1))
            .unwrap();
        assert_eq!(store.get(b"one").unwrap(), Some(b"one".to_vec()));
        sleep(Duration::from_millis(1200));
        assert_eq!(store.get(b"one").unwrap(), None);
        store.delete(b"one").unwrap();
        assert_eq!(store.get(b"one").unwrap(), None);
    }

    #[test]
    fn test_delete_non_existing_item() {
        let store = Store::new();
        assert!(!store.delete(b"one").unwrap())
    }

    #[test]
    fn test_item_exists() {
        let store = Store::new();

        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(5))
            .unwrap();

        assert!(store.exists(b"one").unwrap());
    }

    #[test]
    fn test_item_does_not_exist() {
        let store = Store::new();

        assert!(!store.exists(b"one").unwrap());
    }

    #[test]
    fn test_expired_item() {
        let store = Store::new();
        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(1))
            .unwrap();
        assert_eq!(store.get(b"one").unwrap(), Some(b"one".to_vec()));
        sleep(Duration::from_millis(1100));
        assert!(!store.exists(b"one").unwrap());
    }

    #[test]
    fn test_entry_is_expired_by() {
        let now = Utc::now();
        let later = now + std::time::Duration::from_secs(10);

        let entry = Entry::new(b"value".to_vec(), Some(now));

        let matching_item = ExpiryItem::new(now, b"key".to_vec());
        let mismatching_item = ExpiryItem::new(later, b"key".to_vec());

        assert!(entry.is_expired_by(&matching_item));
        assert!(!entry.is_expired_by(&mismatching_item));
    }

    #[test]
    fn test_cleanup_expired_removes_only_expired_keys() {
        let store = Store::new();

        store
            .put(b"expired_key".to_vec(), b"val1".to_vec(), Some(1))
            .unwrap();
        store
            .put(b"valid_key".to_vec(), b"val2".to_vec(), Some(60))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let removed = store.cleanup_expired().unwrap();

        assert_eq!(removed, 1);
        assert!(!store.exists(b"expired_key").unwrap());
        assert!(store.exists(b"valid_key").unwrap());
    }

    #[test]
    fn test_cleanup_expired_handles_overwritten_keys() {
        let store = Store::new();

        store
            .put(b"key".to_vec(), b"val1".to_vec(), Some(1))
            .unwrap();
        store
            .put(b"key".to_vec(), b"val2".to_vec(), Some(60))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let removed = store.cleanup_expired().unwrap();

        assert_eq!(removed, 0);
        assert!(store.exists(b"key").unwrap());
        assert_eq!(store.get(b"key").unwrap(), Some(b"val2".to_vec()));
    }
}
