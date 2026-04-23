mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn lpush_expects_length_of_list() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);

    let resp = send_cmd(&mut client, &cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(3));
}

#[tokio::test]
async fn llen_expects_length_of_list() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
    ]);
    let len_cmd = Frame::Array(vec![
        Frame::BulkString(b"LLEN".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &cmd.encode()).await;
    let resp = send_cmd(&mut client, &len_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(2));
}

#[tokio::test]
async fn lpush_creates_list_if_nonexistant() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let len_cmd = Frame::Array(vec![
        Frame::BulkString(b"LLEN".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &cmd.encode()).await;
    let resp = send_cmd(&mut client, &len_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(3));
}
