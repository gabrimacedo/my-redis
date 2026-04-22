mod common;

use common::*;
use resp::Frame;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn set_ex_1_then_get_immediately_returns_value() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
        Frame::BulkString(b"quick".to_vec()),
        Frame::BulkString(b"EX".to_vec()),
        Frame::BulkString(b"1".to_vec()),
    ]);
    let get_cmd = Frame::Array(vec![
        Frame::BulkString(b"GET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    let resp = send_cmd(&mut client, &get_cmd.encode()).await;

    assert_eq!(resp, Frame::BulkString(b"quick".to_vec()));
}

#[tokio::test]
async fn get_returns_null_after_key_expiry() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
        Frame::BulkString(b"quick".to_vec()),
        Frame::BulkString(b"EX..".to_vec()),
        Frame::BulkString(b"1".to_vec()),
    ]);
    let get_cmd = Frame::Array(vec![
        Frame::BulkString(b"GET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    sleep(Duration::from_millis(1100)).await;
    let resp = send_cmd(&mut client, &get_cmd.encode()).await;

    assert_eq!(resp, Frame::Null);
}

#[tokio::test]
async fn set_with_px_get_returns_null_after_expiry() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
        Frame::BulkString(b"quick".to_vec()),
        Frame::BulkString(b"PX..".to_vec()),
        Frame::BulkString(b"100".to_vec()),
    ]);
    let get_cmd = Frame::Array(vec![
        Frame::BulkString(b"GET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    sleep(Duration::from_millis(150)).await;
    let resp = send_cmd(&mut client, &get_cmd.encode()).await;

    assert_eq!(resp, Frame::Null);
}

#[tokio::test]
async fn set_with_both_ex_and_px_returns_error() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
        Frame::BulkString(b"quick".to_vec()),
        Frame::BulkString(b"EX..".to_vec()),
        Frame::BulkString(b"10".to_vec()),
        Frame::BulkString(b"PX..".to_vec()),
        Frame::BulkString(b"10000".to_vec()),
    ]);
    let get_cmd = Frame::Array(vec![
        Frame::BulkString(b"GET".to_vec()),
        Frame::BulkString(b"so".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    sleep(Duration::from_millis(150)).await;
    let resp = send_cmd(&mut client, &get_cmd.encode()).await;

    assert_eq!(resp, Frame::Null);
}

#[tokio::test]
async fn ttl_non_expired_key_returns_positive_int() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"key".to_vec()),
        Frame::BulkString(b"val".to_vec()),
        Frame::BulkString(b"EX".to_vec()),
        Frame::BulkString(b"10".to_vec()),
    ]);
    let ttl_cmd = Frame::Array(vec![
        Frame::BulkString(b"ttl".to_vec()),
        Frame::BulkString(b"key".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    let resp = send_cmd(&mut client, &ttl_cmd.encode()).await;
    let Frame::Integer(n) = resp else {
        panic!("expected integer, got {:?}", resp);
    };

    assert!(n > 0);
}

#[tokio::test]
async fn ttl_key_with_no_expiry_returns_neg1() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"key".to_vec()),
        Frame::BulkString(b"val".to_vec()),
    ]);
    let ttl_cmd = Frame::Array(vec![
        Frame::BulkString(b"ttl".to_vec()),
        Frame::BulkString(b"key".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    let resp = send_cmd(&mut client, &ttl_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(-1))
}

#[tokio::test]
async fn ttl_nonexistent_key_returns_neg2() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let ttl_cmd = Frame::Array(vec![
        Frame::BulkString(b"ttl".to_vec()),
        Frame::BulkString(b"key".to_vec()),
    ]);

    let resp = send_cmd(&mut client, &ttl_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(-2))
}

#[tokio::test]
async fn exists_expired_key_returns_0() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"key".to_vec()),
        Frame::BulkString(b"val".to_vec()),
        Frame::BulkString(b"EX".to_vec()),
        Frame::BulkString(b"1".to_vec()),
    ]);
    let exists_cmd = Frame::Array(vec![
        Frame::BulkString(b"EXISTS".to_vec()),
        Frame::BulkString(b"key".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    sleep(Duration::from_millis(1100)).await;
    let resp = send_cmd(&mut client, &exists_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(0))
}

#[tokio::test]
async fn ttl_expired_key_returns_neg2() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"key".to_vec()),
        Frame::BulkString(b"val".to_vec()),
        Frame::BulkString(b"EX".to_vec()),
        Frame::BulkString(b"1".to_vec()),
    ]);
    let ttl_cmd = Frame::Array(vec![
        Frame::BulkString(b"ttl".to_vec()),
        Frame::BulkString(b"key".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;
    sleep(Duration::from_millis(1100)).await;
    let resp = send_cmd(&mut client, &ttl_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(-2))
}
