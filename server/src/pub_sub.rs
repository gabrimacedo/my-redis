use resp::Frame;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc, oneshot};

// channel name -> broadcast sender shared by all subscribers of that channel
type ChannelMap = HashMap<Vec<u8>, broadcast::Sender<Frame>>;

enum PubSubMsg {
    // subscribe to one or more channels; returns one receiver per channel, in order
    Subscribe {
        channels: Vec<Vec<u8>>,
        reply: oneshot::Sender<Vec<broadcast::Receiver<Frame>>>,
    },
    // called after handler aborts its viewer tasks; removes broadcast channels with no receivers
    CleanupChannels {
        channels: Vec<Vec<u8>>,
    },
    // broadcast a message; returns the number of subscribers that received it
    Publish {
        channel: Vec<u8>,
        message: Vec<u8>,
        reply: oneshot::Sender<i64>,
    },
}

/// Handle to the pubsub actor. Cheap to clone — just clones the channel sender.
#[derive(Clone)]
pub struct PubSubHandle {
    tx: mpsc::Sender<PubSubMsg>,
}

impl PubSubHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(run(rx));
        PubSubHandle { tx }
    }

    /// Returns one broadcast::Receiver per channel, in the same order as `channels`.
    /// The handler uses these to spawn its own viewer tasks.
    pub async fn subscribe(&self, channels: Vec<Vec<u8>>) -> Vec<broadcast::Receiver<Frame>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(PubSubMsg::Subscribe {
                channels,
                reply: reply_tx,
            })
            .await
            .unwrap();
        reply_rx.await.unwrap()
    }

    /// Remove channels that have no remaining subscribers from the channel map.
    /// Should be called after the handler has aborted its viewer tasks.
    pub async fn cleanup_channels(&self, channels: Vec<Vec<u8>>) {
        self.tx
            .send(PubSubMsg::CleanupChannels { channels })
            .await
            .unwrap();
    }

    /// Returns the number of subscribers that received the message.
    pub async fn publish(&self, channel: Vec<u8>, message: Vec<u8>) -> i64 {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(PubSubMsg::Publish {
                channel,
                message,
                reply: reply_tx,
            })
            .await
            .unwrap();
        reply_rx.await.unwrap()
    }
}

async fn run(mut rx: mpsc::Receiver<PubSubMsg>) {
    let mut channel_map: ChannelMap = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            PubSubMsg::Subscribe { channels, reply } => {
                let receivers = channels
                    .into_iter()
                    .map(|ch| {
                        // create the broadcast channel on first subscriber
                        let sender = channel_map
                            .entry(ch)
                            .or_insert_with(|| broadcast::channel(16).0);
                        sender.subscribe()
                    })
                    .collect();
                let _ = reply.send(receivers);
            }
            PubSubMsg::CleanupChannels { channels } => {
                for ch in channels {
                    if channel_map
                        .get(&ch)
                        .is_some_and(|sender| sender.receiver_count() == 0)
                    {
                        channel_map.remove(&ch);
                    }
                }
            }
            PubSubMsg::Publish {
                channel,
                message,
                reply,
            } => {
                let count = match channel_map.get(&channel) {
                    // no channel entry means no subscribers
                    None => 0,
                    Some(sender) => {
                        let frame = Frame::Array(vec![
                            Frame::BulkString(b"message".to_vec()),
                            Frame::BulkString(channel.clone()),
                            Frame::BulkString(message),
                        ]);
                        sender.send(frame).unwrap_or(0) as i64
                    }
                };
                let _ = reply.send(count);
            }
        }
    }
}
