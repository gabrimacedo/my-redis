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
async fn llen_nonexistent_ley_returns_0() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let len_cmd = Frame::Array(vec![
        Frame::BulkString(b"LLEN".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let resp = send_cmd(&mut client, &len_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(0));
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

#[tokio::test]
async fn lpush_inserts_at_the_front() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"a".to_vec()),
        Frame::BulkString(b"b".to_vec()),
        Frame::BulkString(b"c".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"0".to_vec()),
        Frame::BulkString(b"-1".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &cmd.encode()).await;
    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"c".to_vec()),
            Frame::BulkString(b"b".to_vec()),
            Frame::BulkString(b"a".to_vec()),
        ])
    );
}

#[tokio::test]
async fn rpush_inserts_at_the_back() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"a".to_vec()),
        Frame::BulkString(b"b".to_vec()),
        Frame::BulkString(b"c".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"0".to_vec()),
        Frame::BulkString(b"-1".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &cmd.encode()).await;
    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"a".to_vec()),
            Frame::BulkString(b"b".to_vec()),
            Frame::BulkString(b"c".to_vec()),
        ])
    );
}

#[tokio::test]
async fn list_cmds_if_key_holds_string_returns_error() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"not a list".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
    ]);
    let lpush_cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"not a list".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let rpush_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"not a list".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let llen_cmd = Frame::Array(vec![
        Frame::BulkString(b"llen".to_vec()),
        Frame::BulkString(b"not a list".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"lrange".to_vec()),
        Frame::BulkString(b"not a list".to_vec()),
        Frame::BulkString(b"0".to_vec()),
        Frame::BulkString(b"-1".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &set_cmd.encode()).await;

    assert_eq!(
        send_cmd(&mut client, &lpush_cmd.encode()).await,
        Frame::Error("WRONGTYPE error".to_string())
    );
    assert_eq!(
        send_cmd(&mut client, &rpush_cmd.encode()).await,
        Frame::Error("WRONGTYPE error".to_string())
    );
    assert_eq!(
        send_cmd(&mut client, &llen_cmd.encode()).await,
        Frame::Error("WRONGTYPE error".to_string())
    );
    assert_eq!(
        send_cmd(&mut client, &range_cmd.encode()).await,
        Frame::Error("WRONGTYPE error".to_string())
    );
}

#[tokio::test]
async fn lrange_returns_elements_from_start_to_stop() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"0".to_vec()),
        Frame::BulkString(b"2".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"world".to_vec()),
            Frame::BulkString(b"magic".to_vec()),
            Frame::BulkString(b"hello".to_vec()),
        ])
    );
}

#[tokio::test]
async fn lrange_works_with_negative_indexes() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"insane".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"0".to_vec()),
        Frame::BulkString(b"-1".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"hello".to_vec()),
            Frame::BulkString(b"insane".to_vec()),
            Frame::BulkString(b"magic".to_vec()),
            Frame::BulkString(b"world".to_vec()),
        ])
    );
}

#[tokio::test]
async fn lrange_ouside_of_list_range_returns_empty_array() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"LPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"hello".to_vec()),
        Frame::BulkString(b"magic".to_vec()),
        Frame::BulkString(b"world".to_vec()),
    ]);
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"10".to_vec()),
        Frame::BulkString(b"15".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(resp, Frame::Array(vec![]));
}

#[tokio::test]
async fn lrange_on_nonexistent_key_returns_empty_array() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let range_cmd = Frame::Array(vec![
        Frame::BulkString(b"LRANGE".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"10".to_vec()),
        Frame::BulkString(b"15".to_vec()),
    ]);

    let resp = send_cmd(&mut client, &range_cmd.encode()).await;

    assert_eq!(resp, Frame::Array(vec![]));
}

#[tokio::test]
async fn lpop_returns_first_element() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"a".to_vec()),
        Frame::BulkString(b"b".to_vec()),
        Frame::BulkString(b"c".to_vec()),
        Frame::BulkString(b"d".to_vec()),
    ]);
    let pop_cmd = Frame::Array(vec![
        Frame::BulkString(b"LPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let resp = send_cmd(&mut client, &pop_cmd.encode()).await;

    assert_eq!(resp, Frame::BulkString(b"a".to_vec()));
}

#[tokio::test]
async fn rpop_returns_last_element() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"a".to_vec()),
        Frame::BulkString(b"b".to_vec()),
        Frame::BulkString(b"c".to_vec()),
        Frame::BulkString(b"d".to_vec()),
    ]);
    let pop_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let resp = send_cmd(&mut client, &pop_cmd.encode()).await;

    assert_eq!(resp, Frame::BulkString(b"d".to_vec()));
}

#[tokio::test]
async fn rpop_nonexisent_key_returns_null() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let pop_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let resp = send_cmd(&mut client, &pop_cmd.encode()).await;

    assert_eq!(resp, Frame::Null)
}

#[tokio::test]
async fn pop_until_empty_deletes_list() {
    let addr = spawn_server().await;
    let mut client = connect_to_server(addr).await;
    let push_cmd = Frame::Array(vec![
        Frame::BulkString(b"RPUSH".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
        Frame::BulkString(b"a".to_vec()),
        Frame::BulkString(b"b".to_vec()),
    ]);
    let pop_cmd1 = Frame::Array(vec![
        Frame::BulkString(b"RPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);
    let pop_cmd2 = Frame::Array(vec![
        Frame::BulkString(b"RPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);
    let pop_cmd3 = Frame::Array(vec![
        Frame::BulkString(b"RPOP".to_vec()),
        Frame::BulkString(b"mylist".to_vec()),
    ]);

    let _ = send_cmd(&mut client, &push_cmd.encode()).await;
    let _ = send_cmd(&mut client, &pop_cmd1.encode()).await;
    let _ = send_cmd(&mut client, &pop_cmd2.encode()).await;
    let resp = send_cmd(&mut client, &pop_cmd3.encode()).await;

    assert_eq!(resp, Frame::Null);
}
