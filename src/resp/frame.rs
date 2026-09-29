#[derive(Debug, PartialEq)]
pub enum FrameError {
    InvalidType,
    Incomplete,
    InvalidFormat,
    ParseError(String),
}

impl From<std::str::Utf8Error> for FrameError {
    fn from(err: std::str::Utf8Error) -> Self {
        FrameError::ParseError(err.to_string())
    }
}

#[derive(Debug, PartialEq)]
pub enum RespFrame {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Vec<u8>),
    Array(Vec<RespFrame>),
    Null,
}

impl RespFrame {
    pub fn parse_prefix(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        if *offset >= input.len() {
            return Err(FrameError::Incomplete);
        }
        match input[*offset] {
            b'+' => {
                *offset += 1;
                Self::parse_string(input, offset)
            }
            b'-' => {
                *offset += 1;
                Self::parse_error(input, offset)
            }

            b':' => {
                *offset += 1;
                Self::parse_int(input, offset)
            }
            b'$' => {
                *offset += 1;
                Self::parse_bulk_string(input, offset)
            }
            b'*' => {
                *offset += 1;
                Self::parse_array(input, offset)
            }
            _ => Err(FrameError::InvalidType),
        }
    }

    fn parse_string(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        let line = Self::read_line(input, offset)?;
        let ouput_str = std::str::from_utf8(line)?;
        Ok(RespFrame::SimpleString(ouput_str.to_string()))
    }

    fn parse_error(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        let line = Self::read_line(input, offset)?;
        let error_str = std::str::from_utf8(line)?;
        Ok(RespFrame::Error(error_str.to_string()))
    }

    fn parse_int(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        let line = Self::read_line(input, offset)?;
        let num_str = std::str::from_utf8(line)?;
        let num = num_str
            .parse::<i64>()
            .map_err(|_| FrameError::InvalidFormat)?;
        Ok(RespFrame::Integer(num))
    }

    fn parse_bulk_string(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        let line = Self::read_line(input, offset)?;
        let num_str = std::str::from_utf8(line)?;
        let len = num_str
            .parse::<i64>()
            .map_err(|_| FrameError::InvalidFormat)?;

        if len == -1 {
            return Ok(RespFrame::Null);
        }

        if len < -1 {
            return Err(FrameError::InvalidFormat);
        }

        let end_pos = *offset + len as usize;

        if (end_pos + 2) > input.len() {
            return Err(FrameError::Incomplete);
        }

        if &input[end_pos..end_pos + 2] != b"\r\n" {
            return Err(FrameError::InvalidFormat);
        }

        let output_bytes = &input[*offset..end_pos];
        *offset = end_pos + 2;

        Ok(RespFrame::BulkString(output_bytes.to_vec()))
    }

    fn parse_array(input: &[u8], offset: &mut usize) -> Result<RespFrame, FrameError> {
        let line = Self::read_line(input, offset)?;
        let num_str = std::str::from_utf8(line)?;
        let count = num_str
            .parse::<i64>()
            .map_err(|_| FrameError::InvalidFormat)?;

        if count == -1 {
            return Ok(RespFrame::Null);
        }

        if count < -1 {
            return Err(FrameError::InvalidFormat);
        }

        let usize_count = count as usize;
        let mut output_vec = Vec::with_capacity(usize_count);
        for _ in 0..usize_count {
            output_vec.push(Self::parse_prefix(input, offset)?);
        }
        Ok(RespFrame::Array(output_vec))
    }

    fn read_line<'a>(input: &'a [u8], offset: &mut usize) -> Result<&'a [u8], FrameError> {
        let pos = input[*offset..]
            .iter()
            .position(|&b| b == b'\r')
            .ok_or(FrameError::Incomplete)?;

        let abs_pos = *offset + pos;

        if abs_pos + 1 >= input.len() {
            return Err(FrameError::Incomplete);
        }

        if input[abs_pos + 1] != b'\n' {
            return Err(FrameError::InvalidFormat);
        }

        let line = &input[*offset..abs_pos];
        *offset = abs_pos + 2;
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_string() {
        let input = b"+OK\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::SimpleString("OK".to_string()));
    }

    #[test]
    fn parse_error() {
        let input = b"-Error\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Error("Error".to_string()));
    }

    #[test]
    fn parse_positive_integer() {
        let input = b":1000\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Integer(1000));
    }

    #[test]
    fn parse_negative_integer() {
        let input = b":-1000\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Integer(-1000));
    }

    #[test]
    fn parse_zero_integer() {
        let input = b":0\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Integer(0));
    }

    #[test]
    fn parse_valid_bulk_string() {
        let input = b"$5\r\nhello\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::BulkString(b"hello".to_vec()));
    }

    #[test]
    fn parse_empty_bulk_string() {
        let input = b"$0\r\n\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::BulkString(b"".to_vec()));
    }

    #[test]
    fn parse_null_bulk_string() {
        let input = b"$-1\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Null);
    }

    #[test]
    fn parse_empty_array() {
        let input = b"*0\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        assert_eq!(frame, RespFrame::Array(vec![]));
    }

    #[test]
    fn parse_array_with_mixed_types() {
        let input = b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n:100\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        let expected = RespFrame::Array(vec![
            RespFrame::BulkString(b"SET".to_vec()),
            RespFrame::BulkString(b"key".to_vec()),
            RespFrame::Integer(100),
        ]);
        assert_eq!(frame, expected);
    }

    #[test]
    fn parse_nested_array() {
        let input = b"*2\r\n*1\r\n+nested\r\n:42\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        let expected = RespFrame::Array(vec![
            RespFrame::Array(vec![RespFrame::SimpleString("nested".to_string())]),
            RespFrame::Integer(42),
        ]);
        assert_eq!(frame, expected);
    }

    #[test]
    fn parse_array_with_bulk_strings() {
        let input = b"*2\r\n$3\r\nGET\r\n$4\r\nkey1\r\n";
        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();
        let expected = RespFrame::Array(vec![
            RespFrame::BulkString(b"GET".to_vec()),
            RespFrame::BulkString(b"key1".to_vec()),
        ]);
        assert_eq!(frame, expected);
    }

    #[test]
    fn parse_null_array() {
        let input = b"*-1\r\n";

        let frame = RespFrame::parse_prefix(input, &mut 0).unwrap();

        assert_eq!(frame, RespFrame::Null);
    }

    #[test]
    fn parse_incomplete_data() {
        let input = b"$5\r\nhel";
        let result = RespFrame::parse_prefix(input, &mut 0);
        assert_eq!(result, Err(FrameError::Incomplete));
    }

    #[test]
    fn parse_invalid_type_prefix() {
        let input = b"#invalid\r\n";
        let result = RespFrame::parse_prefix(input, &mut 0);
        assert_eq!(result, Err(FrameError::InvalidType));
    }

    #[test]
    fn parse_invalid_integer_format() {
        let input = b":abc\r\n";
        let result = RespFrame::parse_prefix(input, &mut 0);
        assert_eq!(result, Err(FrameError::InvalidFormat));
    }

    #[test]
    fn parse_multiple_frames_with_offset() {
        let input = b"+PING\r\n+PONG\r\n";
        let mut offset = 0;

        let frame1 = RespFrame::parse_prefix(input, &mut offset).unwrap();
        assert_eq!(frame1, RespFrame::SimpleString("PING".to_string()));
        assert_eq!(offset, 7);

        let frame2 = RespFrame::parse_prefix(input, &mut offset).unwrap();
        assert_eq!(frame2, RespFrame::SimpleString("PONG".to_string()));
        assert_eq!(offset, 14);
    }
}
