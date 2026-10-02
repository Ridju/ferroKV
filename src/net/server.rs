use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::engine::aof::AofEngine;
use crate::engine::kv_engine::KvEngine;
use crate::net::connection::Connection;
use crate::resp::frame::RespFrame;

pub struct Config {
    pub addr: String,
    pub aof_path: PathBuf,
}

impl Config {
    pub fn new(addr: impl Into<String>, aof_path: impl Into<PathBuf>) -> Self {
        Self {
            addr: addr.into(),
            aof_path: aof_path.into(),
        }
    }
}

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut aof = AofEngine::open(&self.config.aof_path)?;
        let mut engine = KvEngine::new();

        let recovery_frames = aof.load_frames()?;
        engine.execute_batch(recovery_frames)?;

        let engine_arc = Arc::new(Mutex::new(engine));

        let (tx, mut rx) = mpsc::channel::<RespFrame>(1024);
        tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                let mut bytes = Vec::new();
                frame.encode(&mut bytes);

                if let Err(e) = aof.append_bytes(&bytes) {
                    eprintln!("AOF error: {:?}", e);
                }
            }
        });

        let listener = TcpListener::bind(&self.config.addr).await?;
        println!("Server running on {}", self.config.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let engine_clone = Arc::clone(&engine_arc);
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                let mut connection = Connection::new(stream);

                while let Ok(Some(frame)) = connection.read_frame().await {
                    let is_write = Self::is_write_command(&frame);

                    // 1. Mutex droppen VOR allen await-Aufrufen
                    let response = {
                        let mut guard = engine_clone.lock().unwrap();
                        match guard.execute(frame.clone()) {
                            Ok(resp) => resp,
                            Err(e) => RespFrame::Error(format!("ERR {:?}", e)),
                        }
                    }; // <- Guard wird genau HIER gedroppt!

                    // 2. AOF-Channel senden (.await ist jetzt sicher)
                    if is_write {
                        let _ = tx_clone.send(frame).await;
                    }

                    // 3. Antwort an Client schreiben (.await ist jetzt sicher)
                    if connection.write_frame(&response).await.is_err() {
                        break;
                    }
                }
            });
        }
    }

    fn is_write_command(frame: &RespFrame) -> bool {
        if let RespFrame::Array(items) = frame
            && let Some(RespFrame::BulkString(cmd_bytes)) = items.first()
        {
            let cmd_upper = String::from_utf8_lossy(cmd_bytes).to_uppercase();
            return matches!(cmd_upper.as_str(), "SET" | "PUT" | "DEL" | "DELETE");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::{Duration, sleep};

    fn get_temp_aof_path(test_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("test_{}_{}.aof", test_name, std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn test_is_write_command() {
        let set_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::BulkString(b"value".to_vec()),
        ]);
        assert!(Server::is_write_command(&set_frame));

        let get_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
        ]);
        assert!(!Server::is_write_command(&get_frame));
    }

    #[tokio::test]
    async fn test_server_startup_and_command_execution() {
        let aof_path = get_temp_aof_path("startup");
        let addr = "127.0.0.1:16379";

        let config = Config::new(addr, aof_path.clone());
        let server = Server::new(config);

        tokio::spawn(async move {
            server.run().await.unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n")
            .await
            .unwrap();

        let mut buffer = [0u8; 5];
        stream.read_exact(&mut buffer).await.unwrap();
        assert_eq!(&buffer, b"+OK\r\n");

        let _ = fs::remove_file(&aof_path);
    }

    #[tokio::test]
    async fn test_server_write_persists_to_aof() {
        let aof_path = get_temp_aof_path("persist");
        let addr = "127.0.0.1:16380";

        let config = Config::new(addr, aof_path.clone());
        let server = Server::new(config);

        tokio::spawn(async move {
            server.run().await.unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n")
            .await
            .unwrap();

        let mut buffer = [0u8; 5];
        stream.read_exact(&mut buffer).await.unwrap();
        assert_eq!(&buffer, b"+OK\r\n");

        sleep(Duration::from_millis(200)).await;

        let aof = AofEngine::open(&aof_path).unwrap();
        let frames = aof.load_frames().unwrap();

        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            RespFrame::Array(vec![
                RespFrame::BulkString(b"SET".to_vec()),
                RespFrame::BulkString(b"k".to_vec()),
                RespFrame::BulkString(b"v".to_vec()),
            ])
        );

        let _ = fs::remove_file(&aof_path);
    }
}
