#[path = "common/client.rs"]
mod client;
mod common;
use client::{array, integer, ok, queued, Client, Reply};

fn bulk(bytes: &[u8]) -> Reply {
    Reply::Bulk(Some(bytes.to_vec()))
}
fn values(bytes: &[&[u8]]) -> Reply {
    array(bytes.iter().map(|value| bulk(value)).collect())
}

#[test]
fn score_order_uses_binary_member_ties_and_negative_indexes() {
    let server = common::Server::start();
    let mut c = Client::new(&server);
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"2", b"b", b"1", b"a\0", b"2", b"a", b"-inf", b"first"]),
        integer(4)
    );
    assert_eq!(c.call(&[b"TYPE", b"z"]), Reply::Simple(b"zset".to_vec()));
    assert_eq!(
        c.call(&[b"ZRANGE", b"z", b"0", b"-1"]),
        values(&[b"first", b"a\0", b"a", b"b"])
    );
    assert_eq!(
        c.call(&[b"ZRANGE", b"z", b"-3", b"-1", b"WITHSCORES"]),
        values(&[b"a\0", b"1", b"a", b"2", b"b", b"2"])
    );
    assert_eq!(
        c.call(&[b"ZREVRANGE", b"z", b"0", b"1"]),
        values(&[b"b", b"a"])
    );
    assert_eq!(
        c.call(&[b"ZRANGE", b"z", b"0", b"1", b"REV"]),
        values(&[b"b", b"a"])
    );
    assert_eq!(c.call(&[b"ZRANK", b"z", b"a\0"]), integer(1));
    assert_eq!(c.call(&[b"ZREVRANK", b"z", b"b"]), integer(0));
    assert_eq!(c.call(&[b"ZSCORE", b"z", b"first"]), bulk(b"-inf"));
    assert_eq!(c.call(&[b"ZCARD", b"z"]), integer(4));
    assert_eq!(c.call(&[b"ZRANGE", b"z", b"4", b"9"]), array(vec![]));
}

#[test]
fn add_flags_count_new_or_changed_members_and_reject_conflicts() {
    let server = common::Server::start();
    let mut c = Client::new(&server);
    assert_eq!(c.call(&[b"ZADD", b"z", b"1", b"a"]), integer(1));
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"NX", b"2", b"a", b"2", b"b"]),
        integer(1)
    );
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"XX", b"3", b"a", b"3", b"missing"]),
        integer(0)
    );
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"CH", b"GT", b"2", b"a", b"4", b"b"]),
        integer(1)
    );
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"CH", b"LT", b"2", b"a", b"5", b"b"]),
        integer(1)
    );
    assert_eq!(c.call(&[b"ZSCORE", b"z", b"a"]), bulk(b"2"));
    assert_eq!(c.call(&[b"ZSCORE", b"z", b"b"]), bulk(b"4"));
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"XX", b"INCR", b"5", b"missing"]),
        Reply::Bulk(None)
    );
    assert_eq!(c.call(&[b"ZADD", b"z", b"INCR", b"2", b"a"]), bulk(b"4"));
    assert_eq!(c.call(&[b"ZINCRBY", b"z", b"-1", b"a"]), bulk(b"3"));
    for args in [
        &[b"ZADD".as_slice(), b"z", b"NX", b"XX", b"1", b"x"][..],
        &[b"ZADD".as_slice(), b"z", b"GT", b"LT", b"1", b"x"][..],
        &[b"ZADD".as_slice(), b"z", b"INCR", b"1", b"x", b"2", b"y"][..],
    ] {
        assert!(matches!(c.call(args), Reply::Error(_)));
    }
}

#[test]
fn count_remove_missing_and_wrong_type_behave_consistently() {
    let server = common::Server::start();
    let mut c = Client::new(&server);
    assert_eq!(c.call(&[b"ZCARD", b"missing"]), integer(0));
    assert_eq!(c.call(&[b"ZSCORE", b"missing", b"a"]), Reply::Bulk(None));
    assert_eq!(c.call(&[b"ZRANK", b"missing", b"a"]), Reply::Bulk(None));
    assert_eq!(c.call(&[b"ZRANGE", b"missing", b"0", b"-1"]), array(vec![]));
    assert_eq!(
        c.call(&[b"ZADD", b"z", b"1", b"a", b"2", b"b", b"3", b"c"]),
        integer(3)
    );
    assert_eq!(c.call(&[b"ZCOUNT", b"z", b"(1", b"3"]), integer(2));
    assert_eq!(c.call(&[b"ZCOUNT", b"z", b"2", b"(3"]), integer(1));
    assert_eq!(
        c.call(&[b"ZREM", b"z", b"a", b"a", b"not-there"]),
        integer(1)
    );
    assert_eq!(c.call(&[b"ZREM", b"z", b"b", b"c"]), integer(2));
    assert_eq!(c.call(&[b"TYPE", b"z"]), Reply::Simple(b"none".to_vec()));
    assert_eq!(c.call(&[b"SET", b"string", b"value"]), ok());
    for args in [
        &[b"ZADD".as_slice(), b"string", b"1", b"x"][..],
        &[b"ZRANGE".as_slice(), b"string", b"0", b"-1"][..],
        &[b"ZREM".as_slice(), b"string", b"x"][..],
    ] {
        assert!(matches!(c.call(args), Reply::Error(_)));
    }
}

#[test]
fn malformed_scores_are_atomic_and_expiry_survives_updates() {
    let server = common::Server::start();
    let mut c = Client::new(&server);
    assert!(matches!(
        c.call(&[b"ZADD", b"z", b"1", b"a", b"NaN", b"b"]),
        Reply::Error(_)
    ));
    assert_eq!(c.call(&[b"EXISTS", b"z"]), integer(0));
    assert_eq!(c.call(&[b"ZADD", b"z", b"1", b"a"]), integer(1));
    assert_eq!(c.call(&[b"EXPIRE", b"z", b"100"]), integer(1));
    assert!(matches!(
        c.call(&[b"ZADD", b"z", b"2", b"a", b"invalid", b"b"]),
        Reply::Error(_)
    ));
    assert_eq!(c.call(&[b"ZSCORE", b"z", b"a"]), bulk(b"1"));
    assert_eq!(c.call(&[b"ZINCRBY", b"z", b"2", b"a"]), bulk(b"3"));
    assert_eq!(c.call(&[b"TTL", b"z"]), integer(100));
    assert!(matches!(
        c.call(&[b"ZADD", b"z", b"+inf", b"infinite"]),
        Reply::Integer(1)
    ));
    assert!(matches!(
        c.call(&[b"ZINCRBY", b"z", b"-inf", b"infinite"]),
        Reply::Error(_)
    ));
    assert_eq!(c.call(&[b"ZSCORE", b"z", b"infinite"]), bulk(b"inf"));
}

#[test]
fn sorted_sets_work_in_transactions_and_watch_tracks_real_changes() {
    let server = common::Server::start();
    let mut watcher = Client::new(&server);
    let mut writer = Client::new(&server);
    assert_eq!(writer.call(&[b"ZADD", b"z", b"1", b"a"]), integer(1));
    assert_eq!(watcher.call(&[b"WATCH", b"z"]), ok());
    assert_eq!(writer.call(&[b"ZADD", b"z", b"NX", b"2", b"a"]), integer(0));
    assert_eq!(writer.call(&[b"ZREM", b"z", b"missing"]), integer(0));
    assert_eq!(watcher.call(&[b"MULTI"]), ok());
    assert_eq!(watcher.call(&[b"EXEC"]), array(vec![]));
    assert_eq!(watcher.call(&[b"WATCH", b"z"]), ok());
    assert_eq!(writer.call(&[b"ZADD", b"z", b"CH", b"2", b"a"]), integer(1));
    assert_eq!(watcher.call(&[b"MULTI"]), ok());
    assert_eq!(watcher.call(&[b"EXEC"]), Reply::Array(None));
    assert_eq!(writer.call(&[b"MULTI"]), ok());
    assert_eq!(writer.call(&[b"ZADD", b"z", b"3", b"b"]), queued());
    assert_eq!(writer.call(&[b"ZRANGE", b"z", b"0", b"-1"]), queued());
    assert_eq!(
        writer.call(&[b"EXEC"]),
        array(vec![integer(1), values(&[b"a", b"b"])])
    );
}
