use crate::resp::frame::{FrameError, RespFrame};
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: Vec::with_capacity(1024),
        }
    }

    pub async fn read_frame(
        &mut self,
    ) -> Result<Option<RespFrame>, Box<dyn std::error::Error + Send + Sync>> {
        let mut chunk = [0u8; 512];

        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            let bytes_read = self.stream.read(&mut chunk).await?;
            if bytes_read == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed while frame was incomplete",
                    )
                    .into());
                }
            }
            self.buffer.extend_from_slice(&chunk[..bytes_read]);
        }
    }

    fn parse_frame(
        &mut self,
    ) -> Result<Option<RespFrame>, Box<dyn std::error::Error + Send + Sync>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        let mut offset = 0;
        match RespFrame::parse_prefix(&self.buffer, &mut offset) {
            Ok(frame) => {
                self.buffer.drain(..offset);
                Ok(Some(frame))
            }
            Err(FrameError::Incomplete) => Ok(None),
            Err(err) => {
                self.buffer.clear();
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Frame parse error: {:?}", err),
                )))
            }
        }
    }

    pub async fn write_frame(
        &mut self,
        frame: &RespFrame,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut response_bytes = Vec::new();
        frame.encode(&mut response_bytes);

        self.stream.write_all(&response_bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_read_single_frame_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"+OK\r\n").await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut connection = Connection::new(stream);

        let frame = connection.read_frame().await.unwrap();
        assert_eq!(frame, Some(RespFrame::SimpleString("OK".to_string())));

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_read_pipelined_frames() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"+PING\r\n+PONG\r\n").await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut connection = Connection::new(stream);

        let frame1 = connection.read_frame().await.unwrap();
        assert_eq!(frame1, Some(RespFrame::SimpleString("PING".to_string())));

        let frame2 = connection.read_frame().await.unwrap();
        assert_eq!(frame2, Some(RespFrame::SimpleString("PONG".to_string())));

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_read_fragmented_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"$5\r\nhel").await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            socket.write_all(b"lo\r\n").await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut connection = Connection::new(stream);

        let frame = connection.read_frame().await.unwrap();
        assert_eq!(frame, Some(RespFrame::BulkString(b"hello".to_vec())));

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_read_eof_clean() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            drop(socket);
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut connection = Connection::new(stream);

        let frame = connection.read_frame().await.unwrap();
        assert_eq!(frame, None);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_write_frame_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let bytes_read = socket.read(&mut buf).await.unwrap();
            buf[..bytes_read].to_vec()
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let mut connection = Connection::new(stream);

        let frame_to_send = RespFrame::BulkString(b"world".to_vec());
        connection.write_frame(&frame_to_send).await.unwrap();

        let received_bytes = handle.await.unwrap();
        assert_eq!(received_bytes, b"$5\r\nworld\r\n");
    }
}
