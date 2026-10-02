use crate::memory::store::Store;
use crate::memory::store::StoreError;
use crate::resp::command::Command;
use crate::resp::command::CommandError;
use crate::resp::frame::RespFrame;
use std::fmt;

#[derive(Debug)]
pub enum KVEngineError {
    CommandError(CommandError),
    StoreError(StoreError),
}

impl From<CommandError> for KVEngineError {
    fn from(err: CommandError) -> Self {
        Self::CommandError(err)
    }
}

impl From<StoreError> for KVEngineError {
    fn from(err: StoreError) -> Self {
        Self::StoreError(err)
    }
}

impl fmt::Display for KVEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KVEngine error: {:?}", self)
    }
}

impl std::error::Error for KVEngineError {}

pub struct KvEngine {
    store: Store,
    default_ttl: Option<u64>,
}

impl Default for KvEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl KvEngine {
    pub fn new() -> Self {
        Self {
            store: Store::new(),
            default_ttl: None,
        }
    }

    pub fn with_default_ttl(ttl_seconds: u64) -> Self {
        Self {
            store: Store::new(),
            default_ttl: Some(ttl_seconds),
        }
    }

    pub fn execute(&mut self, frame: RespFrame) -> Result<RespFrame, KVEngineError> {
        let cmd = Command::try_from(frame)?;
        let response = match cmd {
            Command::Get { key } => match self.store.get(key.as_bytes())? {
                Some(val) => RespFrame::BulkString(val),
                None => RespFrame::Null,
            },
            Command::Put { key, value } => {
                self.store.put(key.into_bytes(), value, self.default_ttl)?;
                RespFrame::SimpleString("OK".to_string())
            }
            Command::Delete { key } => {
                let deleted = self.store.delete(key.as_bytes())?;
                RespFrame::Integer(if deleted { 1 } else { 0 })
            }
            Command::Unknown(s) => RespFrame::Error(format!("ERR unknown command '{}'", s)),
        };

        Ok(response)
    }

    pub fn execute_batch(&mut self, frames: Vec<RespFrame>) -> Result<(), KVEngineError> {
        for frame in frames {
            let cmd = Command::try_from(frame)?;
            match cmd {
                Command::Put { key, value } => {
                    self.store.put(key.into_bytes(), value, self.default_ttl)?;
                }
                Command::Delete { key } => {
                    self.store.delete(key.as_bytes())?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resp::frame::RespFrame;

    #[test]
    fn test_execute_put_and_get_success() {
        let mut engine = KvEngine::new();

        let put_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"mykey".to_vec()),
            RespFrame::BulkString(b"myvalue".to_vec()),
        ]);

        let response = engine.execute(put_frame).unwrap();
        assert_eq!(response, RespFrame::SimpleString("OK".to_string()));

        let get_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"mykey".to_vec()),
        ]);

        let response = engine.execute(get_frame).unwrap();
        assert_eq!(response, RespFrame::BulkString(b"myvalue".to_vec()));
    }

    #[test]
    fn test_execute_get_non_existent_key() {
        let mut engine = KvEngine::new();

        let get_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"missing_key".to_vec()),
        ]);

        let response = engine.execute(get_frame).unwrap();
        assert_eq!(response, RespFrame::Null);
    }

    #[test]
    fn test_execute_delete() {
        let mut engine = KvEngine::new();

        let put_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key_to_delete".to_vec()),
            RespFrame::BulkString(b"val".to_vec()),
        ]);
        engine.execute(put_frame).unwrap();

        let del_frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"DEL".to_vec()),
            RespFrame::BulkString(b"key_to_delete".to_vec()),
        ]);
        let response = engine.execute(del_frame).unwrap();
        assert_eq!(response, RespFrame::Integer(1));

        let del_frame_again = RespFrame::Array(vec![
            RespFrame::BulkString(b"DEL".to_vec()),
            RespFrame::BulkString(b"key_to_delete".to_vec()),
        ]);
        let response = engine.execute(del_frame_again).unwrap();
        assert_eq!(response, RespFrame::Integer(0));
    }

    #[test]
    fn test_execute_unknown_command() {
        let mut engine = KvEngine::new();

        let unknown_frame = RespFrame::Array(vec![RespFrame::BulkString(b"FOOBAR".to_vec())]);

        let response = engine.execute(unknown_frame).unwrap();
        match response {
            RespFrame::Error(msg) => assert!(msg.contains("ERR unknown command")),
            _ => panic!("Expected RespFrame::Error"),
        }
    }

    #[test]
    fn test_execute_batch_aof_replay() {
        let mut engine = KvEngine::new();

        let aof_frames = vec![
            RespFrame::Array(vec![
                RespFrame::BulkString(b"SET".to_vec()),
                RespFrame::BulkString(b"k1".to_vec()),
                RespFrame::BulkString(b"v1".to_vec()),
            ]),
            RespFrame::Array(vec![
                RespFrame::BulkString(b"SET".to_vec()),
                RespFrame::BulkString(b"k2".to_vec()),
                RespFrame::BulkString(b"v2".to_vec()),
            ]),
            RespFrame::Array(vec![
                RespFrame::BulkString(b"DEL".to_vec()),
                RespFrame::BulkString(b"k1".to_vec()),
            ]),
        ];

        engine.execute_batch(aof_frames).unwrap();

        let get_k1 = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"k1".to_vec()),
        ]);
        assert_eq!(engine.execute(get_k1).unwrap(), RespFrame::Null);

        let get_k2 = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"k2".to_vec()),
        ]);
        assert_eq!(
            engine.execute(get_k2).unwrap(),
            RespFrame::BulkString(b"v2".to_vec())
        );
    }
}
