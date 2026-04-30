use resp::Frame;
use std::{collections::HashMap, time::Duration};
use tokio::{
    net::TcpListener,
    select, spawn,
    sync::{
        broadcast,
        mpsc::{self},
        oneshot,
    },
};

mod command;
mod connection;
mod handler;
mod store;

use crate::{handler::handle_client, store::StoreMap};
use command::Command;

struct CommandRequest {
    cmd: Command,
    response_sender: oneshot::Sender<Vec<Frame>>,
    handler_id: tokio::task::Id,
    subscription_sender: Option<mpsc::Sender<Frame>>,
}

type ChannelMap = HashMap<Vec<u8>, broadcast::Sender<Frame>>;
type ClientMap = HashMap<tokio::task::Id, Client>;

enum RedisMode {
    Subscription,
    Normal,
}

struct Client {
    mode: RedisMode,
    subs: HashMap<Vec<u8>, tokio::task::JoinHandle<()>>,
}

pub async fn start_server(listener: TcpListener) {
    let mut store_map = StoreMap::new();
    let mut channel_map: ChannelMap = HashMap::new();
    let mut client_map: ClientMap = HashMap::new();

    let (tx, mut rx) = mpsc::channel::<CommandRequest>(10);
    let (registration_tx, mut registration_rx) = mpsc::channel::<tokio::task::Id>(10);
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    spawn(async move {
        loop {
            select! {
                Some(id) = registration_rx.recv() => {
                    client_map.insert(id, Client {
                        mode: RedisMode::Normal,
                        subs: HashMap::new(),
                    });
                }
                _ = interval.tick() => {
                    store_map.sweep_expired();
                }
                Some(req) = rx.recv() => {
                    let CommandRequest { cmd, response_sender, handler_id, subscription_sender } = req;
                    match cmd {
                        Command::Publish { channel, message } => {
                            // if ch does not exist send 0 and exit early
                            let Some(ch) = channel_map.get(&channel) else {
                                if response_sender.send(vec![Frame::Integer(0)]).is_err() {
                                    eprintln!("the receiver dropped");
                                };
                                continue;
                            };

                            // publish message
                            let f = Frame::Array(vec![
                                Frame::BulkString(b"message".to_vec()),
                                Frame::BulkString(channel.clone()),
                                Frame::BulkString(message.clone()),
                            ]);
                            let viewers = ch
                                .send(f)
                                .unwrap_or(0);

                            // return msg to handler
                            if response_sender
                                .send(vec![Frame::Integer(viewers as i64)])
                                .is_err()
                            {
                                eprintln!("the receiver dropped");
                            };
                        }
                        Command::Unsubscribe(channels) => {
                            let mut frame_buffer = Vec::new();

                            // if theres no channels argument, unsub from all
                            let client = client_map.get_mut(&handler_id).unwrap();
                            let to_unsub: Vec<_> = match channels {
                                Some(channels) => channels.to_vec(),
                                None => client.subs.keys().cloned().collect(),
                            };

                            for ch in to_unsub {
                                // delete viewer task
                                client.subs.get_mut(&ch).unwrap().abort();

                                let cl_sub_count = client.subs.len();
                                // exit sub mode if not longer subbed to any channels
                                if cl_sub_count == 0 {
                                    client.mode = RedisMode::Normal;
                                }

                                // construct response
                                frame_buffer.push(Frame::Array(vec![
                                    Frame::BulkString(b"unsubscribe".to_vec()),
                                    Frame::BulkString(ch.clone()),
                                    Frame::Integer(cl_sub_count as i64),
                                ]));

                                // if the channel has no subscribers left, remove it
                                if channel_map.get_mut(&ch).unwrap().receiver_count() == 0 {
                                    channel_map.remove(&ch);
                                };
                            }

                            if response_sender.send(frame_buffer).is_err() {
                                eprintln!("the receiver dropped");
                            };
                        }
                        Command::Subscribe(channels) => {
                            let mut frame_buffer = Vec::new();
                            for ch in channels {
                                // subscribe to broadcast
                                let broadcaster = channel_map
                                    .entry(ch.clone())
                                    .or_insert_with(|| broadcast::channel(16).0);

                                // spawn viewer task
                                let mut receiver = broadcaster.subscribe();
                                let sender = subscription_sender.clone().unwrap();
                                let handler = spawn(async move {
                                    while let Ok(msg) = receiver.recv().await {
                                        // send published msg back to handler
                                        sender
                                            .send(msg)
                                            .await
                                            .unwrap();
                                    }
                                });

                                // update client list of subscribers
                                let client = client_map.get_mut(&handler_id).unwrap();
                                client.mode = RedisMode::Subscription;
                                client.subs.insert(ch.clone(), handler);

                                // append frame to response
                                frame_buffer.push(Frame::Array(vec![
                                    Frame::BulkString(b"subscribe".to_vec()),
                                    Frame::BulkString(ch.clone()),
                                    Frame::Integer(client.subs.len() as i64),
                                ]));
                            }
                            if response_sender.send(frame_buffer).is_err() {
                                eprintln!("the receiver dropped");
                            };
                        }
                        other => {
                            if response_sender
                                .send(vec![other.execute(&mut store_map)])
                                .is_err()
                            {
                                eprintln!("the receiver dropped");
                            };
                        }
                    } // end match cmd
                } // end select rx arm
            } // end select!
        }
    });

    // handle connections
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let id = spawn(handle_client(socket, tx.clone())).id();
        registration_tx.send(id).await.unwrap();
    }
}
