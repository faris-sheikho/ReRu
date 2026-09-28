mod common;

use std::io::{Read, Write};

fn command(parts: &[&[u8]]) -> Vec<u8> {
    let mut data = format!("*{}\r\n", parts.len()).into_bytes();
    for part in parts {
        data.extend(format!("${}\r\n", part.len()).as_bytes());
        data.extend(*part);
        data.extend(b"\r\n");
    }
    data
}

fn read_exact_response(stream: &mut std::net::TcpStream, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    stream.read_exact(&mut actual).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn binds_to_a_port() {
    let server = common::Server::start();
    assert!(server.connect().is_ok());
}

#[test]
fn responds_to_ping() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream.write_all(&command(&[b"PING"])).unwrap();
    read_exact_response(&mut stream, b"+PONG\r\n");
}

#[test]
fn responds_to_multiple_pings_on_one_connection() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    let mut pipelined = command(&[b"PING"]);
    pipelined.extend(command(&[b"ping"]));
    stream.write_all(&pipelined).unwrap();
    read_exact_response(&mut stream, b"+PONG\r\n+PONG\r\n");
    stream.write_all(&command(&[b"PING"])).unwrap();
    read_exact_response(&mut stream, b"+PONG\r\n");
}

#[test]
fn handles_clients_while_another_connection_is_idle() {
    let server = common::Server::start();
    let mut idle = server.connect().unwrap();
    let mut active = server.connect().unwrap();
    active.write_all(&command(&[b"PING"])).unwrap();
    read_exact_response(&mut active, b"+PONG\r\n");
    idle.write_all(&command(&[b"PING"])).unwrap();
    read_exact_response(&mut idle, b"+PONG\r\n");
}

#[test]
fn echoes_binary_data_across_split_tcp_writes() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    let request = command(&[b"eChO", b"hi\r\n\0there"]);
    let mid = request.len() / 2;
    stream.write_all(&request[..mid]).unwrap();
    stream.write_all(&request[mid..]).unwrap();
    read_exact_response(&mut stream, b"$10\r\nhi\r\n\0there\r\n");
}
