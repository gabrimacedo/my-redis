mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn set_key_returns_ok() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, Frame::SimpleString("OK".to_string()));
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
    let overwrite_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
        Frame::BulkString(b"this ain't no secret no more".to_vec()),
    ])
    .encode();
    let get_cmd = Frame::Array(vec![
        Frame::BulkString(b"GET".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut client, &set_cmd).await;
    let _ = send_cmd(&mut client, &overwrite_cmd).await;
    let resp = send_cmd(&mut client, &get_cmd).await;

    assert_eq!(
        resp,
        Frame::BulkString(b"this ain't no secret no more".to_vec())
    );
}

#[tokio::test]
async fn set_then_get_returs_same_value() {
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

    let _ = send_cmd(&mut conn, &set_cmd).await;
    let resp = send_cmd(&mut conn, &get_cmd).await;

    assert_eq!(resp, Frame::BulkString(b"secret stuff".to_vec()));
}

#[tokio::test]
async fn set_then_get_by_different_clients_returns_same_value() {
    let addr = spawn_server().await;
    let mut client1 = connect_to_server(addr).await;
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

    let _ = send_cmd(&mut client1, &set_cmd).await;
    let resp = send_cmd(&mut client2, &get_cmd).await;

    assert_eq!(resp, Frame::BulkString(b"ULTRA SECRET".to_vec()));
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

    let resp = send_cmd(&mut conn, &get_cmd).await;

    assert_eq!(resp, Frame::Null);
}
