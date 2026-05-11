use std::{
    collections::HashMap,
    io::{self, Result},
};

use resp::Frame;
use tokio::{net::TcpStream, select, spawn, sync::mpsc, task::JoinHandle};

use crate::{
    command::{Command, PubSubCommand},
    connection::Connection,
    pub_sub::PubSubHandle,
    store::StoreHandle,
};

macro_rules! client_err {
    ($e:expr) => {
        io::Error::new(io::ErrorKind::Other, format!("client error: {}", $e))
    };
}

enum RedisMode {
    Normal,
    Subscription,
}

pub async fn handle_client(
    socket: TcpStream,
    store: StoreHandle,
    pubsub: PubSubHandle,
) -> Result<()> {
    let mut conn = Connection::new(socket);
    let mut mode = RedisMode::Normal;
    // viewer tasks owned by this handler: channel name -> task that forwards broadcast msgs
    let mut subs: HashMap<Vec<u8>, JoinHandle<()>> = HashMap::new();
    // viewer tasks send received messages here; handler forwards them to the client
    let (subscription_tx, mut subscription_rx) = mpsc::channel::<Frame>(32);

    loop {
        let frame = select! {
            // read next command from client
            res = conn.read_frame() => {
                res.map_err(|e| client_err!(e))?
            }
            // forward messages pushed by pub/sub viewer tasks to the client
            Some(frame) = subscription_rx.recv() => {
                conn.write_frames(&[frame]).await.map_err(|e| client_err!(e))?;
                continue;
            }
        };

        let cmd = match Command::parse(frame) {
            Ok(cmd) => cmd,
            Err(msg) => {
                conn.write_frames(&[Frame::Error(msg)])
                    .await
                    .map_err(|e| client_err!(e))?;
                continue;
            }
        };

        // in subscription mode only pub/sub commands and PING are allowed
        if matches!(mode, RedisMode::Subscription)
            && !matches!(
                cmd,
                Command::PubSub(_) | Command::Store(crate::command::StoreCommand::Ping(_))
            )
        {
            conn.write_frames(&[Frame::Error(
                "ERR Command not allowed in subscription mode".to_string(),
            )])
            .await
            .map_err(|e| client_err!(e))?;
            continue;
        }

        match cmd {
            Command::PubSub(PubSubCommand::Subscribe(channels)) => {
                // get a broadcast receiver for each channel from the pubsub actor
                let receivers = pubsub.subscribe(channels.clone()).await;
                let mut frames = Vec::new();

                for (ch, receiver) in channels.iter().zip(receivers) {
                    // spawn a viewer task that forwards broadcast messages into subscription_rx
                    let tx = subscription_tx.clone();
                    let handle = spawn(async move {
                        let mut receiver = receiver;
                        while let Ok(msg) = receiver.recv().await {
                            // if the handler has disconnected, stop the viewer task
                            if tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                    });
                    subs.insert(ch.clone(), handle);

                    // sub count reflects all channels after this insert
                    frames.push(Frame::Array(vec![
                        Frame::BulkString(b"subscribe".to_vec()),
                        Frame::BulkString(ch.clone()),
                        Frame::Integer(subs.len() as i64),
                    ]));
                }

                mode = RedisMode::Subscription;
                conn.write_frames(&frames)
                    .await
                    .map_err(|e| client_err!(e))?;
            }
            Command::PubSub(PubSubCommand::Unsubscribe(channels)) => {
                // if no channels specified, unsubscribe from all
                let to_unsub: Vec<_> = match channels {
                    Some(chs) => chs,
                    None => subs.keys().cloned().collect(),
                };

                // abort viewer tasks and remove from local sub map
                for ch in &to_unsub {
                    if let Some(handle) = subs.remove(ch) {
                        handle.abort();
                    }
                }

                // tell pubsub actor to remove broadcast channels that now have no receivers
                pubsub.cleanup_channels(to_unsub.clone()).await;

                let mut frames = Vec::new();
                for ch in &to_unsub {
                    frames.push(Frame::Array(vec![
                        Frame::BulkString(b"unsubscribe".to_vec()),
                        Frame::BulkString(ch.clone()),
                        Frame::Integer(subs.len() as i64),
                    ]));
                }

                if subs.is_empty() {
                    mode = RedisMode::Normal;
                }

                conn.write_frames(&frames)
                    .await
                    .map_err(|e| client_err!(e))?;
            }
            Command::PubSub(PubSubCommand::Publish { channel, message }) => {
                // returns number of clients that received the message
                let count = pubsub.publish(channel, message).await;
                conn.write_frames(&[Frame::Integer(count)])
                    .await
                    .map_err(|e| client_err!(e))?;
            }
            Command::Store(store_cmd) => {
                // send command to store actor and await the response
                let frame = store.execute(store_cmd).await;
                conn.write_frames(&[frame])
                    .await
                    .map_err(|e| client_err!(e))?;
            }
        }
    }
}
