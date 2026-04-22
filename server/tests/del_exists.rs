mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn delete_2_set_keys_returns_2() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let set_cmd1 = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
        Frame::BulkString(b"ULTRA SECRET".to_vec()),
    ])
    .encode();
    let set_cmd2 = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"some_other_key".to_vec()),
        Frame::BulkString(b"cookies".to_vec()),
    ])
    .encode();
    let del_cmd = Frame::Array(vec![
        Frame::BulkString(b"DEL".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
        Frame::BulkString(b"some_other_key".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut client, &set_cmd1).await;
    let _ = send_cmd(&mut client, &set_cmd2).await;
    let resp = send_cmd(&mut client, &del_cmd).await;

    assert_eq!(resp, Frame::Integer(2));
}

#[tokio::test]
async fn delete_non_existent_key_returns_0() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let del_cmd = Frame::Array(vec![
        Frame::BulkString(b"DEL".to_vec()),
        Frame::BulkString(b"mykey".to_vec()),
        Frame::BulkString(b"some_other_key".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut client, &del_cmd).await;

    assert_eq!(resp, Frame::Integer(0));
}

#[tokio::test]
async fn exists_on_existing_key_returns_1() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
        Frame::BulkString(b"maybe..".to_vec()),
    ])
    .encode();
    let exists_cmd = Frame::Array(vec![
        Frame::BulkString(b"EXISTS".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut client, &set_cmd).await;
    let resp = send_cmd(&mut client, &exists_cmd).await;

    assert_eq!(resp, Frame::Integer(1));
}

#[tokio::test]
async fn exists_on_non_existent_returns_0() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let exists_cmd = Frame::Array(vec![
        Frame::BulkString(b"EXISTS".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut client, &exists_cmd).await;

    assert_eq!(resp, Frame::Integer(0));
}

#[tokio::test]
async fn exists_same_key_twice_returns_2() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;

    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
        Frame::BulkString(b"maybe..".to_vec()),
    ])
    .encode();
    let exists_cmd = Frame::Array(vec![
        Frame::BulkString(b"EXISTS".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
        Frame::BulkString(b"am_i_real?".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut client, &set_cmd).await;
    let resp = send_cmd(&mut client, &exists_cmd).await;

    assert_eq!(resp, Frame::Integer(2));
}
