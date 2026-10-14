# ReRu

A Redis-compatible server written in Rust. **Work in progress:** ReRu is not a
complete Redis replacement.

## Run

```sh
cargo run
cargo test
cargo run -- --port 6380 --snapshot ./reru.snap
cargo run -- --port 6381 --replicaof 127.0.0.1 6380
cargo run -- --maxmemory 64mb --maxmemory-policy allkeys-lru
```

The server listens on `127.0.0.1:6379`; use `--port PORT` to change the port.
On Unix, `./your_program.sh` builds and starts it. `--snapshot` saves the dataset
after writes and restores it on startup. Its file format is specific to ReRu.
`--replicaof` starts a read-only ReRu replica that resynchronizes from the primary.
`--maxmemory` uses a ReRu dataset-size estimate. Its policies are `noeviction`
(default) and `allkeys-lru`; the default limit is `0` (unlimited).

## Current support

- RESP2, concurrent clients, strings, lists, streams, hashes, sets, and sorted sets; key expiry
- `PING`, `ECHO`, `SET`, `GET`, `INCR`, `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP`, `TYPE`
- Cache-oriented string operations: conditional/expiring `SET`, `GETEX`, `GETDEL`,
  `MGET`, `MSET`, `MSETNX`, `APPEND`, and `STRLEN`
- Initial `XADD`, `XRANGE`, `XREAD`, and `XLEN` support
- Hash fields: `HSET`, `HGET`, multi-field operations, enumeration, deletion, and `HINCRBY`
- Set membership, removal, cardinality, `SUNION`, `SINTER`, and `SDIFF`
- Sorted-set insertion, score/rank lookup, ranges, counts, and removal
- Bitmap operations on strings: `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`, and `BITPOS`
- Optional memory limit with OOM rejection or least-recently-used key eviction
- `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, and `PERSIST`
- `MULTI`, `EXEC`, `DISCARD`, `WATCH`, and `UNWATCH`, including optimistic locking
  on primary and replica connections

71 public command names are supported, with limitations documented in the
[compatibility status](docs/COMPATIBILITY.md). ReRu replication uses the custom
`RERUREP1` framing protocol; primary and replicas must run matching builds.

Redis RDB/AOF files, PSYNC replication, clustering, authentication,
other eviction policies, and many commands remain. See the [roadmap](docs/ROADMAP.md) and
[team implementation plan](docs/ReRu_Implementation_Plan.pdf).
