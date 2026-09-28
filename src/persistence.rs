use crate::{streams, Entry, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"RERUSNP1";
const MAX_SNAPSHOT: usize = 256 * 1024 * 1024;
const MAX_ITEMS: usize = 1_000_000;

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid ReRu snapshot")
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend(value.to_be_bytes());
}

fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> io::Result<()> {
    put_u64(output, u64::try_from(bytes.len()).map_err(|_| invalid())?);
    output.extend(bytes);
    if output.len() > MAX_SNAPSHOT {
        return Err(invalid());
    }
    Ok(())
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> io::Result<u64> {
    let mut bytes = [0; 8];
    cursor.read_exact(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

fn read_bytes(cursor: &mut Cursor<&[u8]>) -> io::Result<Vec<u8>> {
    let len = usize::try_from(read_u64(cursor)?).map_err(|_| invalid())?;
    if len > MAX_SNAPSHOT
        || len
            > cursor
                .get_ref()
                .len()
                .saturating_sub(cursor.position() as usize)
    {
        return Err(invalid());
    }
    let mut bytes = vec![0; len];
    cursor.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn encode(entries: &HashMap<Vec<u8>, Entry>) -> io::Result<Vec<u8>> {
    let mut output = MAGIC.to_vec();
    put_u64(
        &mut output,
        u64::try_from(entries.len()).map_err(|_| invalid())?,
    );
    let now_instant = Instant::now();
    let now_system = SystemTime::now();
    for (key, entry) in entries {
        put_bytes(&mut output, key)?;
        let expiry = match entry.expires_at {
            None => u64::MAX,
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(now_instant);
                let absolute = now_system.checked_add(remaining).ok_or_else(invalid)?;
                u64::try_from(
                    absolute
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| invalid())?
                        .as_millis(),
                )
                .map_err(|_| invalid())?
            }
        };
        put_u64(&mut output, expiry);
        match &entry.value {
            Value::String(value) => {
                output.push(1);
                put_bytes(&mut output, value)?;
            }
            Value::List(items) => {
                output.push(2);
                put_u64(
                    &mut output,
                    u64::try_from(items.len()).map_err(|_| invalid())?,
                );
                for item in items {
                    put_bytes(&mut output, item)?;
                }
            }
            Value::Stream(stream) => {
                output.push(3);
                put_u64(
                    &mut output,
                    u64::try_from(stream.entries.len()).map_err(|_| invalid())?,
                );
                for item in &stream.entries {
                    put_u64(&mut output, item.id.milliseconds);
                    put_u64(&mut output, item.id.sequence);
                    put_u64(
                        &mut output,
                        u64::try_from(item.fields.len()).map_err(|_| invalid())?,
                    );
                    for field in &item.fields {
                        put_bytes(&mut output, field)?;
                    }
                }
            }
        }
    }
    if output.len() > MAX_SNAPSHOT {
        return Err(invalid());
    }
    Ok(output)
}

pub fn decode(bytes: &[u8]) -> io::Result<HashMap<Vec<u8>, Entry>> {
    if bytes.len() > MAX_SNAPSHOT || !bytes.starts_with(MAGIC) {
        return Err(invalid());
    }
    let mut cursor = Cursor::new(bytes);
    cursor.set_position(MAGIC.len() as u64);
    let count = usize::try_from(read_u64(&mut cursor)?).map_err(|_| invalid())?;
    if count > MAX_ITEMS {
        return Err(invalid());
    }
    let now_system = SystemTime::now();
    let now_instant = Instant::now();
    let mut entries = HashMap::new();
    for _ in 0..count {
        let key = read_bytes(&mut cursor)?;
        let expiry = read_u64(&mut cursor)?;
        let mut kind = [0];
        cursor.read_exact(&mut kind)?;
        let value = match kind[0] {
            1 => Value::String(read_bytes(&mut cursor)?),
            2 => {
                let count = usize::try_from(read_u64(&mut cursor)?).map_err(|_| invalid())?;
                if count > MAX_ITEMS {
                    return Err(invalid());
                }
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(read_bytes(&mut cursor)?);
                }
                Value::List(items)
            }
            3 => {
                let count = usize::try_from(read_u64(&mut cursor)?).map_err(|_| invalid())?;
                if count > MAX_ITEMS {
                    return Err(invalid());
                }
                let mut stream = streams::Stream::default();
                for _ in 0..count {
                    let id = streams::StreamId {
                        milliseconds: read_u64(&mut cursor)?,
                        sequence: read_u64(&mut cursor)?,
                    };
                    let fields_count =
                        usize::try_from(read_u64(&mut cursor)?).map_err(|_| invalid())?;
                    if fields_count > MAX_ITEMS || fields_count % 2 != 0 {
                        return Err(invalid());
                    }
                    let mut fields = Vec::with_capacity(fields_count);
                    for _ in 0..fields_count {
                        fields.push(read_bytes(&mut cursor)?);
                    }
                    if stream.entries.last().is_some_and(|last| id <= last.id) {
                        return Err(invalid());
                    }
                    stream.entries.push(streams::StreamEntry { id, fields });
                }
                Value::Stream(stream)
            }
            _ => return Err(invalid()),
        };
        let expires_at = if expiry == u64::MAX {
            None
        } else {
            let deadline = UNIX_EPOCH
                .checked_add(Duration::from_millis(expiry))
                .ok_or_else(invalid)?;
            match deadline.duration_since(now_system) {
                Ok(remaining) => Some(now_instant.checked_add(remaining).ok_or_else(invalid)?),
                Err(_) => continue,
            }
        };
        if entries.insert(key, Entry { value, expires_at }).is_some() {
            return Err(invalid());
        }
    }
    if cursor.position() as usize != bytes.len() {
        return Err(invalid());
    }
    Ok(entries)
}

pub fn load(path: &Path) -> io::Result<HashMap<Vec<u8>, Entry>> {
    let backup = path.with_extension("bak");
    for candidate in [path, backup.as_path()] {
        match fs::read(candidate) {
            Ok(bytes) => match decode(&bytes) {
                Ok(entries) => return Ok(entries),
                Err(error) if candidate == backup.as_path() => return Err(error),
                Err(_) => {}
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if path.exists() {
        return Err(invalid());
    }
    Ok(HashMap::new())
}

pub fn save(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAX_SNAPSHOT {
        return Err(invalid());
    }
    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error);
    }
    Ok(())
}
