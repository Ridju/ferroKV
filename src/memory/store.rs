use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
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
}

struct KvState {
    elements: BTreeMap<Vec<u8>, Entry>,
}

impl KvState {
    pub fn new() -> Self {
        Self {
            elements: BTreeMap::<Vec<u8>, Entry>::new(),
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
    fn test_exipred_item() {
        let store = Store::new();
        store
            .put(b"one".to_vec(), b"one".to_vec(), Some(1))
            .unwrap();
        assert_eq!(store.get(b"one").unwrap(), Some(b"one".to_vec()));
        sleep(Duration::from_millis(1100));
        assert!(!store.exists(b"one").unwrap());
    }
}
