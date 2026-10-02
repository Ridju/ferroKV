use std::fmt;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::resp::frame::{FrameError, RespFrame};

#[derive(Debug)]
pub enum AofError {
    IOError(String),
}

impl From<std::io::Error> for AofError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(err.to_string())
    }
}

impl fmt::Display for AofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AofError::IOError(msg) => write!(f, "AOF IO Error: {}", msg),
        }
    }
}

impl std::error::Error for AofError {}

pub struct AofEngine {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl AofEngine {
    pub fn open(path: &Path) -> Result<Self, AofError> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let writer = BufWriter::new(file);

        Ok(Self {
            writer,
            path: path.to_path_buf(),
        })
    }

    pub fn append_bytes(&mut self, bytes: &[u8]) -> Result<(), AofError> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;

        Ok(())
    }

    pub fn load_frames(&self) -> Result<Vec<RespFrame>, AofError> {
        let buffer = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut offset = 0;
        let mut result = Vec::new();

        while offset < buffer.len() {
            match RespFrame::parse_prefix(&buffer, &mut offset) {
                Ok(RespFrame::Array(args)) => {
                    result.push(RespFrame::Array(args));
                }
                Err(FrameError::Incomplete) => break,
                Err(_) => return Err(AofError::IOError("Corrupt AOF Frame".into())),
                _ => continue,
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_open_creates_file() {
        let path = Path::new("test_open.aof");
        let _ = fs::remove_file(path);

        let engine = AofEngine::open(path);
        assert!(engine.is_ok());
        assert!(path.exists());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_append_bytes_and_load_frames() {
        let path = Path::new("test_append_load.aof");
        let _ = fs::remove_file(path);

        {
            let mut engine = AofEngine::open(path).unwrap();
            let frame1 = b"*3\r\n$3\r\nSET\r\n$4\r\nkey1\r\n$6\r\nvalue1\r\n";
            let frame2 = b"*2\r\n$3\r\nDEL\r\n$4\r\nkey1\r\n";

            engine.append_bytes(frame1).unwrap();
            engine.append_bytes(frame2).unwrap();
        }

        let engine = AofEngine::open(path).unwrap();
        let frames = engine.load_frames().unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0],
            RespFrame::Array(vec![
                RespFrame::BulkString(b"SET".to_vec()),
                RespFrame::BulkString(b"key1".to_vec()),
                RespFrame::BulkString(b"value1".to_vec()),
            ])
        );
        assert_eq!(
            frames[1],
            RespFrame::Array(vec![
                RespFrame::BulkString(b"DEL".to_vec()),
                RespFrame::BulkString(b"key1".to_vec()),
            ])
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_load_frames_non_existent_file() {
        let path = Path::new("test_non_existent.aof");
        let _ = fs::remove_file(path);

        let engine = AofEngine {
            writer: BufWriter::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("dummy.aof")
                    .unwrap(),
            ),
            path: path.to_path_buf(),
        };

        let frames = engine.load_frames().unwrap();
        assert!(frames.is_empty());

        let _ = fs::remove_file("dummy.aof");
    }

    #[test]
    fn test_load_frames_with_incomplete_trailing_data() {
        let path = Path::new("test_incomplete.aof");
        let _ = fs::remove_file(path);

        {
            let mut file = File::create(path).unwrap();
            file.write_all(b"*2\r\n$3\r\nDEL\r\n$4\r\nkey1\r\n*3\r\n$3\r\nSET\r\n$2\r\n")
                .unwrap();
        }

        let engine = AofEngine::open(path).unwrap();
        let frames = engine.load_frames().unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            RespFrame::Array(vec![
                RespFrame::BulkString(b"DEL".to_vec()),
                RespFrame::BulkString(b"key1".to_vec()),
            ])
        );

        let _ = fs::remove_file(path);
    }
}
