use std::collections::HashMap;

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

pub enum Command {
    Ping(Option<String>),
    Echo(String),
    Set { key: String, value: String },
    Get { key: String },
    Del { keys: Vec<String> },
    Exists { keys: Vec<String> },
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
            _ => Err(format!("ERR unknown command '{}'", cmd)),
        }
    }

    pub fn execute(self, map: &mut HashMap<String, String>) -> Frame {
        match self {
            Command::Ping(arg) => {
                if let Some(arg) = arg {
                    Frame::BulkString(arg.as_bytes().to_vec())
                } else {
                    Frame::SimpleString("PONG".to_string())
                }
            }
            Command::Echo(arg) => Frame::BulkString(arg.as_bytes().to_vec()),
            Command::Set { key, value } => {
                map.insert(key, value);
                Frame::SimpleString("OK".to_string())
            }
            Command::Get { key } => match map.get(&key) {
                Some(value) => Frame::BulkString(value.as_bytes().to_vec()),
                None => Frame::Null,
            },
            Command::Del { keys } => {
                let count = keys.iter().fold(0, |acc, k| {
                    if map.remove(k).is_none() {
                        acc
                    } else {
                        acc + 1
                    }
                });
                Frame::Integer(count)
            }
            Command::Exists { keys } => {
                let count = keys
                    .iter()
                    .fold(0, |acc, k| if map.get(k).is_none() { acc } else { acc + 1 });
                Frame::Integer(count)
            }
        }
    }
}

pub fn run() {}

type CommandRequest = (Command, oneshot::Sender<Frame>);

pub async fn start_server(listener: TcpListener) {
    let mut map: HashMap<String, String> = HashMap::new();
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

async fn handle_client(mut socket: TcpStream, tx: Sender<CommandRequest>) {
    loop {
        // read
        let mut buf = [0u8; 1024];
        let nr = socket.read(&mut buf).await.unwrap();
        let req = buf[..nr].to_vec();

        // decode frame TODO: handle imcomplete frame
        let (in_frame, _) = Frame::decode(&req);

        match Command::parse(in_frame) {
            Ok(cmd) => {
                let (resp_tx, resp_rx) = oneshot::channel();
                tx.send((cmd, resp_tx)).await.unwrap();
                let resp = resp_rx.await.unwrap();
                let _ = socket.write(&resp.encode()).await.unwrap();
            }
            Err(msg) => {
                let _ = socket.write(&Frame::Error(msg).encode()).await.unwrap();
            }
        };
    }
}
