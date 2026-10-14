# ReRu

A Redis-compatible server written in Rust. **Work in progress:** ReRu is not a
complete Redis replacement.

## Run

```sh
cargo run
cargo test
cargo run -- --port 6380 --snapshot ./reru.snap
cargo run -- --port 6380 --appendonly ./reru.aof
cargo run -- --port 6381 --replicaof 127.0.0.1 6380
cargo run -- --maxmemory 64mb --maxmemory-policy allkeys-lru
```

The server listens on `127.0.0.1:6379`; use `--port PORT` to change the port.
On Unix, `./your_program.sh` builds and starts it. `--snapshot` saves the dataset
after writes and restores it on startup. Its file format is specific to ReRu.
`--appendonly FILE` instead writes RESP command transactions and replays them on
startup. It fsyncs each committed batch. It cannot currently be combined with
`--snapshot` or `--replicaof`.
`--replicaof` starts a read-only ReRu replica that resynchronizes from the primary.
`--maxmemory` uses a ReRu dataset-size estimate. Its policies are `noeviction`
(default), `allkeys-lru`, `allkeys-lrm`, `allkeys-lfu`, `allkeys-random`,
`volatile-lru`, `volatile-lrm`, `volatile-lfu`, `volatile-random`, and
`volatile-ttl`; the default limit is `0` (unlimited).
`CONFIG GET/SET` can inspect and change these two settings at runtime.
`INFO memory` and `INFO stats` report the estimated dataset size and eviction
count. These memory figures are not the process's actual RAM use.

## Current support

- RESP2, concurrent clients, strings, lists, streams, hashes, sets, and sorted sets; key expiry
- `PING`, `ECHO`, `SET`, `GET`, `INCR`, `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP`, `TYPE`
- Additional list editing and movement: `RPOP`, `LINDEX`, `LSET`, `LTRIM`,
  `LINSERT`, `LPUSHX`, `RPUSHX`, `LMOVE`, `RPOPLPUSH`, `LPOS`, and `LREM`
- Cache-oriented string operations: conditional/expiring `SET`, `GETEX`, `GETDEL`,
  `MGET`, `MSET`, `MSETNX`, `APPEND`, and `STRLEN`
- Initial `XADD`, `XRANGE`, `XREAD`, and `XLEN` support
- Hash fields: `HSET`, `HGET`, multi-field operations, enumeration, deletion, and `HINCRBY`
- Set membership, removal, cardinality, random selection, moves, and
  `SUNION`/`SINTER`/`SDIFF` with destination storing variants
- Sorted-set insertion, score/rank lookup, ranges, counts, and removal
- Bitmap operations on strings: `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`, and `BITPOS`
- Optional memory limit with OOM rejection and ten maxmemory policies
- `CONFIG GET/SET` for `maxmemory` and `maxmemory-policy`
- `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, and `PERSIST`
- `COPY`, `RENAME`, `RENAMENX`, `UNLINK`, `TOUCH`, and `DBSIZE`
- `KEYS`, `SCAN`, `RANDOMKEY`, `EXPIRETIME`, and `PEXPIRETIME` for keyspace discovery
- `MULTI`, `EXEC`, `DISCARD`, `WATCH`, and `UNWATCH`, including optimistic locking
  on primary and replica connections
- Pub/Sub: `PUBLISH`, channel and pattern subscriptions, unsubscribe, and basic
  `PUBSUB` introspection; `QUIT` closes a client connection
- Optional append-only persistence with restart and partial-tail recovery;
  `EXPIREAT` and `PEXPIREAT` for absolute key expiry

110 public command names are supported, with limitations documented in the
[compatibility status](docs/COMPATIBILITY.md). ReRu replication uses the custom
`RERUREP1` framing protocol; primary and replicas must run matching builds.

Redis RDB and full Redis AOF compatibility, PSYNC replication, clustering, authentication,
production-grade memory accounting, and many commands remain. See the [roadmap](docs/ROADMAP.md) and
[team implementation plan](docs/ReRu_Implementation_Plan.pdf).
