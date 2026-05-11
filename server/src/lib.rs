use tokio::{net::TcpListener, spawn};

mod command;
mod connection;
mod handler;
mod pub_sub;
mod store;

use crate::{handler::handle_client, pub_sub::PubSubHandle, store::StoreHandle};

pub async fn start_server(listener: TcpListener) {
    // spawn both actors; handles are clonable and shared across handler tasks
    let store = StoreHandle::new();
    let pubsub = PubSubHandle::new();

    // accept connections and spawn a handler task per client
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        spawn(handle_client(socket, store.clone(), pubsub.clone()));
    }
}
