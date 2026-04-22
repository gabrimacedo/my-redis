mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn ping_returns_pong() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![Frame::BulkString(b"PING".to_vec())]).encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, Frame::SimpleString("PONG".to_string()));
}

#[tokio::test]
async fn ping_with_argument_returns_argument() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"PING".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, Frame::BulkString(b"hello".to_vec()));
}

#[tokio::test]
async fn echo_with_argument_returns_argument() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"ECHO".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, Frame::BulkString(b"hello".to_vec()));
}

#[tokio::test]
async fn echo_with_no_argument_returns_error() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![Frame::BulkString(b"ECHO".to_vec())]).encode();

    let expected = Frame::Error("ERR wrong number of arguments for 'ECHO' command".to_string());
    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, expected);
}
