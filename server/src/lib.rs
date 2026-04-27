use std::{
    collections::{HashMap, VecDeque, hash_map::Entry},
    io,
    time::{Duration, Instant},
};

use resp::Frame;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select, spawn,
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};

type CommandRequest = (Command, oneshot::Sender<Frame>);

pub struct Map {
    data: HashMap<Vec<u8>, StoredEntry>,
}

impl Map {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    // TODO: add doc string explaning lazy expiration

    fn insert(&mut self, key: Vec<u8>, value: StoredEntry) -> Option<StoredEntry> {
        self.data.insert(key, value)
    }

    fn contains_key(&mut self, key: &[u8]) -> bool {
        self.lazy_delete(key);
        self.data.contains_key(key)
    }

    fn get(&mut self, key: &[u8]) -> Option<&StoredEntry> {
        self.lazy_delete(key);
        self.data.get(key)
    }

    fn get_mut(&mut self, key: &[u8]) -> Option<&mut StoredEntry> {
        self.lazy_delete(key);
        self.data.get_mut(key)
    }

    fn remove(&mut self, key: &[u8]) -> Option<StoredEntry> {
        self.lazy_delete(key);
        self.data.remove(key)
    }

    fn lazy_delete(&mut self, key: &[u8]) {
        if let Some(entry) = self.data.get(key)
            && entry.is_expired()
        {
            self.data.remove(key);
        }
    }

    fn sweep_expired(&mut self) -> usize {
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

#[derive(Debug)]
pub struct StoredEntry {
    data: DataType,
    expires_at: Option<Instant>,
}

impl StoredEntry {
    pub fn new(data: DataType) -> Self {
        Self {
            data,
            expires_at: None,
        }
    }
}

#[derive(Debug)]
pub enum DataType {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
}

impl StoredEntry {
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|t| t < Instant::now())
    }
}

pub enum Command {
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
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::RPush {
                    key: args.remove(0),
                    items: args[0..].to_vec(),
                })
            }
            "LLEN" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::LLen(args.swap_remove(0)))
            }
            "LPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::LPop(args.swap_remove(0)))
            }
            "RPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::RPop(args.swap_remove(0)))
            }
            "LRANGE" => {
                if args.len() < 3 {
                    return Err("ERR wrong number of arguments for 'RANGE' command".to_string());
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

    pub fn execute(self, map: &mut Map) -> Frame {
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

                println!("start = {:?}", start);
                println!("stop = {:?}", stop);

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
        }
    }
}

pub async fn start_server(listener: TcpListener) {
    let mut map = Map::new();
    let (tx, mut rx) = mpsc::channel::<CommandRequest>(10);
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    spawn(async move {
        loop {
            select! {
                // handle commands
                Some(( cmd, resp_tx )) = rx.recv() => {
                    let frame = cmd.execute(&mut map);
                    resp_tx.send(frame).unwrap();
                }
                // sweep expired routine
                _ = interval.tick() => {
                    map.sweep_expired();
                }
            }
        }
    });

    // handle connections
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let tx = tx.clone();
        spawn(handle_client(socket, tx));
    }
}

struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            stream,
        }
    }

    async fn read_frame(&mut self) -> io::Result<Frame> {
        // TODO: investigate better memory efficiency for internal buffer
        loop {
            // read frame from internal buffer
            let (in_frame, consumed) = Frame::decode(&self.buffer[self.cursor..]);

            if matches!(in_frame, Frame::Incomplete) {
                // grow the buffer to read into new space
                let len = self.buffer.len();
                self.buffer.resize(len + 128, 0);

                // read from stream & append to internal buffer
                let n = self.stream.read(&mut self.buffer[len..]).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "client disconnected",
                    ));
                }

                // truncate to used size
                self.buffer.truncate(len + n);
                continue;
            }

            self.cursor += consumed;
            if self.cursor > self.buffer.len() / 2 {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
            }
            return Ok(in_frame);
        }
    }

    async fn write_frame(&mut self, frame: &Frame) -> io::Result<usize> {
        self.stream.write(&frame.encode()).await
    }
}

async fn handle_client(socket: TcpStream, tx: Sender<CommandRequest>) {
    let mut conn = Connection::new(socket);

    loop {
        let frame = match conn.read_frame().await {
            Ok(frame) => frame,
            Err(e) => {
                eprint!("client error: {e}");
                return;
            }
        };

        match Command::parse(frame) {
            Ok(cmd) => {
                let (resp_tx, resp_rx) = oneshot::channel();
                tx.send((cmd, resp_tx)).await.unwrap();
                let out_frame = resp_rx.await.unwrap();
                if let Err(err) = conn.write_frame(&out_frame).await {
                    eprint!("client error {err}");
                    return;
                }
            }
            Err(msg) => {
                if let Err(err) = conn.write_frame(&Frame::Error(msg)).await {
                    eprintln!("client error {err}");
                    return;
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expire_sweep_removes_keys_without_access() {
        let mut expired_map = Map::new();

        for i in 0..20 {
            expired_map.insert(
                i.to_string().into_bytes(),
                StoredEntry {
                    data: DataType::String(b"data".to_vec()),
                    expires_at: Some(Instant::now() - Duration::from_secs(3600)),
                },
            );
        }

        let removed = expired_map.sweep_expired();
        assert_eq!(removed, 20);
    }

    #[test]
    fn expire_sweep_capped_at_20_removals() {
        let mut map = Map::new();

        for i in 0..40 {
            map.insert(
                i.to_string().into_bytes(),
                StoredEntry {
                    data: DataType::String(b"data".to_vec()),
                    expires_at: Some(Instant::now() - Duration::from_secs(3600)),
                },
            );
        }

        let removed = map.sweep_expired();
        assert!(removed <= 20);
    }
}
