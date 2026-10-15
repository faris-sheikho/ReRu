use crate::{execute_command, protocol, Entry, Store, Value};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, BufReader, Seek, Write};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn command(output: &mut Vec<u8>, args: &[&[u8]]) {
    output.extend(format!("*{}\r\n", args.len()).as_bytes());
    for arg in args {
        output.extend(protocol::bulk(arg));
    }
}

fn append_value(output: &mut Vec<u8>, key: &[u8], entry: &Entry) -> io::Result<()> {
    match &entry.value {
        Value::String(value) => command(output, &[b"SET", key, value]),
        Value::List(items) => {
            for chunk in items.chunks(1022) {
                let mut args: Vec<&[u8]> = vec![b"RPUSH", key];
                args.extend(chunk.iter().map(|member| member.as_slice()));
                command(output, &args);
            }
        }
        Value::Hash(fields) => {
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_by(|left, right| left.0.cmp(right.0));
            for chunk in fields.chunks(511) {
                let mut args: Vec<&[u8]> = vec![b"HSET", key];
                for (field, value) in chunk {
                    args.push(field);
                    args.push(value);
                }
                command(output, &args);
            }
        }
        Value::Set(members) => {
            let mut members: Vec<_> = members.iter().collect();
            members.sort();
            for chunk in members.chunks(1022) {
                let mut args: Vec<&[u8]> = vec![b"SADD", key];
                args.extend(chunk.iter().map(|member| member.as_slice()));
                command(output, &args);
            }
        }
        Value::SortedSet(members) => {
            let members: Vec<_> = members.entries().collect();
            for chunk in members.chunks(511) {
                let scores: Vec<_> = chunk.iter().map(|(_, score)| score.to_string()).collect();
                let mut args: Vec<&[u8]> = vec![b"ZADD", key];
                for ((member, _), score) in chunk.iter().zip(&scores) {
                    args.push(score.as_bytes());
                    args.push(member);
                }
                command(output, &args);
            }
        }
        Value::Stream(stream) => {
            for item in &stream.entries {
                let id = item.id.to_string();
                let mut args: Vec<&[u8]> = vec![b"XADD", key, id.as_bytes()];
                args.extend(item.fields.iter().map(Vec::as_slice));
                command(output, &args);
            }
            if stream.last_generated_id
                > stream
                    .entries
                    .last()
                    .map_or(crate::streams::StreamId::default(), |item| item.id)
            {
                let id = stream.last_generated_id.to_string();
                command(output, &[b"XADD", key, id.as_bytes(), b"__reru_aof__", b""]);
                command(output, &[b"XDEL", key, id.as_bytes()]);
            }
        }
    }
    if let Some(deadline) = entry.expires_at {
        let absolute = SystemTime::now()
            .checked_add(deadline.saturating_duration_since(Instant::now()))
            .ok_or_else(|| invalid("AOF expiry out of range"))?;
        let milliseconds = absolute
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid("AOF expiry before epoch"))?
            .as_millis();
        let time = i64::try_from(milliseconds)
            .map_err(|_| invalid("AOF expiry out of range"))?
            .to_string();
        command(output, &[b"PEXPIREAT", key, time.as_bytes()]);
    }
    Ok(())
}

pub fn append(
    path: &Path,
    changed_keys: &[Vec<u8>],
    entries: &HashMap<Vec<u8>, Entry>,
) -> io::Result<()> {
    let mut output = Vec::new();
    command(&mut output, &[b"MULTI"]);
    let mut keys = changed_keys.to_vec();
    keys.sort();
    let now = Instant::now();
    for key in keys {
        command(&mut output, &[b"DEL", &key]);
        if let Some(entry) = entries
            .get(&key)
            .filter(|entry| entry.expires_at.is_none_or(|deadline| deadline > now))
        {
            append_value(&mut output, &key, entry)?;
        }
    }
    command(&mut output, &[b"EXEC"]);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let original_len = file.metadata()?.len();
    if let Err(error) = file.write_all(&output).and_then(|_| file.sync_data()) {
        let _ = file.set_len(original_len);
        return Err(error);
    }
    Ok(())
}

pub fn replay(path: &Path, store: &Store) -> io::Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let mut reader = BufReader::new(file);
    let mut committed = 0;
    let mut transaction: Option<Vec<Vec<Vec<u8>>>> = None;
    let mut partial_tail = false;
    loop {
        let command = match protocol::read_command(&mut reader) {
            Ok(Some(command)) => command,
            Ok(None) => break,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                partial_tail = true;
                break;
            }
            Err(error) => return Err(error),
        };
        if command.len() == 1 && command[0].eq_ignore_ascii_case(b"MULTI") {
            if transaction.replace(Vec::new()).is_some() {
                return Err(invalid("nested AOF transaction"));
            }
        } else if command.len() == 1 && command[0].eq_ignore_ascii_case(b"EXEC") {
            let batch = transaction
                .take()
                .ok_or_else(|| invalid("AOF EXEC without MULTI"))?;
            for item in batch {
                let reply = execute_command(&item, store, false);
                if reply.starts_with(b"-") {
                    return Err(invalid("AOF command failed during replay"));
                }
            }
            committed = reader.stream_position()?;
        } else if let Some(batch) = transaction.as_mut() {
            batch.push(command);
        } else {
            return Err(invalid("AOF command outside transaction"));
        }
    }
    if partial_tail || transaction.is_some() {
        reader.get_mut().set_len(committed)?;
    }
    store.pending_keys.lock().unwrap().clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn writes_resp_arrays() {
        let mut output = Vec::new();
        command(&mut output, &[b"SET", b"a", b"x\0y"]);
        assert_eq!(output, b"*3\r\n$3\r\nSET\r\n$1\r\na\r\n$3\r\nx\0y\r\n");
    }
}
