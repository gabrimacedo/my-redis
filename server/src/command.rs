use std::collections::VecDeque;
use std::{collections::hash_map::Entry, time::Duration};

use crate::{
    Frame,
    store::{DataType, StoreMap, StoredEntry},
};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub enum Command {
    Subscribe(Vec<Vec<u8>>),
    Unsubscribe(Option<Vec<Vec<u8>>>),
    Publish {
        channel: Vec<u8>,
        message: Vec<u8>,
    },
    Ping(Option<Vec<u8>>),
    Echo(Vec<u8>),
    Set {
        key: Vec<u8>,
        value: Vec<u8>,
        expires_at: Option<Instant>,
    },
    Get {
        key: Vec<u8>,
    },
    Del {
        keys: Vec<Vec<u8>>,
    },
    Exists {
        keys: Vec<Vec<u8>>,
    },
    Ttl(Vec<u8>),
    LPush {
        key: Vec<u8>,
        items: Vec<Vec<u8>>,
    },
    RPush {
        key: Vec<u8>,
        items: Vec<Vec<u8>>,
    },
    LPop(Vec<u8>),
    RPop(Vec<u8>),
    LRange {
        key: Vec<u8>,
        start: i64,
        stop: i64,
    },
    LLen(Vec<u8>),
}

impl Command {
    pub fn parse(f: Frame) -> Result<Command, String> {
        let Frame::Array(frames) = f else {
            return Err("invalid frame".to_owned());
        };

        let args: Result<Vec<_>, _> = frames
            .into_iter()
            .map(|f| match f {
                Frame::BulkString(s) => Ok(s),
                _ => Err("invalid frame"),
            })
            .collect();

        let mut args = args?;
        let cmd = String::from_utf8(args.remove(0))
            .map_err(|_| "ERR invalid command".to_string())?
            .to_uppercase();

        match cmd.as_str() {
            "PING" => {
                if args.is_empty() {
                    return Ok(Command::Ping(None));
                }
                Ok(Command::Ping(Some(args.swap_remove(0))))
            }
            "ECHO" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'ECHO' command".to_string());
                }
                Ok(Command::Echo(args.swap_remove(0)))
            }
            "SET" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'SET' command".to_string());
                }
                Ok(Command::Set {
                    key: args.remove(0),
                    value: args.remove(0),
                    expires_at: Self::parse_expiry(args)?,
                })
            }
            "GET" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'GET' command".to_string());
                }
                Ok(Command::Get {
                    key: args.swap_remove(0),
                })
            }
            "DEL" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'DEL' command".to_string());
                }
                Ok(Command::Del { keys: args })
            }
            "EXISTS" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'EXISTS' command".to_string());
                }
                Ok(Command::Exists { keys: args })
            }
            "TTL" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'TTL' command".to_string());
                }
                Ok(Command::Ttl(args.swap_remove(0)))
            }
            "LPUSH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::LPush {
                    key: args.remove(0),
                    items: args[0..].to_vec(),
                })
            }
            "RPUSH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'RPUSH' command".to_string());
                };
                Ok(Command::RPush {
                    key: args.remove(0),
                    items: args[0..].to_vec(),
                })
            }
            "LLEN" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LLEN' command".to_string());
                };
                Ok(Command::LLen(args.swap_remove(0)))
            }
            "LPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPOP' command".to_string());
                };
                Ok(Command::LPop(args.swap_remove(0)))
            }
            "RPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'RPOP' command".to_string());
                };
                Ok(Command::RPop(args.swap_remove(0)))
            }
            "LRANGE" => {
                if args.len() < 3 {
                    return Err("ERR wrong number of arguments for 'LRANGE' command".to_string());
                };
                let start =
                    std::str::from_utf8(args[1].as_slice()).map_err(|_| "Invalid argument")?;
                let start: i64 = start.parse().map_err(|_| "Invalid argument")?;

                let stop =
                    std::str::from_utf8(args[2].as_slice()).map_err(|_| "Invalid argument")?;
                let stop: i64 = stop.parse().map_err(|_| "Invalid argument")?;

                Ok(Command::LRange {
                    key: args.swap_remove(0),
                    start,
                    stop,
                })
            }
            "SUBSCRIBE" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'SUBSCRIBE' command".to_string());
                };
                Ok(Command::Subscribe(args))
            }
            "UNSUBSCRIBE" => {
                if args.is_empty() {
                    return Ok(Command::Unsubscribe(None));
                };
                Ok(Command::Unsubscribe(Some(args)))
            }
            "PUBLISH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'PUBLISH' command".to_string());
                };
                Ok(Command::Publish {
                    channel: args.remove(0),
                    message: args.remove(0),
                })
            }
            _ => Err(format!("ERR unknown command '{}'", cmd)),
        }
    }

    fn parse_expiry(mut opts: Vec<Vec<u8>>) -> Result<Option<Instant>, String> {
        enum Opt {
            Ex(Instant),
            Px(Instant),
            None,
        }
        let mut exp = Opt::None;
        opts.iter_mut().for_each(|b| b.make_ascii_uppercase());
        let mut iter = opts.chunks_exact(2);

        for s in iter.by_ref() {
            match s[0].as_slice() {
                b"EX" => {
                    if matches!(exp, Opt::Px(_)) {
                        return Err(
                            "ERR EX and PX options at the same time are not compatible".to_string()
                        );
                    }
                    let n = String::from_utf8(s[1].clone())
                        .map_err(|_| "Err: Invalid UTF-8".to_string())?
                        .parse()
                        .map_err(|_| "Err: Not an integer or out of range".to_string())?;

                    exp = Opt::Ex(Instant::now() + Duration::from_secs(n));
                }
                b"PX" => {
                    if matches!(exp, Opt::Ex(_)) {
                        return Err(
                            "ERR PX and EX options at the same time are not compatible".to_string()
                        );
                    }
                    let n = String::from_utf8(s[1].clone())
                        .map_err(|_| "Err: Invalid UTF-8".to_string())?
                        .parse()
                        .map_err(|_| "Err: Not an integer or out of range".to_string())?;

                    exp = Opt::Px(Instant::now() + Duration::from_millis(n));
                }
                _ => return Err("ERR syntax error".to_string()),
            }
        }

        if !iter.remainder().is_empty() {
            return Err("ERR syntax error".to_string());
        };

        match exp {
            Opt::Ex(i) | Opt::Px(i) => Ok(Some(i)),
            Opt::None => Ok(None),
        }
    }

    pub fn execute(self, map: &mut StoreMap) -> Frame {
        match self {
            Command::Ping(arg) => {
                if let Some(arg) = arg {
                    return Frame::BulkString(arg);
                }
                Frame::SimpleString("PONG".to_string())
            }
            Command::Echo(arg) => Frame::BulkString(arg),
            Command::Set {
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
            Command::Get { key } => {
                let Some(value) = map.get_mut(&key) else {
                    return Frame::Null;
                };
                let DataType::String(s) = &value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::BulkString(s.clone())
            }
            Command::Del { keys } => {
                let mut count = 0;
                for k in keys {
                    if map.remove(&k).is_some() {
                        count += 1;
                    };
                }
                Frame::Integer(count)
            }
            Command::Exists { keys } => {
                let mut count = 0;
                for k in keys {
                    if map.contains_key(&k) {
                        count += 1;
                    }
                }
                Frame::Integer(count)
            }
            Command::Ttl(key) => {
                let Some(value) = map.get(&key) else {
                    return Frame::Integer(-2);
                };

                let Some(exp) = value.expires_at else {
                    return Frame::Integer(-1);
                };

                Frame::Integer((exp - Instant::now()).as_secs() as i64)
            }
            Command::LPush { key, items } => {
                map.lazy_delete(&key);
                match map.data.entry(key) {
                    Entry::Occupied(mut e) => {
                        let DataType::List(l) = &mut e.get_mut().data else {
                            return Frame::Error("WRONGTYPE error".to_string());
                        };
                        for item in items {
                            l.push_front(item);
                        }
                        Frame::Integer(l.len() as i64)
                    }
                    Entry::Vacant(e) => {
                        let added = items.len();
                        let new_list: VecDeque<_> = items.into_iter().rev().collect();
                        e.insert(StoredEntry::new(DataType::List(new_list)));
                        Frame::Integer(added as i64)
                    }
                }
            }
            Command::RPush { key, items } => {
                map.lazy_delete(&key);
                match map.data.entry(key) {
                    Entry::Occupied(mut e) => {
                        let DataType::List(l) = &mut e.get_mut().data else {
                            return Frame::Error("WRONGTYPE error".to_string());
                        };
                        for item in items {
                            l.push_back(item);
                        }
                        Frame::Integer(l.len() as i64)
                    }
                    Entry::Vacant(e) => {
                        let added = items.len();
                        let new_list: VecDeque<_> = items.into_iter().collect();
                        e.insert(StoredEntry::new(DataType::List(new_list)));
                        Frame::Integer(added as i64)
                    }
                }
            }
            Command::LPop(key) => {
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
            Command::RPop(key) => {
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
            Command::LRange {
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
            Command::LLen(key) => {
                let Some(value) = map.get(&key) else {
                    return Frame::Integer(0);
                };
                let DataType::List(list) = &value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::Integer(list.len() as i64)
            }

            // sub/pub commands are handled in the task store
            // before we even get here
            Command::Subscribe(_)
            | Command::Unsubscribe(_)
            | Command::Publish {
                channel: _,
                message: _,
            } => unreachable!(),
        }
    }
}
