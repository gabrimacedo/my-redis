use resp::Frame;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    spawn,
};

async fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("localhost:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    spawn(server::start_server(listener));

    addr
}

async fn connect_to_server(addr: SocketAddr) -> TcpStream {
    TcpStream::connect(addr)
        .await
        .expect("could not connect to server")
}

async fn send_cmd(conn: &mut TcpStream, cmd: &[u8]) -> Frame {
    let _ = conn.write(cmd).await.unwrap();
    let mut resp = [0u8; 1024];
    let n = conn.read(&mut resp).await.unwrap();

    let (frame, _) = Frame::decode(&resp[..n]);
    frame
}

mod integration_test {
    use resp::Frame;

    use crate::{connect_to_server, send_cmd, spawn_server};

    #[tokio::test]
    async fn ping_with_argument_returns_argument() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![
            Frame::BulkString(b"PING".to_vec()),
            Frame::BulkString(b"hello".to_vec()),
        ])
        .encode();
        let expected = Frame::BulkString(b"hello".to_vec());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn get_ping_send_pong() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![Frame::BulkString(b"PING".to_vec())]).encode();
        let expected = Frame::SimpleString("PONG".to_string());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn send_echo_with_argument_returns_argument() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![
            Frame::BulkString(b"ECHO".to_vec()),
            Frame::BulkString(b"hello".to_vec()),
        ])
        .encode();
        let expected = Frame::BulkString(b"hello".to_vec());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn send_echo_with_no_argument_returns_error() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![Frame::BulkString(b"ECHO".to_vec())]).encode();
        let expected = Frame::Error("ERR wrong number of arguments for 'ECHO' command".to_string());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn send_foobar_expect_unknow_cmd_error() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![Frame::BulkString(b"FOOBAR".to_vec())]).encode();
        let expected = Frame::Error("ERR unknown command 'FOOBAR'".to_string());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn send_set_key_expect_ok() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let cmd = Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
            Frame::BulkString(b"hello".to_vec()),
        ])
        .encode();
        let expected = Frame::SimpleString("OK".to_string());

        let resp = send_cmd(&mut conn, &cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn send_set_then_get_expect_same_value() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;

        let set_cmd = Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
            Frame::BulkString(b"secret stuff".to_vec()),
        ])
        .encode();
        let get_cmd = Frame::Array(vec![
            Frame::BulkString(b"GET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
        ])
        .encode();

        let expected = Frame::BulkString(b"secret stuff".to_vec());
        let _ = send_cmd(&mut conn, &set_cmd).await;
        let resp = send_cmd(&mut conn, &get_cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn get_non_existent_key_returns_null() {
        let addr = spawn_server().await;
        let mut conn = connect_to_server(addr).await;
        let get_cmd = Frame::Array(vec![
            Frame::BulkString(b"GET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
        ])
        .encode();

        let expected = Frame::Null;
        let resp = send_cmd(&mut conn, &get_cmd).await;

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn different_clients_set_and_get_expect_same_value() {
        let addr = spawn_server().await;
        let mut client = connect_to_server(addr).await;
        let mut client2 = connect_to_server(addr).await;

        let set_cmd = Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
            Frame::BulkString(b"ULTRA SECRET".to_vec()),
        ])
        .encode();
        let get_cmd = Frame::Array(vec![
            Frame::BulkString(b"GET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
        ])
        .encode();

        let _ = send_cmd(&mut client, &set_cmd).await;
        let resp = send_cmd(&mut client2, &get_cmd).await;
        let expected = Frame::BulkString(b"ULTRA SECRET".to_vec());

        assert_eq!(resp, expected);
    }

    #[tokio::test]
    async fn set_ovewrites_previous_value() {
        let addr = spawn_server().await;
        let mut client = connect_to_server(addr).await;

        let set_cmd = Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
            Frame::BulkString(b"ULTRA SECRET".to_vec()),
        ])
        .encode();
        let set_cmd2 = Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
            Frame::BulkString(b"this is not a secret no more".to_vec()),
        ])
        .encode();
        let get_cmd = Frame::Array(vec![
            Frame::BulkString(b"GET".to_vec()),
            Frame::BulkString(b"mykey".to_vec()),
        ])
        .encode();

        let _ = send_cmd(&mut client, &set_cmd).await;
        let _ = send_cmd(&mut client, &set_cmd2).await;
        let resp = send_cmd(&mut client, &get_cmd).await;
        let expected = Frame::BulkString(b"this is not a secret no more".to_vec());

        assert_eq!(resp, expected);
    }
}
