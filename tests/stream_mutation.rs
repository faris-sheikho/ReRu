#[path = "common/client.rs"]
#[allow(dead_code)]
mod client;
#[allow(dead_code)]
mod common;

use client::{array, integer, Client, Reply};
use std::time::{SystemTime, UNIX_EPOCH};

fn bulk(bytes: &[u8]) -> Reply {
    Reply::Bulk(Some(bytes.to_vec()))
}

#[test]
fn stream_deletion_keeps_last_generated_id_and_empty_key() {
    let server = common::Server::start();
    let mut c = Client::new(&server);
    assert_eq!(c.call(&[b"XADD", b"s", b"1-0", b"f", b"a"]), bulk(b"1-0"));
    assert_eq!(c.call(&[b"XADD", b"s", b"2-0", b"f", b"b"]), bulk(b"2-0"));
    assert_eq!(c.call(&[b"XADD", b"s", b"3-0", b"f", b"c"]), bulk(b"3-0"));
    assert_eq!(c.call(&[b"XDEL", b"s", b"3-0", b"3-0", b"9-0"]), integer(1));
    assert_eq!(c.call(&[b"XTRIM", b"s", b"MAXLEN", b"0"]), integer(2));
    assert_eq!(c.call(&[b"XLEN", b"s"]), integer(0));
    assert_eq!(c.call(&[b"TYPE", b"s"]), Reply::Simple(b"stream".to_vec()));
    assert!(matches!(
        c.call(&[b"XADD", b"s", b"2-1", b"f", b"old"]),
        Reply::Error(_)
    ));
    assert_eq!(c.call(&[b"XADD", b"s", b"3-*", b"f", b"new"]), bulk(b"3-1"));
    assert_eq!(c.call(&[b"XTRIM", b"s", b"MINID", b"4-0"]), integer(1));
    assert_eq!(c.call(&[b"XREVRANGE", b"s", b"+", b"-"]), array(vec![]));
    assert!(matches!(c.call(&[b"XDEL", b"s", b"bad"]), Reply::Error(_)));
}

#[test]
fn stream_last_id_survives_snapshot_and_aof_restarts() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for (mode, extension) in [("--snapshot", "snap"), ("--appendonly", "aof")] {
        let path = std::env::temp_dir().join(format!(
            "reru-stream-{mode}-{}-{unique}.{extension}",
            std::process::id()
        ));
        let filename = path.to_str().unwrap();
        {
            let server = common::Server::start_with(&[mode, filename]);
            let mut c = Client::new(&server);
            assert_eq!(c.call(&[b"XADD", b"s", b"1-0", b"f", b"a"]), bulk(b"1-0"));
            assert_eq!(c.call(&[b"XADD", b"s", b"2-0", b"f", b"b"]), bulk(b"2-0"));
            assert_eq!(c.call(&[b"XDEL", b"s", b"2-0"]), integer(1));
            assert_eq!(c.call(&[b"XTRIM", b"s", b"MAXLEN", b"0"]), integer(1));
        }
        {
            let server = common::Server::start_with(&[mode, filename]);
            let mut c = Client::new(&server);
            assert_eq!(c.call(&[b"TYPE", b"s"]), Reply::Simple(b"stream".to_vec()));
            assert_eq!(c.call(&[b"XLEN", b"s"]), integer(0));
            assert_eq!(c.call(&[b"XADD", b"s", b"2-*", b"f", b"c"]), bulk(b"2-1"));
        }
        std::fs::remove_file(&path).unwrap();
        if mode == "--snapshot" {
            std::fs::remove_file(path.with_extension("bak")).ok();
        }
    }
}
