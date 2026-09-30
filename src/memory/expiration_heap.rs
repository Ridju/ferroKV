use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Eq, PartialEq, Serialize, Deserialize)]
pub struct ExpiryItem {
    pub expires_at: DateTime<Utc>,
    pub key: Vec<u8>,
}

impl Ord for ExpiryItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other.expires_at.cmp(&self.expires_at)
    }
}

impl PartialOrd for ExpiryItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ExpiryItem {
    pub fn new(expires_at: DateTime<Utc>, key: Vec<u8>) -> Self {
        Self { expires_at, key }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BinaryHeap;

    use super::*;

    #[test]
    fn test_ordering() {
        let item_one = ExpiryItem::new(
            Utc::now() + std::time::Duration::from_secs(5),
            b"five".to_vec(),
        );
        let item_two = ExpiryItem::new(
            Utc::now() + std::time::Duration::from_secs(10),
            b"ten".to_vec(),
        );
        let item_three = ExpiryItem::new(
            Utc::now() + std::time::Duration::from_secs(2),
            b"two".to_vec(),
        );

        let mut heap = BinaryHeap::<ExpiryItem>::new();
        heap.push(item_one);
        heap.push(item_two);
        heap.push(item_three);

        assert_eq!(heap.pop().unwrap().key, b"two".to_vec());
        assert_eq!(heap.pop().unwrap().key, b"five".to_vec());
        assert_eq!(heap.pop().unwrap().key, b"ten".to_vec());
    }
}
