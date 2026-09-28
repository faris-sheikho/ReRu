use std::io::{self, BufRead};

const MAX_BULK: usize = 64 * 1024 * 1024;
const MAX_ARGS: usize = 1024;

fn line<R: BufRead>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_until(b'\n', &mut bytes)?;
    if bytes.len() > 64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "header too long"));
    }
    if !bytes.ends_with(b"\r\n") {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "expected CRLF"));
    }
    bytes.truncate(bytes.len() - 2);
    Ok(bytes)
}

fn number(bytes: &[u8]) -> io::Result<usize> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<usize>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid length"))
}

pub fn read_command<R: BufRead>(reader: &mut R) -> io::Result<Option<Vec<Vec<u8>>>> {
    let first = reader.fill_buf()?;
    if first.is_empty() {
        return Ok(None);
    }
    let header = line(reader)?;
    if header.first() != Some(&b'*') {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "expected RESP array"));
    }
    let count = number(&header[1..])?;
    if count == 0 || count > MAX_ARGS {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid argument count"));
    }
    let mut args = Vec::with_capacity(count);
    for _ in 0..count {
        let header = line(reader)?;
        if header.first() != Some(&b'$') {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "expected bulk string"));
        }
        let len = number(&header[1..])?;
        if len > MAX_BULK {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bulk string too large"));
        }
        let mut value = vec![0; len];
        reader.read_exact(&mut value)?;
        let mut suffix = [0; 2];
        reader.read_exact(&mut suffix)?;
        if suffix != *b"\r\n" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "expected CRLF"));
        }
        args.push(value);
    }
    Ok(Some(args))
}

pub fn error(message: &str) -> Vec<u8> {
    format!("-ERR {message}\r\n").into_bytes()
}
