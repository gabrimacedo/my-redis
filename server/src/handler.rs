use std::io::{self, Result};

use resp::Frame;
use tokio::{
    net::TcpStream,
    select,
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};

use crate::{CommandRequest, command::Command, connection::Connection};

macro_rules! client_err {
    ($e:expr) => {
        io::Error::new(io::ErrorKind::Other, format!("client error: {}", $e))
    };
}

pub async fn handle_client(socket: TcpStream, tx: Sender<CommandRequest>) -> Result<()> {
    let mut conn = Connection::new(socket);
    let (subscription_tx, mut subscription_rx) = mpsc::channel::<Frame>(32);

    loop {
        let frame = select! {
            // read frame from socket
            res = conn.read_frame() => {
                res.map_err(|e| client_err!(e))?
            }
            // read from sub channels
            Some(frame) = subscription_rx.recv() => {
                conn.write_frames(&[frame]).await.map_err(|e| client_err!(e))?;
                continue;
            }
        };

        match Command::parse(frame) {
            Ok(cmd) => {
                // send cmd to store task
                // TODO: restrict commands based on client mode
                let (response_tx, response_rx) = oneshot::channel();
                let task_id = tokio::task::id();

                match &cmd {
                    Command::Subscribe(_) => tx
                        .send(CommandRequest {
                            cmd,
                            response_sender: response_tx,
                            handler_id: task_id,
                            subscription_sender: Some(subscription_tx.clone()),
                        })
                        .await
                        .map_err(|e| client_err!(e))?,
                    _ => tx
                        .send(CommandRequest {
                            cmd,
                            response_sender: response_tx,
                            handler_id: task_id,
                            subscription_sender: None,
                        })
                        .await
                        .map_err(|e| client_err!(e))?,
                };

                // read respone frame back from store task
                let frames = response_rx.await.map_err(|e| client_err!(e))?;

                // write to client
                conn.write_frames(&frames)
                    .await
                    .map_err(|e| client_err!(e))?;
            }
            Err(msg) => {
                // write to client
                conn.write_frames(&[Frame::Error(msg)])
                    .await
                    .map_err(|e| client_err!(e))?;
            }
        };
    }
}
