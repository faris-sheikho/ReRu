mod protocol;

use std::collections::HashMap;
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

enum Value {
    String(Vec<u8>),
    List(Vec<Vec<u8>>),
}

struct Entry {
    value: Value,
    expires_at: Option<Instant>,
}

type Store = Arc<Mutex<HashMap<Vec<u8>, Entry>>>;

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
                        store.lock().unwrap().insert(
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
                let mut store = store.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get(&command[1]).map(|entry| &entry.value) {
                    Some(Value::String(value)) => protocol::bulk(value),
                    Some(Value::List(_)) => {
                        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                            .to_vec()
                    }
                    None => b"$-1\r\n".to_vec(),
                }
            }
        } else if command[0].eq_ignore_ascii_case(b"RPUSH") {
            if command.len() != 3 {
                protocol::error("wrong number of arguments for 'rpush' command")
            } else {
                let mut store = store.lock().unwrap();
                remove_expired(&mut store, &command[1]);
                match store.get_mut(&command[1]) {
                    Some(Entry {
                        value: Value::List(items),
                        ..
                    }) => {
                        items.push(command[2].clone());
                        format!(":{}\r\n", items.len()).into_bytes()
                    }
                    Some(Entry {
                        value: Value::String(_),
                        ..
                    }) => b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                        .to_vec(),
                    None => {
                        store.insert(
                            command[1].clone(),
                            Entry {
                                value: Value::List(vec![command[2].clone()]),
                                expires_at: None,
                            },
                        );
                        b":1\r\n".to_vec()
                    }
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
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
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
