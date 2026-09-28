use std::io;
use std::net::TcpListener;

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
        stream?;
    }
    Ok(())
}
