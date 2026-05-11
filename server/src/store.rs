use resp::Frame;
use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};
use tokio::{
    select, spawn,
    sync::{mpsc, oneshot},
    time::Instant,
};

use crate::command::StoreCommand;

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

// message sent to the store actor: a command plus where to send the response
struct StoreMsg {
    cmd: StoreCommand,
    reply: oneshot::Sender<Frame>,
}

/// Handle to the store actor. Cheap to clone — just clones the channel sender.
#[derive(Clone)]
pub struct StoreHandle {
    tx: mpsc::Sender<StoreMsg>,
}

impl StoreHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        spawn(run(rx));
        StoreHandle { tx }
    }

    /// Execute a command against the store and await the response frame.
    pub async fn execute(&self, cmd: StoreCommand) -> Frame {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(StoreMsg {
                cmd,
                reply: reply_tx,
            })
            .await
            .unwrap();
        reply_rx.await.unwrap()
    }
}

async fn run(mut rx: mpsc::Receiver<StoreMsg>) {
    let mut store_map = StoreMap::new();
    let mut sweep_interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        select! {
            _ = sweep_interval.tick() => {
                store_map.sweep_expired();
            }
            Some(msg) = rx.recv() => {
                let frame = msg.cmd.execute(&mut store_map);
                let _ = msg.reply.send(frame);
            }
        }
    }
}

impl StoreCommand {
    pub fn execute(self, map: &mut StoreMap) -> Frame {
        match self {
            StoreCommand::Ping(arg) => {
                if let Some(arg) = arg {
                    return Frame::BulkString(arg);
                }
                Frame::SimpleString("PONG".to_string())
            }
            StoreCommand::Echo(arg) => Frame::BulkString(arg),
            StoreCommand::Set {
                key,
                value,
                expires_at,
            } => {
                map.insert(
                    key,
                    StoredEntry {
                        data: DataType::String(value),
                        expires_at,
                    },
                );
                Frame::SimpleString("OK".to_string())
            }
            StoreCommand::Get { key } => {
                let Some(value) = map.get_mut(&key) else {
                    return Frame::Null;
                };
                let DataType::String(s) = &value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::BulkString(s.clone())
            }
            StoreCommand::Del { keys } => {
                let mut count = 0;
                for k in keys {
                    if map.remove(&k).is_some() {
                        count += 1;
                    };
                }
                Frame::Integer(count)
            }
            StoreCommand::Exists { keys } => {
                let mut count = 0;
                for k in keys {
                    if map.contains_key(&k) {
                        count += 1;
                    }
                }
                Frame::Integer(count)
            }
            StoreCommand::Ttl(key) => {
                let Some(value) = map.get(&key) else {
                    return Frame::Integer(-2);
                };

                let Some(exp) = value.expires_at else {
                    return Frame::Integer(-1);
                };

                Frame::Integer((exp - Instant::now()).as_secs() as i64)
            }
            StoreCommand::LPush { key, items } => {
                map.lazy_delete(&key);
                match map.data.entry(key) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        let DataType::List(l) = &mut e.get_mut().data else {
                            return Frame::Error("WRONGTYPE error".to_string());
                        };
                        for item in items {
                            l.push_front(item);
                        }
                        Frame::Integer(l.len() as i64)
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        let added = items.len();
                        let new_list: VecDeque<_> = items.into_iter().rev().collect();
                        e.insert(StoredEntry::new(DataType::List(new_list)));
                        Frame::Integer(added as i64)
                    }
                }
            }
            StoreCommand::RPush { key, items } => {
                map.lazy_delete(&key);
                match map.data.entry(key) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        let DataType::List(l) = &mut e.get_mut().data else {
                            return Frame::Error("WRONGTYPE error".to_string());
                        };
                        for item in items {
                            l.push_back(item);
                        }
                        Frame::Integer(l.len() as i64)
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        let added = items.len();
                        let new_list: VecDeque<_> = items.into_iter().collect();
                        e.insert(StoredEntry::new(DataType::List(new_list)));
                        Frame::Integer(added as i64)
                    }
                }
            }
            StoreCommand::LPop(key) => {
                let Some(value) = map.get_mut(&key) else {
                    return Frame::Null;
                };
                let DataType::List(list) = &mut value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                let element = list.pop_front().unwrap();
                if list.is_empty() {
                    map.remove(&key);
                }
                Frame::BulkString(element)
            }
            StoreCommand::RPop(key) => {
                let Some(value) = map.get_mut(&key) else {
                    return Frame::Null;
                };
                let DataType::List(list) = &mut value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                let element = list.pop_back().unwrap();
                if list.is_empty() {
                    map.remove(&key);
                }
                Frame::BulkString(element)
            }
            StoreCommand::LRange {
                key,
                mut start,
                mut stop,
            } => {
                let Some(value) = map.get_mut(&key) else {
                    return Frame::Array(vec![]);
                };
                let DataType::List(list) = &mut value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };

                let list_len = list.len() as i64;

                // resolve negative indices
                if start.is_negative() {
                    start += list_len;
                }
                if stop.is_negative() {
                    stop += list_len;
                }

                if start > stop || start > list_len - 1 {
                    return Frame::Array(vec![]);
                }

                stop = stop.clamp(stop, list_len - 1);

                let mut resp = vec![];
                for i in start..=stop {
                    // at this point we know range is valid, so we can unwrap
                    let item = list.get(i as usize).unwrap();
                    resp.push(Frame::BulkString(item.clone()));
                }

                Frame::Array(resp)
            }
            StoreCommand::LLen(key) => {
                let Some(value) = map.get(&key) else {
                    return Frame::Integer(0);
                };
                let DataType::List(list) = &value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::Integer(list.len() as i64)
            }
        }
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
