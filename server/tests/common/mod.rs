use resp::Frame;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    spawn,
};

pub async fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("localhost:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    spawn(server::start_server(listener));

    addr
}

pub async fn connect_to_server(addr: SocketAddr) -> TcpStream {
    TcpStream::connect(addr)
        .await
        .expect("could not connect to server")
}

pub async fn send_cmd(conn: &mut TcpStream, cmd: &[u8]) -> Frame {
    let _ = conn.write(cmd).await.unwrap();
    let mut resp = [0u8; 1024];
    let n = conn.read(&mut resp).await.unwrap();

    let (frame, _) = Frame::decode(&resp[..n]);
    frame
}

pub async fn send_multiple_cmds(conn: &mut TcpStream, cmds: Vec<Vec<u8>>) -> Vec<Frame> {
    let bundled_cmds = cmds.into_iter().flatten().collect::<Vec<u8>>();
    let _ = conn.write(&bundled_cmds).await.unwrap();

    let mut response_frames = Vec::new();
    let mut resp = [0u8; 1024];
    let n = conn.read(&mut resp).await.unwrap();
    let mut offset = 0;

    while offset < n {
        let (frame, consumed) = Frame::decode(&resp[offset..n]);
        response_frames.push(frame);
        offset += consumed;
    }

    response_frames
}
