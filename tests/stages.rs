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

#[test]
fn set_and_get_share_data_across_clients() {
    let server = common::Server::start();
    let mut writer = server.connect().unwrap();
    let mut reader = server.connect().unwrap();
    reader.write_all(&command(&[b"GET", b"missing"])).unwrap();
    read_exact_response(&mut reader, b"$-1\r\n");
    writer
        .write_all(&command(&[b"SET", b"key", b"a\0b"]))
        .unwrap();
    read_exact_response(&mut writer, b"+OK\r\n");
    reader.write_all(&command(&[b"GET", b"key"])).unwrap();
    read_exact_response(&mut reader, b"$3\r\na\0b\r\n");
    writer
        .write_all(&command(&[b"SET", b"key", b"new"]))
        .unwrap();
    read_exact_response(&mut writer, b"+OK\r\n");
    reader.write_all(&command(&[b"GET", b"key"])).unwrap();
    read_exact_response(&mut reader, b"$3\r\nnew\r\n");
}

#[test]
fn set_px_expires_and_overwriting_clears_expiry() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"SET", b"temporary", b"one", b"PX", b"250"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    stream.write_all(&command(&[b"GET", b"temporary"])).unwrap();
    read_exact_response(&mut stream, b"$3\r\none\r\n");
    std::thread::sleep(std::time::Duration::from_millis(320));
    stream.write_all(&command(&[b"GET", b"temporary"])).unwrap();
    read_exact_response(&mut stream, b"$-1\r\n");
    stream
        .write_all(&command(&[b"SET", b"temporary", b"two", b"PX", b"250"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    stream
        .write_all(&command(&[b"SET", b"temporary", b"permanent"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    std::thread::sleep(std::time::Duration::from_millis(320));
    stream.write_all(&command(&[b"GET", b"temporary"])).unwrap();
    read_exact_response(&mut stream, b"$9\r\npermanent\r\n");
}

#[test]
fn rpush_creates_a_list() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"first"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream.write_all(&command(&[b"GET", b"items"])).unwrap();
    read_exact_response(
        &mut stream,
        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n",
    );
}

#[test]
fn rpush_appends_to_an_existing_list_across_clients() {
    let server = common::Server::start();
    let mut first = server.connect().unwrap();
    let mut second = server.connect().unwrap();
    first
        .write_all(&command(&[b"RPUSH", b"items", b"first"]))
        .unwrap();
    read_exact_response(&mut first, b":1\r\n");
    second
        .write_all(&command(&[b"RPUSH", b"items", b"second"]))
        .unwrap();
    read_exact_response(&mut second, b":2\r\n");
    first
        .write_all(&command(&[b"RPUSH", b"items", b"third"]))
        .unwrap();
    read_exact_response(&mut first, b":3\r\n");
    second
        .write_all(&command(&[b"SET", b"string", b"value"]))
        .unwrap();
    read_exact_response(&mut second, b"+OK\r\n");
    first
        .write_all(&command(&[b"RPUSH", b"string", b"bad"]))
        .unwrap();
    read_exact_response(
        &mut first,
        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n",
    );
}

#[test]
fn rpush_accepts_multiple_elements_atomically() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"one", b"two", b"three"]))
        .unwrap();
    read_exact_response(&mut stream, b":3\r\n");
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"four", b"five"]))
        .unwrap();
    read_exact_response(&mut stream, b":5\r\n");
    stream.write_all(&command(&[b"RPUSH", b"items"])).unwrap();
    read_exact_response(
        &mut stream,
        b"-ERR wrong number of arguments for 'rpush' command\r\n",
    );
}

#[test]
fn lrange_returns_inclusive_positive_range() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"LRANGE", b"missing", b"0", b"9"]))
        .unwrap();
    read_exact_response(&mut stream, b"*0\r\n");
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"a", b"b", b"c"]))
        .unwrap();
    read_exact_response(&mut stream, b":3\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"1", b"9"]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n$1\r\nb\r\n$1\r\nc\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"9", b"12"]))
        .unwrap();
    read_exact_response(&mut stream, b"*0\r\n");
}

#[test]
fn lrange_supports_negative_indexes() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"a", b"b", b"c"]))
        .unwrap();
    read_exact_response(&mut stream, b":3\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"0", b"-1"]))
        .unwrap();
    read_exact_response(&mut stream, b"*3\r\n$1\r\na\r\n$1\r\nb\r\n$1\r\nc\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"-2", b"-1"]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n$1\r\nb\r\n$1\r\nc\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"-99", b"-4"]))
        .unwrap();
    read_exact_response(&mut stream, b"*0\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"-99", b"99"]))
        .unwrap();
    read_exact_response(&mut stream, b"*3\r\n$1\r\na\r\n$1\r\nb\r\n$1\r\nc\r\n");
}

#[test]
fn lpush_prepends_in_redis_order() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"a", b"b"]))
        .unwrap();
    read_exact_response(&mut stream, b":2\r\n");
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"c", b"d"]))
        .unwrap();
    read_exact_response(&mut stream, b":4\r\n");
    stream
        .write_all(&command(&[b"LRANGE", b"items", b"0", b"-1"]))
        .unwrap();
    read_exact_response(
        &mut stream,
        b"*4\r\n$1\r\nd\r\n$1\r\nc\r\n$1\r\nb\r\n$1\r\na\r\n",
    );
}

#[test]
fn llen_reports_missing_and_existing_lists() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream.write_all(&command(&[b"LLEN", b"missing"])).unwrap();
    read_exact_response(&mut stream, b":0\r\n");
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"a", b"b"]))
        .unwrap();
    read_exact_response(&mut stream, b":2\r\n");
    stream.write_all(&command(&[b"LLEN", b"items"])).unwrap();
    read_exact_response(&mut stream, b":2\r\n");
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"c"]))
        .unwrap();
    read_exact_response(&mut stream, b":3\r\n");
    stream.write_all(&command(&[b"LLEN", b"items"])).unwrap();
    read_exact_response(&mut stream, b":3\r\n");
}
