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

#[test]
fn lpop_removes_front_and_deletes_empty_list_key() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream.write_all(&command(&[b"LPOP", b"items"])).unwrap();
    read_exact_response(&mut stream, b"$-1\r\n");
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"a", b"b"]))
        .unwrap();
    read_exact_response(&mut stream, b":2\r\n");
    stream.write_all(&command(&[b"LPOP", b"items"])).unwrap();
    read_exact_response(&mut stream, b"$1\r\na\r\n");
    stream.write_all(&command(&[b"LPOP", b"items"])).unwrap();
    read_exact_response(&mut stream, b"$1\r\nb\r\n");
    stream.write_all(&command(&[b"LLEN", b"items"])).unwrap();
    read_exact_response(&mut stream, b":0\r\n");
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"new"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
}

#[test]
fn lpop_count_returns_multiple_items() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"RPUSH", b"items", b"a", b"b", b"c"]))
        .unwrap();
    read_exact_response(&mut stream, b":3\r\n");
    stream
        .write_all(&command(&[b"LPOP", b"items", b"2"]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n$1\r\na\r\n$1\r\nb\r\n");
    stream
        .write_all(&command(&[b"LPOP", b"items", b"0"]))
        .unwrap();
    read_exact_response(&mut stream, b"*-1\r\n");
    stream
        .write_all(&command(&[b"LPOP", b"items", b"9"]))
        .unwrap();
    read_exact_response(&mut stream, b"*1\r\n$1\r\nc\r\n");
    stream.write_all(&command(&[b"LLEN", b"items"])).unwrap();
    read_exact_response(&mut stream, b":0\r\n");
}

#[test]
fn blpop_waits_for_a_push_from_another_client() {
    let server = common::Server::start();
    let mut blocked = server.connect().unwrap();
    let mut writer = server.connect().unwrap();
    blocked
        .write_all(&command(&[b"BLPOP", b"items", b"0"]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    writer
        .write_all(&command(&[b"RPUSH", b"items", b"hello"]))
        .unwrap();
    read_exact_response(&mut writer, b":1\r\n");
    read_exact_response(&mut blocked, b"*2\r\n$5\r\nitems\r\n$5\r\nhello\r\n");
    writer.write_all(&command(&[b"LLEN", b"items"])).unwrap();
    read_exact_response(&mut writer, b":0\r\n");
}

#[test]
fn blpop_prefers_the_first_nonempty_key() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"RPUSH", b"second", b"b"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream
        .write_all(&command(&[b"RPUSH", b"first", b"a"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream
        .write_all(&command(&[b"BLPOP", b"first", b"second", b"0"]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n$5\r\nfirst\r\n$1\r\na\r\n");
}

#[test]
fn blpop_times_out_with_null_array() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    let start = std::time::Instant::now();
    stream
        .write_all(&command(&[b"BLPOP", b"missing", b"0.15"]))
        .unwrap();
    read_exact_response(&mut stream, b"*-1\r\n");
    assert!(start.elapsed() >= std::time::Duration::from_millis(100));
    stream.write_all(&command(&[b"PING"])).unwrap();
    read_exact_response(&mut stream, b"+PONG\r\n");
}

#[test]
fn blpop_wakes_before_timeout() {
    let server = common::Server::start();
    let mut blocked = server.connect().unwrap();
    let mut writer = server.connect().unwrap();
    blocked
        .write_all(&command(&[b"BLPOP", b"items", b"1.5"]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    writer
        .write_all(&command(&[b"LPUSH", b"items", b"ready"]))
        .unwrap();
    read_exact_response(&mut writer, b":1\r\n");
    read_exact_response(&mut blocked, b"*2\r\n$5\r\nitems\r\n$5\r\nready\r\n");
}

#[test]
fn type_reports_existing_and_missing_key_types() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream.write_all(&command(&[b"TYPE", b"missing"])).unwrap();
    read_exact_response(&mut stream, b"+none\r\n");
    stream
        .write_all(&command(&[b"SET", b"text", b"value"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    stream.write_all(&command(&[b"TYPE", b"text"])).unwrap();
    read_exact_response(&mut stream, b"+string\r\n");
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"one"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream.write_all(&command(&[b"TYPE", b"items"])).unwrap();
    read_exact_response(&mut stream, b"+list\r\n");
}

#[test]
fn xadd_creates_stream_and_validates_ids() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"XADD", b"events", b"1-0", b"name", b"first"]))
        .unwrap();
    read_exact_response(&mut stream, b"$3\r\n1-0\r\n");
    stream.write_all(&command(&[b"TYPE", b"events"])).unwrap();
    read_exact_response(&mut stream, b"+stream\r\n");
    stream
        .write_all(&command(&[b"XADD", b"events", b"1-*", b"name", b"second"]))
        .unwrap();
    read_exact_response(&mut stream, b"$3\r\n1-1\r\n");
    stream
        .write_all(&command(&[
            b"XADD",
            b"events",
            b"1-1",
            b"name",
            b"duplicate",
        ]))
        .unwrap();
    read_exact_response(
        &mut stream,
        b"-ERR The ID specified in XADD is equal or smaller than the target stream top item\r\n",
    );
}

#[test]
fn xrange_returns_stream_entries_and_respects_bounds() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"XRANGE", b"missing", b"-", b"+"]))
        .unwrap();
    read_exact_response(&mut stream, b"*0\r\n");
    stream
        .write_all(&command(&[b"XADD", b"events", b"1-0", b"f", b"a"]))
        .unwrap();
    read_exact_response(&mut stream, b"$3\r\n1-0\r\n");
    stream
        .write_all(&command(&[b"XADD", b"events", b"2-0", b"f", b"b"]))
        .unwrap();
    read_exact_response(&mut stream, b"$3\r\n2-0\r\n");
    stream
        .write_all(&command(&[b"XRANGE", b"events", b"-", b"+"]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$1\r\nf\r\n$1\r\na\r\n*2\r\n$3\r\n2-0\r\n*2\r\n$1\r\nf\r\n$1\r\nb\r\n");
    stream
        .write_all(&command(&[
            b"XRANGE", b"events", b"(1-0", b"+", b"COUNT", b"1",
        ]))
        .unwrap();
    read_exact_response(
        &mut stream,
        b"*1\r\n*2\r\n$3\r\n2-0\r\n*2\r\n$1\r\nf\r\n$1\r\nb\r\n",
    );
}

#[test]
fn xread_returns_entries_after_requested_id() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"XADD", b"events", b"1-0", b"f", b"a"]))
        .unwrap();
    read_exact_response(&mut stream, b"$3\r\n1-0\r\n");
    stream
        .write_all(&command(&[b"XREAD", b"STREAMS", b"events", b"0"]))
        .unwrap();
    read_exact_response(
        &mut stream,
        b"*1\r\n*2\r\n$6\r\nevents\r\n*1\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$1\r\nf\r\n$1\r\na\r\n",
    );
    stream
        .write_all(&command(&[b"XREAD", b"STREAMS", b"events", b"1-0"]))
        .unwrap();
    read_exact_response(&mut stream, b"*-1\r\n");
}

#[test]
fn xread_block_with_dollar_waits_for_new_entry() {
    let server = common::Server::start();
    let mut reader = server.connect().unwrap();
    let mut writer = server.connect().unwrap();
    writer
        .write_all(&command(&[b"XADD", b"events", b"1-0", b"f", b"old"]))
        .unwrap();
    read_exact_response(&mut writer, b"$3\r\n1-0\r\n");
    reader
        .write_all(&command(&[
            b"XREAD", b"BLOCK", b"500", b"STREAMS", b"events", b"$",
        ]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    writer
        .write_all(&command(&[b"XADD", b"events", b"1-1", b"f", b"new"]))
        .unwrap();
    read_exact_response(&mut writer, b"$3\r\n1-1\r\n");
    read_exact_response(
        &mut reader,
        b"*1\r\n*2\r\n$6\r\nevents\r\n*1\r\n*2\r\n$3\r\n1-1\r\n*2\r\n$1\r\nf\r\n$3\r\nnew\r\n",
    );
}

#[test]
fn xread_handles_multiple_streams_and_count() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    for (key, value) in [(b"first".as_slice(), b"a".as_slice()), (b"second", b"b")] {
        stream
            .write_all(&command(&[b"XADD", key, b"1-0", b"f", value]))
            .unwrap();
        read_exact_response(&mut stream, b"$3\r\n1-0\r\n");
    }
    stream
        .write_all(&command(&[
            b"XREAD", b"COUNT", b"1", b"STREAMS", b"first", b"second", b"0", b"0",
        ]))
        .unwrap();
    read_exact_response(&mut stream, b"*2\r\n*2\r\n$5\r\nfirst\r\n*1\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$1\r\nf\r\n$1\r\na\r\n*2\r\n$6\r\nsecond\r\n*1\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$1\r\nf\r\n$1\r\nb\r\n");
    stream.write_all(&command(&[b"XLEN", b"first"])).unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream.write_all(&command(&[b"XLEN", b"missing"])).unwrap();
    read_exact_response(&mut stream, b":0\r\n");
}

#[test]
fn incr_handles_missing_integer_overflow_and_wrong_type() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream.write_all(&command(&[b"INCR", b"counter"])).unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream.write_all(&command(&[b"GET", b"counter"])).unwrap();
    read_exact_response(&mut stream, b"$1\r\n1\r\n");
    stream
        .write_all(&command(&[b"SET", b"counter", b"9223372036854775807"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    stream.write_all(&command(&[b"INCR", b"counter"])).unwrap();
    read_exact_response(
        &mut stream,
        b"-ERR value is not an integer or out of range\r\n",
    );
    stream.write_all(&command(&[b"GET", b"counter"])).unwrap();
    read_exact_response(&mut stream, b"$19\r\n9223372036854775807\r\n");
    stream
        .write_all(&command(&[b"LPUSH", b"items", b"one"]))
        .unwrap();
    read_exact_response(&mut stream, b":1\r\n");
    stream.write_all(&command(&[b"INCR", b"items"])).unwrap();
    read_exact_response(
        &mut stream,
        b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n",
    );
}

#[test]
fn incr_preserves_existing_expiry() {
    let server = common::Server::start();
    let mut stream = server.connect().unwrap();
    stream
        .write_all(&command(&[b"SET", b"counter", b"4", b"PX", b"200"]))
        .unwrap();
    read_exact_response(&mut stream, b"+OK\r\n");
    stream.write_all(&command(&[b"INCR", b"counter"])).unwrap();
    read_exact_response(&mut stream, b":5\r\n");
    std::thread::sleep(std::time::Duration::from_millis(260));
    stream.write_all(&command(&[b"GET", b"counter"])).unwrap();
    read_exact_response(&mut stream, b"$-1\r\n");
}
