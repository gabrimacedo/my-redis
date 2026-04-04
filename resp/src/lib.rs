use std::io::{BufRead, Cursor, Read, Write};

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
        let mut cursor = Cursor::new(bytes);
        let _ = cursor.read_exact(&mut first_byte);

        match first_byte[0] {
            b'+' => {
                let mut buf = Vec::new();
                let n = cursor.read_until(b'\r', &mut buf).unwrap();
                buf.pop();

                let s = String::from_utf8(buf).unwrap();
                (Frame::SimpleString(s), n + 2)
            }
            b'-' => {
                let c = Cursor::new(bytes);

                let mut buf = Vec::new();
                let n = cursor.read_until(b'\r', &mut buf).unwrap();
                buf.pop();

                let s = String::from_utf8(buf).unwrap();
                (Frame::Error(s), n + 2)
            }
            b':' => {
                let mut buf = Vec::new();
                let n = cursor.read_until(b'\r', &mut buf).unwrap();
                buf.pop();
                let s = str::from_utf8(&buf).unwrap();
                let num: i64 = s.parse().unwrap();

                (Frame::Integer(num), n + 2)
            }
            b'$' => {
                todo!()
            }
            b'*' => todo!(),
            _ => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod decoding_produces_frame {
        use super::super::*;

        #[test]
        fn simple_string() {
            let b = b"+PONG\r\n";
            let (f, _) = Frame::decode(b);
            assert_eq!(f, Frame::SimpleString("PONG".to_string()));

            let b = b"+OK\r\n+PING\r\n";
            let (f, consumed) = Frame::decode(b);
            let (f2, _) = Frame::decode(&b[consumed..]);
            assert_eq!(f, Frame::SimpleString("OK".to_string()));
            assert_eq!(f2, Frame::SimpleString("PING".to_string()));
        }

        #[test]
        fn error() {
            let b = b"-ERR unknown command 'FOO'\r\n";
            let (f, consumed) = Frame::decode(b);

            assert_eq!(f, Frame::Error("ERR unknown command 'FOO'".to_string()));
            assert_eq!(consumed, 28);
        }

        #[test]
        fn integer() {
            let b = b":42\r\n";
            let (f, consumed) = Frame::decode(b);

            assert_eq!(f, Frame::Integer(42));
            assert_eq!(consumed, 5);
        }

        #[test]
        fn bulk_string() {
            let b = b"$26\r\nhello world, how are you!?\r\n";
            let long = Frame::BulkString(b"hello world, how are you!?".to_vec());

            let (f, consumed) = Frame::decode(b);

            assert_eq!(f, long);
            // assert_eq!(consumed, 5);
        }
    }

    mod encoding_produces_prefix_and_crlf {
        use super::super::*;

        #[test]
        fn integer() {
            assert_eq!(Frame::Integer(42).encode(), b":42\r\n");
            assert_eq!(Frame::Integer(-1).encode(), b":-1\r\n");
            assert_eq!(Frame::Integer(0).encode(), b":0\r\n");
        }

        #[test]
        fn simple_string() {
            let short = Frame::SimpleString("OK".to_string());
            let longer = Frame::SimpleString("PONG".to_string());

            assert_eq!(short.encode(), b"+OK\r\n");
            assert_eq!(longer.encode(), b"+PONG\r\n");
        }

        #[test]
        fn bulk_string() {
            // arrange
            let short = Frame::BulkString("hello".as_bytes().to_vec());
            let long = Frame::BulkString("hello world, how are you!?".as_bytes().to_vec());
            let empty = Frame::BulkString("".as_bytes().to_vec());

            assert_eq!(short.encode(), b"$5\r\nhello\r\n");
            assert_eq!(long.encode(), b"$26\r\nhello world, how are you!?\r\n");
            assert_eq!(empty.encode(), b"$0\r\n\r\n");
        }

        #[test]
        fn array() {
            let arr = Frame::Array(Vec::from([
                Frame::BulkString(b"SET".to_vec()),
                Frame::BulkString(b"mykey".to_vec()),
                Frame::BulkString(b"hello".to_vec()),
            ]))
            .encode();

            let simp_arr = Frame::Array(Vec::from([
                Frame::SimpleString("OK".to_string()),
                Frame::SimpleString("OK".to_string()),
                Frame::BulkString(b"1".to_vec()),
            ]))
            .encode();

            let empty = Frame::Array(Vec::new()).encode();

            assert_eq!(arr, b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n");
            assert_eq!(simp_arr, b"*3\r\n+OK\r\n+OK\r\n$1\r\n1\r\n");
            assert_eq!(empty, b"*0\r\n");
        }

        #[test]
        fn null() {
            assert_eq!(Frame::Null.encode(), b"$-1\r\n");
        }

        #[test]
        fn error() {
            assert_eq!(
                Frame::Error("ERR unknow command 'cmd'".to_string()).encode(),
                b"-ERR unknow command 'cmd'\r\n"
            );
            assert_eq!(
                Frame::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
                )
                .encode(),
                b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
            )
        }
    }
}
