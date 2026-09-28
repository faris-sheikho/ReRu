mod protocol;

use std::collections::HashMap;
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

type Store = Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>;

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
            if command.len() != 3 {
                protocol::error("wrong number of arguments for 'set' command")
            } else {
                store.lock().unwrap().insert(command[1].clone(), command[2].clone());
                b"+OK\r\n".to_vec()
            }
        } else if command[0].eq_ignore_ascii_case(b"GET") {
            if command.len() != 2 {
                protocol::error("wrong number of arguments for 'get' command")
            } else {
                match store.lock().unwrap().get(&command[1]) {
                    Some(value) => protocol::bulk(value),
                    None => b"$-1\r\n".to_vec(),
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
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: reru [--port PORT]")),
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
