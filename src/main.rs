mod protocol;

use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
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
    eprintln!("ReRu listening on 127.0.0.1:{port}");
    for stream in listener.incoming() {
        let stream = stream?;
        std::thread::spawn(move || {
            if let Err(error) = handle_client(stream) {
                eprintln!("client error: {error}");
            }
        });
    }
    Ok(())
}
