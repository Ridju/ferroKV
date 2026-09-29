use crate::resp::frame::RespFrame;

#[derive(Debug, PartialEq)]
pub enum CommandError {
    InvalidType,
    Incomplete,
    InvalidFormat,
    ParseError(String),
}

impl From<std::string::FromUtf8Error> for CommandError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        CommandError::ParseError(err.to_string())
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Get { key: String },
    Put { key: String, value: Vec<u8> },
    Delete { key: String },
    Unknown(String),
}

impl TryFrom<RespFrame> for Command {
    type Error = CommandError;

    fn try_from(frame: RespFrame) -> Result<Self, Self::Error> {
        let frames = match frame {
            RespFrame::Array(f) => f,
            _ => return Err(CommandError::InvalidFormat),
        };

        let mut iter = frames.into_iter();
        let cmd_frame = iter.next().ok_or(CommandError::InvalidFormat)?;
        let cmd_name = Self::get_key_name(cmd_frame)?;

        match cmd_name.to_uppercase().as_str() {
            "GET" => {
                let key_frame = iter.next().ok_or(CommandError::InvalidFormat)?;
                let key = Self::get_key_name(key_frame)?;
                if iter.next().is_some() {
                    return Err(CommandError::InvalidFormat);
                }
                Ok(Command::Get { key })
            }
            "PUT" | "SET" => {
                let key_frame = iter.next().ok_or(CommandError::InvalidFormat)?;
                let key = Self::get_key_name(key_frame)?;

                let val_frame = iter.next().ok_or(CommandError::InvalidFormat)?;
                let value = match val_frame {
                    RespFrame::BulkString(bytes) => bytes,
                    RespFrame::SimpleString(s) => s.into_bytes(),
                    _ => {
                        return Err(CommandError::InvalidFormat);
                    }
                };
                if iter.next().is_some() {
                    return Err(CommandError::InvalidFormat);
                }
                Ok(Command::Put { key, value })
            }
            "DELETE" | "DEL" => {
                let key_frame = iter.next().ok_or(CommandError::InvalidFormat)?;
                let key = Self::get_key_name(key_frame)?;
                if iter.next().is_some() {
                    return Err(CommandError::InvalidFormat);
                }
                Ok(Command::Delete { key })
            }
            _ => Ok(Command::Unknown(cmd_name)),
        }
    }
}

impl Command {
    fn get_key_name(frame: RespFrame) -> Result<String, CommandError> {
        let key = match frame {
            RespFrame::BulkString(bytes) => String::from_utf8(bytes)?,
            RespFrame::SimpleString(s) => s,
            _ => return Err(CommandError::InvalidType),
        };

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_get_command_success() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"user_key".to_vec()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Get {
                key: "user_key".to_string()
            }
        );
    }

    #[test]
    fn parse_set_command_success() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::BulkString(b"value".to_vec()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Put {
                key: "key".to_string(),
                value: b"value".to_vec()
            }
        );
    }

    #[test]
    fn parse_put_command_success() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"PUT".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::SimpleString("simple_value".to_string()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Put {
                key: "key".to_string(),
                value: b"simple_value".to_vec()
            }
        );
    }

    #[test]
    fn parse_del_command_success() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"DEL".to_vec()),
            RespFrame::BulkString(b"target_key".to_vec()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Delete {
                key: "target_key".to_string()
            }
        );
    }

    #[test]
    fn parse_delete_command_success() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"DELETE".to_vec()),
            RespFrame::BulkString(b"target_key".to_vec()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Delete {
                key: "target_key".to_string()
            }
        );
    }

    #[test]
    fn parse_case_insensitive() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"gEt".to_vec()),
            RespFrame::SimpleString("my_key".to_string()),
        ]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(
            cmd,
            Command::Get {
                key: "my_key".to_string()
            }
        );
    }

    #[test]
    fn parse_unknown_command() {
        let frame = RespFrame::Array(vec![RespFrame::BulkString(b"INFO".to_vec())]);
        let cmd = Command::try_from(frame).unwrap();
        assert_eq!(cmd, Command::Unknown("INFO".to_string()));
    }

    #[test]
    fn fail_on_non_array_frame() {
        let frame = RespFrame::SimpleString("GET".to_string());
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }

    #[test]
    fn fail_on_empty_array() {
        let frame = RespFrame::Array(vec![]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }

    #[test]
    fn fail_on_missing_key_argument() {
        let frame = RespFrame::Array(vec![RespFrame::BulkString(b"GET".to_vec())]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }

    #[test]
    fn fail_on_missing_value_argument() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
        ]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }

    #[test]
    fn fail_on_extra_arguments() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::BulkString(b"extra".to_vec()),
        ]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }

    #[test]
    fn fail_on_invalid_cmd_type() {
        let frame = RespFrame::Array(vec![
            RespFrame::Integer(100),
            RespFrame::BulkString(b"key".to_vec()),
        ]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidType));
    }

    #[test]
    fn fail_on_invalid_key_type() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::Integer(42),
        ]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidType));
    }

    #[test]
    fn fail_on_invalid_value_type() {
        let frame = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::Integer(500),
        ]);
        let result = Command::try_from(frame);
        assert_eq!(result, Err(CommandError::InvalidFormat));
    }
}
