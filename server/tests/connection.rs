mod common;
use common::*;
use resp::Frame;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn accept_lower_case_cmds() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![Frame::BulkString(b"ping".to_vec())]);

    let resp = send_cmd(&mut conn, &cmd.encode()).await;

    assert_eq!(resp, Frame::SimpleString("PONG".to_string()));
}

#[tokio::test]
async fn send_foobar_expect_unknow_cmd_error() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![Frame::BulkString(b"FOOBAR".to_vec())]);

    let resp = send_cmd(&mut conn, &cmd.encode()).await;

    assert_eq!(
        resp,
        Frame::Error("ERR unknown command 'FOOBAR'".to_string())
    );
}

#[tokio::test]
async fn incomplete_frame_waits_for_more_data() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;

    let mut resp = [0u8; 1024];
    let _ = conn
        .write(b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n")
        .await
        .unwrap();
    let _ = conn.read(&mut resp).await.unwrap();
    let _ = conn.write(b"*2\r\n$3\r\nGET\r\n$3\r\nk").await.unwrap();
    let _ = conn.write(b"ey\r\n").await.unwrap();
    let n = conn.read(&mut resp).await.unwrap();
    let (frame, _) = Frame::decode(&resp[..n]);

    assert_eq!(frame, Frame::BulkString(b"value".to_vec()));
}

#[tokio::test]
async fn send_multiple_cmds_at_once() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let cmds = vec![
        Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"fruit".to_vec()),
            Frame::BulkString(b"papaya".to_vec()),
        ])
        .encode(),
        Frame::Array(vec![
            Frame::BulkString(b"SET".to_vec()),
            Frame::BulkString(b"fruit".to_vec()),
            Frame::BulkString(b"banana".to_vec()),
        ])
        .encode(),
        Frame::Array(vec![
            Frame::BulkString(b"GET".to_vec()),
            Frame::BulkString(b"fruit".to_vec()),
        ])
        .encode(),
    ];

    let frames = send_multiple_cmds(&mut client, cmds).await;

    assert_eq!(frames[0], Frame::SimpleString("OK".to_string()));
    assert_eq!(frames[1], Frame::SimpleString("OK".to_string()));
    assert_eq!(frames[2], Frame::BulkString(b"banana".to_vec()));
}
