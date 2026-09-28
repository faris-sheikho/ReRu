mod protocol;
mod streams;

use std::collections::HashMap;
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

enum Value {
    String(Vec<u8>),
    List(Vec<Vec<u8>>),
    Stream(streams::Stream),
}

struct Entry {
    value: Value,
    expires_at: Option<Instant>,
}

struct SharedStore {
    entries: Mutex<HashMap<Vec<u8>, Entry>>,
    changed: Condvar,
}

type Store = Arc<SharedStore>;

fn remove_expired(store: &mut HashMap<Vec<u8>, Entry>, key: &[u8]) {
    if store.get(key).is_some_and(|entry| {
        entry
            .expires_at
            .is_some_and(|expiry| Instant::now() >= expiry)
    }) {
        store.remove(key);
    }
}

fn set_expiry(command: &[Vec<u8>]) -> Result<Option<Instant>, Vec<u8>> {
    if command.len() == 3 {
        return Ok(None);
    }
    if command.len() != 5 {
        return Err(protocol::error("syntax error"));
    }
    let multiplier = if command[3].eq_ignore_ascii_case(b"PX") {
        1
    } else if command[3].eq_ignore_ascii_case(b"EX") {
        1000
    } else {
        return Err(protocol::error("syntax error"));
    };
    let amount = std::str::from_utf8(&command[4])
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .filter(|amount| *amount > 0)
        .and_then(|amount| amount.checked_mul(multiplier))
        .ok_or_else(|| protocol::error("invalid expire time in 'set' command"))?;
    let expires_at = Instant::now()
        .checked_add(Duration::from_millis(amount))
        .ok_or_else(|| protocol::error("invalid expire time in 'set' command"))?;
    Ok(Some(expires_at))
}

fn list_range(items: &[Vec<u8>], start: i64, stop: i64) -> Vec<u8> {
    let len = items.len() as i128;
    if len == 0 {
        return protocol::array(&[]);
    }
    let normalize = |index: i64| {
        if index < 0 {
            len + i128::from(index)
        } else {
            i128::from(index)
        }
    };
    let start = normalize(start).max(0);
    let stop = normalize(stop).min(len - 1);
    if start >= len || stop < start {
        protocol::array(&[])
    } else {
        protocol::array(&items[start as usize..=stop as usize])
    }
}

fn try_blpop(entries: &mut HashMap<Vec<u8>, Entry>, keys: &[Vec<u8>]) -> Option<Vec<u8>> {
    for key in keys {
        remove_expired(entries, key);
        match entries.get_mut(key) {
            Some(Entry {
                value: Value::List(items),
                ..
            }) => {
                let value = items.remove(0);
                if items.is_empty() {
                    entries.remove(key);
                }
                return Some(protocol::array(&[key.clone(), value]));
            }
            Some(_) => {
                return Some(
                    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                        .to_vec(),
                );
            }
            None => {}
        }
    }
    None
}

fn blpop_deadline(timeout: &[u8]) -> Result<Option<Instant>, Vec<u8>> {
    let seconds = std::str::from_utf8(timeout)
        .ok()
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .ok_or_else(|| protocol::error("timeout is not a float or out of range"))?;
    if seconds == 0.0 {
        return Ok(None);
    }
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| protocol::error("timeout is not a float or out of range"))?;
    Instant::now()
        .checked_add(duration)
        .map(Some)
        .ok_or_else(|| protocol::error("timeout is not a float or out of range"))
}

struct XReadOptions<'a> {
    keys: &'a [Vec<u8>],
    ids: &'a [Vec<u8>],
    count: usize,
    block: Option<Duration>,
}

fn parse_xread(command: &[Vec<u8>]) -> Result<XReadOptions<'_>, Vec<u8>> {
    let mut index = 1;
    let mut count = usize::MAX;
    let mut block = None;
    while index < command.len() && !command[index].eq_ignore_ascii_case(b"STREAMS") {
        if index + 1 >= command.len() {
            return Err(protocol::error("syntax error"));
        }
        let number = std::str::from_utf8(&command[index + 1])
            .ok()
            .and_then(|text| text.parse::<usize>().ok())
            .ok_or_else(|| protocol::error("value is not an integer or out of range"))?;
        if command[index].eq_ignore_ascii_case(b"COUNT") && number > 0 {
            count = number;
        } else if command[index].eq_ignore_ascii_case(b"BLOCK") {
            block = Some(Duration::from_millis(number as u64));
        } else {
            return Err(protocol::error("syntax error"));
        }
        index += 2;
    }
    if index == command.len() {
        return Err(protocol::error("syntax error"));
    }
    let rest = &command[index + 1..];
    if rest.is_empty() || !rest.len().is_multiple_of(2) {
        return Err(protocol::error("Unbalanced XREAD list of streams"));
    }
    let half = rest.len() / 2;
    Ok(XReadOptions {
        keys: &rest[..half],
        ids: &rest[half..],
        count,
        block,
    })
}

fn try_xread(
    entries: &mut HashMap<Vec<u8>, Entry>,
    keys: &[Vec<u8>],
    ids: &[streams::StreamId],
    count: usize,
) -> Result<Option<Vec<u8>>, Vec<u8>> {
    let mut groups = Vec::new();
    for (key, id) in keys.iter().zip(ids) {
        remove_expired(entries, key);
        match entries.get(key).map(|entry| &entry.value) {
            Some(Value::Stream(stream)) => {
                let found = stream.after(*id, count);
                if !found.is_empty() {
                    groups.push((key, protocol::stream_entries(&found)));
                }
            }
            Some(_) => {
                return Err(
                    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                        .to_vec(),
                )
            }
            None => {}
        }
    }
    if groups.is_empty() {
        return Ok(None);
    }
    let mut response = format!("*{}\r\n", groups.len()).into_bytes();
    for (key, encoded_entries) in groups {
        response.extend_from_slice(b"*2\r\n");
        response.extend(protocol::bulk(key));
        response.extend(encoded_entries);
    }
    Ok(Some(response))
}

fn xread_ids(
    entries: &mut HashMap<Vec<u8>, Entry>,
    keys: &[Vec<u8>],
    raw_ids: &[Vec<u8>],
) -> Result<Vec<streams::StreamId>, Vec<u8>> {
    keys.iter()
        .zip(raw_ids)
        .map(|(key, raw)| {
            if raw != b"$" {
                return streams::StreamId::parse_query(raw).map_err(protocol::error);
            }
            remove_expired(entries, key);
            match entries.get(key).map(|entry| &entry.value) {
                Some(Value::Stream(stream)) => Ok(stream.last_id()),
                Some(_) => Err(
                    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                        .to_vec(),
                ),
                None => Ok(streams::StreamId {
                    milliseconds: 0,
                    sequence: 0,
                }),
            }
        })
        .collect()
}

fn handle_client(mut stream: TcpStream, store: Store) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    while let Some(command) = protocol::read_command(&mut reader)? {
        let response = if command[0].eq_ignore_ascii_case(b"PING") {
            b"+PONG\r\n".to_vec()
        } else if command[0].eq_ignore_ascii_case(b"ECHO") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'echo' command")
            } else {
                protocol::bulk(&command[1])
            }
        } else if command[0].eq_ignore_ascii_case(b"SET") {
            if command.len() < 3 {
                protocol::error("wrong number of arguments for 'set' command")
            } else {
                match set_expiry(&command) {
                    Ok(expires_at) => {
                        store.entries.lock().unwrap().insert(
                            command[1].clone(),
                            Entry {
                                value: Value::String(command[2].clone()),
                                expires_at,
                            },
                        );
                        b"+OK\r\n".to_vec()
                    }
                    Err(error) => error,
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"GET") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'get' command")
            } else {
                let mut store = store.entries.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get(&command[1]).map(|entry| &entry.value) {
                    Some(Value::String(value)) => protocol::bulk(value),
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => b"$-1\r\n".to_vec(),
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"INCR") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'incr' command")
            } else {
                let mut entries = store.entries.lock().unwrap();
                remove_expired(&mut entries, &command[1]);
                match entries.get_mut(&command[1]) {
                    Some(Entry {
                        value: Value::String(value),
                        ..
                    }) => match std::str::from_utf8(value)
                        .ok()
                        .and_then(|text| text.parse::<i64>().ok())
                        .and_then(|number| number.checked_add(1))
                    {
                        Some(number) => {
                            *value = number.to_string().into_bytes();
                            format!(":{number}\r\n").into_bytes()
                        }
                        None => protocol::error("value is not an integer or out of range"),
                    },
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => {
                        entries.insert(
                            command[1].clone(),
                            Entry {
                                value: Value::String(b"1".to_vec()),
                                expires_at: None,
                            },
                        );
                        b":1\r\n".to_vec()
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"RPUSH") {
            if command.len() < 3 {
                protocol::error("wrong number of arguments for 'rpush' command")
            } else {
                let changed = &store.changed;
                let mut store = store.entries.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get_mut(&command[1]) {
                    Some(Entry {
                        value: Value::List(items),
                        ..
                    }) => {
                        items.extend_from_slice(&command[2..]);
                        changed.notify_all();
                        format!(":{}\r\n", items.len()).into_bytes()
                    }
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => {
                        store.insert(
                            command[1].clone(),
                            Entry {
                                value: Value::List(command[2..].to_vec()),
                                expires_at: None,
                            },
                        );
                        changed.notify_all();
                        format!(":{}\r\n", command.len() - 2).into_bytes()
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"LRANGE") {
            if command.len() != 4 {
                protocol::error("wrong number of arguments for 'lrange' command")
            } else {
                let indexes = [command[2].as_slice(), command[3].as_slice()].map(|bytes| {
                    std::str::from_utf8(bytes)
                        .ok()
                        .and_then(|s| s.parse::<i64>().ok())
                });
                match indexes {
                    [Some(start), Some(stop)] => {
                        let mut store = store.entries.lock().unwrap();
                        remove_expired(&mut store, &command[1]);
                        match store.get(&command[1]).map(|entry| &entry.value) {
                            Some(Value::List(items)) => list_range(items, start, stop),
                            Some(_) => b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".to_vec(),
                            None => protocol::array(&[]),
                        }
                    }
                    _ => protocol::error("value is not an integer or out of range"),
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"LPUSH") {
            if command.len() < 3 {
                protocol::error("wrong number of arguments for 'lpush' command")
            } else {
                let changed = &store.changed;
                let mut store = store.entries.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get_mut(&command[1]) {
                    Some(Entry {
                        value: Value::List(items),
                        ..
                    }) => {
                        items.splice(0..0, command[2..].iter().rev().cloned());
                        changed.notify_all();
                        format!(":{}\r\n", items.len()).into_bytes()
                    }
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => {
                        store.insert(
                            command[1].clone(),
                            Entry {
                                value: Value::List(command[2..].iter().rev().cloned().collect()),
                                expires_at: None,
                            },
                        );
                        changed.notify_all();
                        format!(":{}\r\n", command.len() - 2).into_bytes()
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"LLEN") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'llen' command")
            } else {
                let mut store = store.entries.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get(&command[1]).map(|entry| &entry.value) {
                    Some(Value::List(items)) => format!(":{}\r\n", items.len()).into_bytes(),
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => b":0\r\n".to_vec(),
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"LPOP") {
            if command.len() != 2 && command.len() != 3 {
                protocol::error("wrong number of arguments for 'lpop' command")
            } else {
                let count = if command.len() == 3 {
                    std::str::from_utf8(&command[2])
                        .ok()
                        .and_then(|s| s.parse::<usize>().ok())
                } else {
                    Some(1)
                };
                match count {
                    None => protocol::error("value is not an integer or out of range"),
                    Some(count) => {
                        let mut store = store.entries.lock().unwrap();
                        remove_expired(&mut store, &command[1]);
                        match store.get_mut(&command[1]) {
                            Some(Entry { value: Value::List(items), .. }) => {
                                if count == 0 {
                                    b"*-1\r\n".to_vec()
                                } else {
                                    let values: Vec<_> = items.drain(..count.min(items.len())).collect();
                                    let empty = items.is_empty();
                                    if empty {
                                        store.remove(&command[1]);
                                    }
                                    if command.len() == 2 {
                                        protocol::bulk(&values[0])
                                    } else {
                                        protocol::array(&values)
                                    }
                                }
                            }
                            Some(_) => b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".to_vec(),
                            None => b"$-1\r\n".to_vec(),
                        }
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"BLPOP") {
            if command.len() < 3 {
                protocol::error("wrong number of arguments for 'blpop' command")
            } else {
                match blpop_deadline(command.last().unwrap()) {
                    Err(error) => error,
                    Ok(deadline) => {
                        let keys = &command[1..command.len() - 1];
                        let mut entries = store.entries.lock().unwrap();
                        loop {
                            if let Some(response) = try_blpop(&mut entries, keys) {
                                break response;
                            }
                            match deadline {
                                Some(deadline) => {
                                    let remaining =
                                        deadline.saturating_duration_since(Instant::now());
                                    if remaining.is_zero() {
                                        break b"*-1\r\n".to_vec();
                                    }
                                    entries =
                                        store.changed.wait_timeout(entries, remaining).unwrap().0;
                                }
                                None => entries = store.changed.wait(entries).unwrap(),
                            }
                        }
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"TYPE") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'type' command")
            } else {
                let mut entries = store.entries.lock().unwrap();
                remove_expired(&mut entries, &command[1]);
                match entries.get(&command[1]).map(|entry| &entry.value) {
                    Some(Value::String(_)) => b"+string\r\n".to_vec(),
                    Some(Value::List(_)) => b"+list\r\n".to_vec(),
                    Some(Value::Stream(_)) => b"+stream\r\n".to_vec(),
                    None => b"+none\r\n".to_vec(),
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"XADD") {
            if command.len() < 5 || command.len() % 2 == 0 {
                protocol::error("wrong number of arguments for 'xadd' command")
            } else {
                let mut entries = store.entries.lock().unwrap();
                remove_expired(&mut entries, &command[1]);
                let fields = command[3..].to_vec();
                let result = match entries.get_mut(&command[1]) {
                    Some(Entry {
                        value: Value::Stream(stream),
                        ..
                    }) => stream.append(&command[2], fields).map_err(protocol::error),
                    Some(_) => Err(
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec(),
                    ),
                    None => {
                        let mut new_stream = streams::Stream::default();
                        let id = new_stream.append(&command[2], fields);
                        if id.is_ok() {
                            entries.insert(
                                command[1].clone(),
                                Entry {
                                    value: Value::Stream(new_stream),
                                    expires_at: None,
                                },
                            );
                        }
                        id.map_err(protocol::error)
                    }
                };
                match result {
                    Ok(id) => {
                        store.changed.notify_all();
                        protocol::bulk(id.to_string().as_bytes())
                    }
                    Err(response) => response,
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"XRANGE") {
            if command.len() != 4
                && (command.len() != 6 || !command[4].eq_ignore_ascii_case(b"COUNT"))
            {
                protocol::error("wrong number of arguments for 'xrange' command")
            } else {
                let count = if command.len() == 6 {
                    std::str::from_utf8(&command[5])
                        .ok()
                        .and_then(|s| s.parse::<usize>().ok())
                } else {
                    Some(usize::MAX)
                };
                match count {
                    None => protocol::error("value is not an integer or out of range"),
                    Some(count) => {
                        let mut entries = store.entries.lock().unwrap();
                        remove_expired(&mut entries, &command[1]);
                        match entries.get(&command[1]).map(|entry| &entry.value) {
                            Some(Value::Stream(stream)) => match stream.range(&command[2], &command[3], Some(count)) {
                                Ok(found) => protocol::stream_entries(&found),
                                Err(message) => protocol::error(message),
                            },
                            Some(_) => b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".to_vec(),
                            None => protocol::array(&[]),
                        }
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"XREAD") {
            match parse_xread(&command) {
                Err(error) => error,
                Ok(options) => {
                    let mut entries = store.entries.lock().unwrap();
                    match xread_ids(&mut entries, options.keys, options.ids) {
                        Err(error) => error,
                        Ok(ids) => {
                            let deadline = match options.block {
                                Some(duration) if !duration.is_zero() => {
                                    Instant::now().checked_add(duration)
                                }
                                _ => None,
                            };
                            loop {
                                match try_xread(&mut entries, options.keys, &ids, options.count) {
                                    Err(error) => break error,
                                    Ok(Some(response)) => break response,
                                    Ok(None) => {}
                                }
                                match options.block {
                                    None => break b"*-1\r\n".to_vec(),
                                    Some(duration) if duration.is_zero() => {
                                        entries = store.changed.wait(entries).unwrap()
                                    }
                                    Some(_) => {
                                        let remaining =
                                            deadline.map_or(Duration::ZERO, |deadline| {
                                                deadline.saturating_duration_since(Instant::now())
                                            });
                                        if remaining.is_zero() {
                                            break b"*-1\r\n".to_vec();
                                        }
                                        entries = store
                                            .changed
                                            .wait_timeout(entries, remaining)
                                            .unwrap()
                                            .0;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"XLEN") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'xlen' command")
            } else {
                let mut entries = store.entries.lock().unwrap();
                remove_expired(&mut entries, &command[1]);
                match entries.get(&command[1]).map(|entry| &entry.value) {
                    Some(Value::Stream(stream)) => {
                        format!(":{}\r\n", stream.entries.len()).into_bytes()
                    }
                    Some(_) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => b":0\r\n".to_vec(),
                }
            }
        } else {
            protocol::error("unknown command")
        };
        stream.write_all(&response)?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = match args.as_slice() {
        [_] => 6379,
        [_, flag, value] if flag == "--port" => value.parse::<u16>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "--port must be a valid port")
        })?,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: reru [--port PORT]",
            ))
        }
    };

    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let store: Store = Arc::new(SharedStore {
        entries: Mutex::new(HashMap::new()),
        changed: Condvar::new(),
    });
    eprintln!("ReRu listening on 127.0.0.1:{port}");
    for stream in listener.incoming() {
        let stream = stream?;
        let store = Arc::clone(&store);
        std::thread::spawn(move || {
            if let Err(error) = handle_client(stream, store) {
                eprintln!("client error: {error}");
            }
        });
    }
    Ok(())
}
