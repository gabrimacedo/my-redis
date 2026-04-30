use resp::Frame;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            stream,
        }
    }

    pub async fn read_frame(&mut self) -> io::Result<Frame> {
        // TODO: investigate better memory efficiency for internal buffer
        loop {
            // read frame from internal buffer
            let (in_frame, consumed) = Frame::decode(&self.buffer[self.cursor..]);

            if matches!(in_frame, Frame::Incomplete) {
                // grow the buffer to read into new space
                let len = self.buffer.len();
                self.buffer.resize(len + 128, 0);

                // read from stream & append to internal buffer
                let n = self.stream.read(&mut self.buffer[len..]).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "client disconnected",
                    ));
                }

                // truncate to used size
                self.buffer.truncate(len + n);
                continue;
            }

            self.cursor += consumed;
            if self.cursor > self.buffer.len() / 2 {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
            }
            return Ok(in_frame);
        }
    }

    pub async fn write_frames(&mut self, frames: &[Frame]) -> io::Result<usize> {
        let mut response_buffer = Vec::new();
        for f in frames {
            response_buffer.extend(f.encode());
        }
        self.stream.write(&response_buffer).await
    }
}
