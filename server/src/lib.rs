use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};

use resp::Frame;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    spawn,
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};

#[derive(Debug)]
pub struct Entry {
    data: String,
    expires_at: Option<Instant>,
}

pub enum Command {
    Ping(Option<String>),
    Echo(String),
    Set {
        key: String,
        value: String,
        expires_at: Option<Instant>,
    },
    Get {
        key: String,
    },
    Del {
        keys: Vec<String>,
    },
    Exists {
        keys: Vec<String>,
    },
    Ttl(String),
}

impl Command {
    pub fn parse(f: Frame) -> Result<Command, String> {
        let Frame::Array(frames) = f else {
            return Err("invalid frame".to_owned());
        };

        let args: Result<Vec<String>, String> = frames
            .into_iter()
            .map(|f| match f {
                Frame::BulkString(bytes) => {
                    String::from_utf8(bytes).map_err(|_| "invalid frame".to_owned())
                }
                _ => Err("invalid frame".to_owned()),
            })
            .collect();
        let mut args = args?;
        let cmd = args.remove(0).to_uppercase();

        match cmd.as_str() {
            "PING" => {
                if args.is_empty() {
                    return Ok(Command::Ping(None));
                }
                Ok(Command::Ping(Some(args[0].clone())))
            }
            "ECHO" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'ECHO' command".to_string());
                }
                Ok(Command::Echo(args[0].clone()))
            }
            "SET" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'SET' command".to_string());
                }
                Ok(Command::Set {
                    key: args[0].clone(),
                    value: args[1].clone(),
                    expires_at: Self::parse_expiry(&args[2..])?,
                })
            }
            "GET" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'GET' command".to_string());
                }
                Ok(Command::Get {
                    key: args[0].clone(),
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
                    return Err("ERR wrong number of arguments for 'EXISTS' command".to_string());
                }
                Ok(Command::Ttl(args[0].clone()))
            }
            _ => Err(format!("ERR unknown command '{}'", cmd)),
        }
    }

    fn parse_expiry(opts: &[String]) -> Result<Option<Instant>, String> {
        enum Opt {
            Ex(Instant),
            Px(Instant),
            None,
        }
        let mut exp = Opt::None;
        let mut iter = opts.chunks_exact(2);

        for s in iter.by_ref() {
            match s[0].as_str() {
                "EX" => {
                    if matches!(exp, Opt::Px(_)) {
                        return Err(
                            "ERR EX and PX options at the same time are not compatible".to_string()
                        );
                    }
                    let n = s[1]
                        .as_str()
                        .parse::<u64>()
                        .map_err(|_| "Err value is not an integer or out of range".to_string())?;

                    exp = Opt::Ex(Instant::now() + Duration::from_secs(n));
                }
                "PX" => {
                    if matches!(exp, Opt::Ex(_)) {
                        return Err(
                            "ERR PX and EX options at the same time are not compatible".to_string()
                        );
                    }
                    let n = s[1]
                        .as_str()
                        .parse::<u64>()
                        .map_err(|_| "Err value is not an integer or out of range".to_string())?;

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

    pub fn execute(self, map: &mut HashMap<String, Entry>) -> Frame {
        match self {
            Command::Ping(arg) => {
                if let Some(arg) = arg {
                    return Frame::BulkString(arg.as_bytes().to_vec());
                }
                Frame::SimpleString("PONG".to_string())
            }
            Command::Echo(arg) => Frame::BulkString(arg.as_bytes().to_vec()),
            Command::Set {
                key,
                value,
                expires_at,
            } => {
                map.insert(
                    key,
                    Entry {
                        data: value,
                        expires_at,
                    },
                );
                Frame::SimpleString("OK".to_string())
            }
            Command::Get { key } => {
                let Some(value) = Self::get_or_expire(&key, map) else {
                    return Frame::Null;
                };
                Frame::BulkString(value.data.as_bytes().to_vec())
            }
            Command::Del { keys } => {
                let count = keys.iter().fold(0, |acc, k| {
                    let _ = Self::get_or_expire(k, map);
                    if map.remove(k).is_none() {
                        acc
                    } else {
                        acc + 1
                    }
                });
                Frame::Integer(count)
            }
            Command::Exists { keys } => {
                let count = keys.iter().fold(0, |acc, k| {
                    if Self::get_or_expire(k, map)
                        .is_some_and(|e| e.expires_at.is_none_or(|t| t > Instant::now()))
                    {
                        acc + 1
                    } else {
                        acc
                    }
                });

                Frame::Integer(count)
            }
            Command::Ttl(key) => {
                let Some(value) = Self::get_or_expire(&key, map) else {
                    return Frame::Integer(-2);
                };

                let Some(exp) = value.expires_at else {
                    return Frame::Integer(-1);
                };

                Frame::Integer((exp - Instant::now()).as_secs() as i64)
            }
        }
    }

    fn get_or_expire<'a>(key: &str, map: &'a mut HashMap<String, Entry>) -> Option<&'a Entry> {
        if map.get(key)?.expires_at.is_some_and(|t| t < Instant::now()) {
            map.remove(key);
            return None;
        }

        map.get(key)
    }
}

type CommandRequest = (Command, oneshot::Sender<Frame>);

pub async fn start_server(listener: TcpListener) {
    let mut map: HashMap<String, Entry> = HashMap::new();
    let (tx, mut rx) = mpsc::channel::<CommandRequest>(10);

    // handle commands
    spawn(async move {
        loop {
            let (cmd, resp_tx) = rx.recv().await.unwrap();
            let frame = cmd.execute(&mut map);
            resp_tx.send(frame).unwrap();
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

pub fn sweep_expired(map: &mut HashMap<String, Entry>) -> usize {
    let before = map.len();
    map.retain(|_, entry| entry.expires_at.is_none_or(|t| t > Instant::now()));
    before - map.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn sweep_removes_expired_keys_without_access() {
        let mut map: HashMap<String, Entry> = HashMap::new();

        map.insert(
            "expired".to_string(),
            Entry {
                data: "val".to_string(),
                expires_at: Some(Instant::now() + Duration::from_millis(50)),
            },
        );
        map.insert(
            "alive".to_string(),
            Entry {
                data: "val".to_string(),
                expires_at: Some(Instant::now() + Duration::from_secs(100)),
            },
        );
        map.insert(
            "no_expiry".to_string(),
            Entry {
                data: "val".to_string(),
                expires_at: None,
            },
        );

        sleep(Duration::from_millis(100));
        let removed = sweep_expired(&mut map);

        assert_eq!(removed, 1);
        assert!(!map.contains_key("expired"));
        assert!(map.contains_key("alive"));
        assert!(map.contains_key("no_expiry"));
    }
}
