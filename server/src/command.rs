use std::time::Duration;

use resp::Frame;
use tokio::time::Instant;

/// Top-level parsed command. Wraps either a store command or a pubsub command
/// so the handler can route to the right actor without ever seeing an
/// "impossible" variant in the inner match.
#[derive(Debug, Clone)]
pub enum Command {
    Store(StoreCommand),
    PubSub(PubSubCommand),
}

#[derive(Debug, Clone)]
pub enum StoreCommand {
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

#[derive(Debug, Clone)]
pub enum PubSubCommand {
    Subscribe(Vec<Vec<u8>>),
    Unsubscribe(Option<Vec<Vec<u8>>>),
    Publish { channel: Vec<u8>, message: Vec<u8> },
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
                    return Ok(Command::Store(StoreCommand::Ping(None)));
                }
                Ok(Command::Store(StoreCommand::Ping(Some(
                    args.swap_remove(0),
                ))))
            }
            "ECHO" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'ECHO' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Echo(args.swap_remove(0))))
            }
            "SET" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'SET' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Set {
                    key: args.remove(0),
                    value: args.remove(0),
                    expires_at: Self::parse_expiry(args)?,
                }))
            }
            "GET" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'GET' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Get {
                    key: args.swap_remove(0),
                }))
            }
            "DEL" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'DEL' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Del { keys: args }))
            }
            "EXISTS" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'EXISTS' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Exists { keys: args }))
            }
            "TTL" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'TTL' command".to_string());
                }
                Ok(Command::Store(StoreCommand::Ttl(args.swap_remove(0))))
            }
            "LPUSH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'LPUSH' command".to_string());
                };
                Ok(Command::Store(StoreCommand::LPush {
                    key: args.remove(0),
                    items: args[0..].to_vec(),
                }))
            }
            "RPUSH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'RPUSH' command".to_string());
                };
                Ok(Command::Store(StoreCommand::RPush {
                    key: args.remove(0),
                    items: args[0..].to_vec(),
                }))
            }
            "LLEN" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LLEN' command".to_string());
                };
                Ok(Command::Store(StoreCommand::LLen(args.swap_remove(0))))
            }
            "LPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'LPOP' command".to_string());
                };
                Ok(Command::Store(StoreCommand::LPop(args.swap_remove(0))))
            }
            "RPOP" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'RPOP' command".to_string());
                };
                Ok(Command::Store(StoreCommand::RPop(args.swap_remove(0))))
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

                Ok(Command::Store(StoreCommand::LRange {
                    key: args.swap_remove(0),
                    start,
                    stop,
                }))
            }
            "SUBSCRIBE" => {
                if args.is_empty() {
                    return Err("ERR wrong number of arguments for 'SUBSCRIBE' command".to_string());
                };
                Ok(Command::PubSub(PubSubCommand::Subscribe(args)))
            }
            "UNSUBSCRIBE" => {
                if args.is_empty() {
                    return Ok(Command::PubSub(PubSubCommand::Unsubscribe(None)));
                };
                Ok(Command::PubSub(PubSubCommand::Unsubscribe(Some(args))))
            }
            "PUBLISH" => {
                if args.len() < 2 {
                    return Err("ERR wrong number of arguments for 'PUBLISH' command".to_string());
                };
                Ok(Command::PubSub(PubSubCommand::Publish {
                    channel: args.remove(0),
                    message: args.remove(0),
                }))
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
}
