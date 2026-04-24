use std::{
    collections::{
        HashMap, VecDeque,
        hash_map::{self},
    },
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

    fn insert(&mut self, key: Vec<u8>, value: StoredEntry) -> Option<StoredEntry> {
        self.data.insert(key, value)
    }

    fn entry(&mut self, key: Vec<u8>) -> hash_map::Entry<Vec<u8>, StoredEntry> {
        self.data.entry(key)
    }

    fn get_mut(&mut self, key: &Vec<u8>) -> Option<&mut StoredEntry> {
        self.data.get_mut(key)
    }

    fn remove(&mut self, key: &Vec<u8>) -> Option<StoredEntry> {
        self.data.remove(key)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &StoredEntry)> {
        self.data.iter()
    }

    fn retain(&mut self, f: impl FnMut(&Vec<u8>, &mut StoredEntry) -> bool) {
        self.data.retain(f);
    }

    fn get_mut_or_expire(&mut self, key: Vec<u8>) -> Option<&mut StoredEntry> {
        match self.data.entry(key) {
            hash_map::Entry::Vacant(_) => None,
            hash_map::Entry::Occupied(entry) if entry.get().is_expired() => {
                entry.remove();
                None
            }
            hash_map::Entry::Occupied(entry) => Some(entry.into_mut()),
        }
    }
}

#[derive(Debug)]
pub struct StoredEntry {
    data: DataType,
    expires_at: Option<Instant>,
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
        start: usize,
        stop: usize,
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
            "LLEN" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::LLen(args.swap_remove(0)))
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
                let Some(value) = map.get_mut_or_expire(key) else {
                    return Frame::Null;
                };
                let DataType::String(s) = &value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::BulkString(s.clone())
            }
            Command::Del { keys } => {
                let mut count = 0;
                for key in keys {
                    if let hash_map::Entry::Occupied(entry) = map.entry(key) {
                        let expired = entry.get().is_expired();
                        entry.remove();
                        if !expired {
                            count += 1
                        };
                    }
                }
                Frame::Integer(count)
            }
            Command::Exists { keys } => {
                let count = keys.into_iter().fold(0, |acc, k| {
                    if map.get_mut_or_expire(k).is_some() {
                        acc + 1
                    } else {
                        acc
                    }
                });

                Frame::Integer(count)
            }
            Command::Ttl(key) => {
                let Some(value) = map.get_mut_or_expire(key) else {
                    return Frame::Integer(-2);
                };

                let Some(exp) = value.expires_at else {
                    return Frame::Integer(-1);
                };

                Frame::Integer((exp - Instant::now()).as_secs() as i64)
            }
            Command::LPush { key, items } => {
                match map.get_mut(&key) {
                    Some(value) => {
                        let DataType::List(list) = &mut value.data else {
                            return Frame::Error("WRONGTYPE error".to_string());
                        };

                        items.into_iter().for_each(|item| list.push_back(item));
                    }
                    None => {
                        let len = items.len() as i64;
                        map.insert(
                            key,
                            StoredEntry {
                                data: DataType::List(VecDeque::from(items)),
                                expires_at: None,
                            },
                        );
                        return Frame::Integer(len);
                    }
                };
                Frame::Integer(0)
            }
            Command::RPush { key, items: values } => todo!(),
            Command::LPop(key) => {
                let Some(value) = map.get_mut_or_expire(key) else {
                    return Frame::Integer(0);
                };
                let DataType::List(list) = &mut value.data else {
                    return Frame::Error("WRONGTYPE error".to_string());
                };
                Frame::BulkString(list.pop_front().unwrap())
            }
            Command::RPop(_) => todo!(),
            Command::LRange { key, start, stop } => todo!(),
            Command::LLen(key) => {
                let Some(value) = map.get_mut_or_expire(key) else {
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
                    sweep_expired(&mut map);
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

pub fn sweep_expired(map: &mut Map) -> usize {
    let before = map.len();

    let keys_to_remove: Vec<_> = map
        .iter()
        .take(20)
        .filter(|(_k, v)| v.is_expired())
        .map(|(k, _v)| k.clone())
        .collect();

    for key in keys_to_remove {
        map.remove(&key);
    }

    before - map.len()
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

        let removed = sweep_expired(&mut expired_map);
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

        let removed = sweep_expired(&mut map);
        assert!(removed <= 20);
    }
}
