mod common;
use common::*;
use resp::Frame;

#[tokio::test]
async fn subscribe_returns_sub_confirmation() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;
    let cmd = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();

    let resp = send_cmd(&mut conn, &cmd).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"subscribe".to_vec()),
            Frame::BulkString(b"ch1".to_vec()),
            Frame::Integer(1),
        ])
    );
}

#[tokio::test]
async fn publish_broadcasts_to_all_subs() {
    let addr = spawn_server().await;
    let mut client1 = connect_to_server(addr).await;
    let mut client2 = connect_to_server(addr).await;
    let mut client3 = connect_to_server(addr).await;
    let sub_cmd = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"channel 5 news".to_vec()),
    ]);
    let pub_cmd = Frame::Array(vec![
        Frame::BulkString(b"PUBLISH".to_vec()),
        Frame::BulkString(b"channel 5 news".to_vec()),
        Frame::BulkString(b"this is top secret".to_vec()),
    ]);

    let _ = send_cmd(&mut client1, &sub_cmd.encode()).await;
    let _ = send_cmd(&mut client3, &sub_cmd.encode()).await;
    let _ = send_cmd(&mut client2, &pub_cmd.encode()).await;

    assert_eq!(
        get_response(&mut client1).await,
        Frame::Array(vec![
            Frame::BulkString(b"message".to_vec()),
            Frame::BulkString(b"channel 5 news".to_vec()),
            Frame::BulkString(b"this is top secret".to_vec()),
        ])
    );
    assert_eq!(
        get_response(&mut client3).await,
        Frame::Array(vec![
            Frame::BulkString(b"message".to_vec()),
            Frame::BulkString(b"channel 5 news".to_vec()),
            Frame::BulkString(b"this is top secret".to_vec()),
        ])
    );
}

#[tokio::test]
async fn publish_returns_number_of_clients_that_received() {
    let addr = spawn_server().await;
    let mut client1 = connect_to_server(addr).await;
    let mut client2 = connect_to_server(addr).await;
    let mut client3 = connect_to_server(addr).await;
    let sub_cmd = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"channel 5 news".to_vec()),
    ]);
    let pub_cmd = Frame::Array(vec![
        Frame::BulkString(b"PUBLISH".to_vec()),
        Frame::BulkString(b"channel 5 news".to_vec()),
        Frame::BulkString(b"this is top secret".to_vec()),
    ]);

    let _ = send_cmd(&mut client1, &sub_cmd.encode()).await;
    let _ = send_cmd(&mut client3, &sub_cmd.encode()).await;
    let resp = send_cmd(&mut client2, &pub_cmd.encode()).await;

    assert_eq!(resp, Frame::Integer(2));
}
