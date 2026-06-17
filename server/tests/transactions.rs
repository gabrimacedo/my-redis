mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn multi_returns_ok() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![Frame::BulkString(b"MULTI".to_vec())]).encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(resp, Frame::SimpleString("OK".to_string()));
}
