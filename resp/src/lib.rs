use std::io::{BufRead, Read, Write};

#[derive(PartialEq, Debug)]
enum Frame {
    Integer(i64),
    SimpleString(String),
    BulkString(Vec<u8>),
    Error(String),
    Null,
    Array(Vec<Frame>),
}

impl Frame {
    fn encode(&self) -> Vec<u8> {
        match self {
            Frame::Integer(i) => format!(":{i}\r\n").as_bytes().to_vec(),
            Frame::SimpleString(s) => format!("+{s}\r\n").as_bytes().to_vec(),
            Frame::BulkString(items) => {
                // overhead: $ (1) + length digits (up to 20 for u64) + 2x \r\n (4) = 25
                let mut buf = Vec::with_capacity(items.len() + 25);
                let _ = write!(&mut buf, "${}\r\n", items.len());
                buf.extend_from_slice(items);
                buf.extend_from_slice(b"\r\n");

                buf
            }
            Frame::Error(s) => format!("-{s}\r\n").as_bytes().to_vec(),
            Frame::Array(frames) => {
                let mut buf = Vec::new();
                let _ = write!(&mut buf, "*{}\r\n", frames.len());
                let f: Vec<u8> = frames.iter().flat_map(|frame| frame.encode()).collect();
                buf.extend_from_slice(&f);

                buf
            }
            Frame::Null => b"$-1\r\n".to_vec(),
        }
    }
    fn decode(mut bytes: &[u8]) -> (Self, usize) {
        let mut first_byte = [0; 1];
        let _ = bytes.read_exact(&mut first_byte);

        match first_byte[0] {
            b'+' => {
                let mut buf = Vec::new();
                let n = bytes.read_until(b'\r', &mut buf).unwrap();
                buf.pop();

                let s = String::from_utf8(buf).unwrap();
                (Frame::SimpleString(s), n + 2)
            }
            b'-' => {
                let mut buf = Vec::new();
                let n = bytes.read_until(b'\r', &mut buf).unwrap();
                buf.pop();

                let s = String::from_utf8(buf).unwrap();
                (Frame::Error(s), n + 2)
            }
            b':' => {
                let mut buf = Vec::new();
                let n = bytes.read_until(b'\r', &mut buf).unwrap();
                buf.pop();
                let s = str::from_utf8(&buf).unwrap();
                let num: i64 = s.parse().unwrap();

                (Frame::Integer(num), n + 2)
            }
            b'$' => {
                let mut length = Vec::new();
                let n = bytes.read_until(b'\n', &mut length).unwrap();
                length.pop();
                length.pop();
                let count = String::from_utf8(length).unwrap();
                let count: i64 = count.parse().unwrap();

                if count == -1 {
                    return (Frame::Null, 5);
                }

                let mut data = vec![0; count as usize];
                bytes.read_exact(&mut data).unwrap();

                (Frame::BulkString(data), n + (count as usize) + 3)
            }
            b'*' => {
                let mut count = Vec::new();
                let n = bytes.read_until(b'\n', &mut count).unwrap();
                count.pop();
                count.pop();
                let count = String::from_utf8(count).unwrap();
                let count: i64 = count.parse().unwrap();

                if count == -1 {
                    return (Frame::Null, 5);
                }

                let mut arr: Vec<Frame> = Vec::new();
                let mut cursor = 0;
                for _ in 0..count {
                    let (f, consumed) = Self::decode(&bytes[cursor..]);
                    arr.push(f);
                    cursor += consumed;
                }

                (Frame::Array(arr), cursor + n + 1)
            }
            _ => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod decoding_produces_frame {
        use super::super::*;

        mod simple_string {
            use crate::Frame;

            #[test]
            fn single() {
                let b = b"+PONG\r\n";

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, Frame::SimpleString("PONG".to_string()));
                assert_eq!(consumed, 7);
            }
            #[test]
            fn multiple() {
                let b = b"+OK\r\n+PING\r\n";

                let (f, consumed) = Frame::decode(b);
                let (f2, _) = Frame::decode(&b[consumed..]);

                assert_eq!(f, Frame::SimpleString("OK".to_string()));
                assert_eq!(f2, Frame::SimpleString("PING".to_string()));
            }
        }

        #[test]
        fn error() {
            let b = b"-ERR unknown command 'FOO'\r\n";

            let (f, consumed) = Frame::decode(b);

            assert_eq!(f, Frame::Error("ERR unknown command 'FOO'".to_string()));
            assert_eq!(consumed, 28);
        }

        mod integer {
            use crate::Frame;

            #[test]
            fn positive() {
                let b = b":42069\r\n";

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, Frame::Integer(42069));
                assert_eq!(consumed, 8);
            }
            #[test]
            fn negative() {
                let b = b":-1337\r\n";

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, Frame::Integer(-1337));
                assert_eq!(consumed, 8);
            }
        }

        mod bulk_string {
            use crate::Frame;

            #[test]
            fn non_empty() {
                let b = b"$26\r\nhello world, how are you!?\r\n";
                let expected = Frame::BulkString(b"hello world, how are you!?".to_vec());

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 33);
            }
            #[test]
            fn empty() {
                let b = b"$0\r\n\r\n";
                let expected = Frame::BulkString(b"".to_vec());

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 6);
            }
        }

        mod null {
            use crate::Frame;

            #[test]
            fn from_bulk_string() {
                let b = b"$-1\r\n";
                let expected = Frame::Null;

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 5);
            }
            #[test]
            fn from_array() {
                let b = b"*-1\r\n";
                let expected = Frame::Null;

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 5);
            }
        }

        mod array {
            use crate::Frame;

            #[test]
            fn empty() {
                let b = b"*0\r\n";
                let expected = Frame::Array(Vec::new());

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 4);
            }
            #[test]
            fn simple() {
                let b = b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n+OK\r\n";
                let expected = Frame::Array(Vec::from([
                    Frame::BulkString(b"SET".to_vec()),
                    Frame::BulkString(b"mykey".to_vec()),
                    Frame::BulkString(b"hello".to_vec()),
                ]));

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 35);
            }
            #[test]
            fn nested() {
                // Input bytes: an array containing [integer 1, array [simple string "OK", integer 2]]
                let b = b"*2\r\n:1\r\n*2\r\n+OK\r\n:2\r\n";
                let expected = Frame::Array(vec![
                    Frame::Integer(1),
                    Frame::Array(vec![
                        Frame::SimpleString("OK".to_string()),
                        Frame::Integer(2),
                    ]),
                ]);

                let (f, consumed) = Frame::decode(b);

                assert_eq!(f, expected);
                assert_eq!(consumed, 21);
            }
        }
    }

    mod encoding_produces_prefix_and_crlf {
        use crate::Frame;

        mod integer {
            use crate::Frame;

            #[test]
            fn positive() {
                assert_eq!(Frame::Integer(42).encode(), b":42\r\n");
            }
            #[test]
            fn negative() {
                assert_eq!(Frame::Integer(-1).encode(), b":-1\r\n");
            }
            #[test]
            fn zero() {
                assert_eq!(Frame::Integer(0).encode(), b":0\r\n");
            }
        }

        #[test]
        fn simple_string() {
            let short = Frame::SimpleString("OK".to_string());
            let longer = Frame::SimpleString("PONG".to_string());

            assert_eq!(short.encode(), b"+OK\r\n");
            assert_eq!(longer.encode(), b"+PONG\r\n");
        }

        mod bulk_string {
            use crate::Frame;

            #[test]
            fn empty() {
                let empty = Frame::BulkString("".as_bytes().to_vec());

                assert_eq!(empty.encode(), b"$0\r\n\r\n");
            }
            #[test]
            fn non_empty() {
                let short = Frame::BulkString("hello".as_bytes().to_vec());
                let long = Frame::BulkString("hello world, how are you!?".as_bytes().to_vec());

                assert_eq!(short.encode(), b"$5\r\nhello\r\n");
                assert_eq!(long.encode(), b"$26\r\nhello world, how are you!?\r\n");
            }
        }

        mod array {
            use crate::Frame;

            #[test]
            fn empty() {
                let empty = Frame::Array(Vec::new()).encode();

                assert_eq!(empty, b"*0\r\n");
            }
            #[test]
            fn non_empty() {
                let b = Frame::Array(Vec::from([
                    Frame::BulkString(b"SET".to_vec()),
                    Frame::BulkString(b"mykey".to_vec()),
                    Frame::BulkString(b"hello".to_vec()),
                ]))
                .encode();

                let b2 = Frame::Array(Vec::from([
                    Frame::SimpleString("OK".to_string()),
                    Frame::SimpleString("OK".to_string()),
                    Frame::BulkString(b"1".to_vec()),
                ]))
                .encode();

                assert_eq!(b, b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n");
                assert_eq!(b2, b"*3\r\n+OK\r\n+OK\r\n$1\r\n1\r\n");
            }
            #[test]
            fn nested() {
                let b = Frame::Array(vec![
                    Frame::Integer(1),
                    Frame::Array(vec![
                        Frame::SimpleString("OK".to_string()),
                        Frame::Integer(2),
                    ]),
                ])
                .encode();

                let expected = b"*2\r\n:1\r\n*2\r\n+OK\r\n:2\r\n";

                assert_eq!(b, expected);
            }
        }

        #[test]
        fn null() {
            assert_eq!(Frame::Null.encode(), b"$-1\r\n");
        }

        #[test]
        fn error() {
            let b = Frame::Error("ERR unknow command 'cmd'".to_string());
            let b2 = Frame::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            );

            assert_eq!(b.encode(), b"-ERR unknow command 'cmd'\r\n");
            assert_eq!(
                b2.encode(),
                b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
            )
        }
    }
}
