use std::collections::{HashMap, VecDeque};
use tokio::time::Instant;

#[derive(Debug)]
pub enum DataType {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
}

#[derive(Debug)]
pub struct StoredEntry {
    pub data: DataType,
    pub expires_at: Option<Instant>,
}

impl StoredEntry {
    pub fn new(data: DataType) -> Self {
        Self {
            data,
            expires_at: None,
        }
    }
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|t| t < Instant::now())
    }
}

pub struct StoreMap {
    pub data: HashMap<Vec<u8>, StoredEntry>,
}

impl StoreMap {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    // TODO: add doc string explaning lazy expiration
    pub fn insert(&mut self, key: Vec<u8>, value: StoredEntry) -> Option<StoredEntry> {
        self.data.insert(key, value)
    }

    pub fn contains_key(&mut self, key: &[u8]) -> bool {
        self.lazy_delete(key);
        self.data.contains_key(key)
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&StoredEntry> {
        self.lazy_delete(key);
        self.data.get(key)
    }

    pub fn get_mut(&mut self, key: &[u8]) -> Option<&mut StoredEntry> {
        self.lazy_delete(key);
        self.data.get_mut(key)
    }

    pub fn remove(&mut self, key: &[u8]) -> Option<StoredEntry> {
        self.lazy_delete(key);
        self.data.remove(key)
    }

    pub fn lazy_delete(&mut self, key: &[u8]) {
        if let Some(entry) = self.data.get(key)
            && entry.is_expired()
        {
            self.data.remove(key);
        }
    }

    pub fn sweep_expired(&mut self) -> usize {
        let before = self.data.len();

        let keys_to_remove: Vec<_> = self
            .data
            .iter()
            .take(20)
            .filter(|(_k, v)| v.is_expired())
            .map(|(k, _v)| k.clone())
            .collect();

        for key in keys_to_remove {
            self.data.remove(&key);
        }

        before - self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expire_sweep_removes_keys_without_access() {
        let mut expired_map = StoreMap::new();

        for i in 0..20 {
            expired_map.insert(
                i.to_string().into_bytes(),
                StoredEntry {
                    data: DataType::String(b"data".to_vec()),
                    expires_at: Some(Instant::now() - std::time::Duration::from_secs(3600)),
                },
            );
        }

        let removed = expired_map.sweep_expired();
        assert_eq!(removed, 20);
    }

    #[test]
    fn expire_sweep_capped_at_20_removals() {
        let mut map = StoreMap::new();

        for i in 0..40 {
            map.insert(
                i.to_string().into_bytes(),
                StoredEntry {
                    data: DataType::String(b"data".to_vec()),
                    expires_at: Some(Instant::now() - std::time::Duration::from_secs(3600)),
                },
            );
        }

        let removed = map.sweep_expired();
        assert!(removed <= 20);
    }
}
