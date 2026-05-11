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

#[tokio::test]
async fn unsubscribe_one_channel_keeps_others() {
    let addr = spawn_server().await;
    let mut sub_client = connect_to_server(addr).await;
    let mut pub_client = connect_to_server(addr).await;

    let sub_ch1 = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let sub_ch2 = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch2".to_vec()),
    ])
    .encode();
    let unsub_ch1 = Frame::Array(vec![
        Frame::BulkString(b"UNSUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let pub_ch2 = Frame::Array(vec![
        Frame::BulkString(b"PUBLISH".to_vec()),
        Frame::BulkString(b"ch2".to_vec()),
        Frame::BulkString(b"hi".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut sub_client, &sub_ch1).await;
    let _ = send_cmd(&mut sub_client, &sub_ch2).await;
    let resp = send_cmd(&mut sub_client, &unsub_ch1).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"unsubscribe".to_vec()),
            Frame::BulkString(b"ch1".to_vec()),
            Frame::Integer(1),
        ])
    );

    let _ = send_cmd(&mut pub_client, &pub_ch2).await;

    assert_eq!(
        get_response(&mut sub_client).await,
        Frame::Array(vec![
            Frame::BulkString(b"message".to_vec()),
            Frame::BulkString(b"ch2".to_vec()),
            Frame::BulkString(b"hi".to_vec()),
        ])
    );
}

#[tokio::test]
async fn unsubscribe_with_no_args_removes_all() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;

    let sub_ch1 = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let unsub_all = Frame::Array(vec![Frame::BulkString(b"UNSUBSCRIBE".to_vec())]).encode();

    let _ = send_cmd(&mut conn, &sub_ch1).await;
    let resp = send_cmd(&mut conn, &unsub_all).await;

    assert_eq!(
        resp,
        Frame::Array(vec![
            Frame::BulkString(b"unsubscribe".to_vec()),
            Frame::BulkString(b"ch1".to_vec()),
            Frame::Integer(0),
        ])
    );
}

#[tokio::test]
async fn subscribed_client_set_returns_error() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;

    let sub_cmd = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let set_cmd = Frame::Array(vec![
        Frame::BulkString(b"SET".to_vec()),
        Frame::BulkString(b"key".to_vec()),
        Frame::BulkString(b"value".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut conn, &sub_cmd).await;
    let resp = send_cmd(&mut conn, &set_cmd).await;

    assert_eq!(
        resp,
        Frame::Error("ERR Command not allowed in subscription mode".to_string())
    );
}

#[tokio::test]
async fn unsubscribe_multiple_channels_decrements_count_per_frame() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;

    let sub_ch1 = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let sub_ch2 = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch2".to_vec()),
    ])
    .encode();
    let unsub_both = Frame::Array(vec![
        Frame::BulkString(b"UNSUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
        Frame::BulkString(b"ch2".to_vec()),
    ])
    .encode();

    let _ = send_cmd(&mut conn, &sub_ch1).await;
    let _ = send_cmd(&mut conn, &sub_ch2).await;
    let frames = send_cmd_multi_response(&mut conn, &unsub_both).await;

    assert_eq!(
        frames,
        vec![
            Frame::Array(vec![
                Frame::BulkString(b"unsubscribe".to_vec()),
                Frame::BulkString(b"ch1".to_vec()),
                Frame::Integer(1),
            ]),
            Frame::Array(vec![
                Frame::BulkString(b"unsubscribe".to_vec()),
                Frame::BulkString(b"ch2".to_vec()),
                Frame::Integer(0),
            ]),
        ]
    );
}

#[tokio::test]
async fn subscribed_client_ping_returns_pong() {
    let addr = spawn_server().await;
    let mut conn = connect_to_server(addr).await;

    let sub_cmd = Frame::Array(vec![
        Frame::BulkString(b"SUBSCRIBE".to_vec()),
        Frame::BulkString(b"ch1".to_vec()),
    ])
    .encode();
    let ping_cmd = Frame::Array(vec![Frame::BulkString(b"PING".to_vec())]).encode();

    let _ = send_cmd(&mut conn, &sub_cmd).await;
    let resp = send_cmd(&mut conn, &ping_cmd).await;

    assert_eq!(resp, Frame::SimpleString("PONG".to_string()));
}
