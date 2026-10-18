# ReRu

A Redis-compatible server written in Rust. **Work in progress:** ReRu is not a
complete Redis replacement.

## Run

```sh
cargo run
cargo test
cargo run -- --port 6380 --snapshot ./reru.snap
cargo run -- --port 6380 --appendonly ./reru.aof
cargo run -- --port 6380 --dir . --dbfilename dump.rdb
cargo run -- --port 6381 --replicaof 127.0.0.1 6380
cargo run -- --maxmemory 64mb --maxmemory-policy allkeys-lru
```

The server listens on `127.0.0.1:6379`; use `--port PORT` to change the port.
On Unix, `./your_program.sh` builds and starts it. `--snapshot` saves the dataset
after writes and restores it on startup. Its file format is specific to ReRu.
`--appendonly FILE` instead writes RESP command transactions and replays them on
startup. It fsyncs each committed batch. It cannot currently be combined with
`--snapshot` or `--replicaof`.
`--dir DIR --dbfilename FILE` enables Redis RDB loading at startup. `SAVE` and
`BGSAVE` write snapshots explicitly; there is no automatic save schedule yet.
RDB supports strings, lists, hashes, sets, sorted sets, and key expiry. Streams
and other unsupported encodings fail with an error. RDB mode is separate from
the custom snapshot and AOF modes.
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
- `PING`, `ECHO`, `SET`, `GET`, `INCR`, `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP`, `BRPOP`, `TYPE`
- Additional list editing and movement: `RPOP`, `LINDEX`, `LSET`, `LTRIM`,
  `LINSERT`, `LPUSHX`, `RPUSHX`, `LMOVE`, `RPOPLPUSH`, `BLMOVE`,
  `BRPOPLPUSH`, `LMPOP`, `BLMPOP`, `LPOS`, and `LREM`
- Cache-oriented string operations: conditional/expiring `SET`, `GETEX`, `GETDEL`,
  `MGET`, `MSET`, `MSETNX`, `APPEND`, `STRLEN`, legacy set variants,
  integer and floating-point increments, `GETRANGE`, `SUBSTR`, and `SETRANGE`
- `TIME` returns server epoch seconds and microseconds
- Initial `XADD`, `XRANGE`, `XREVRANGE`, `XREAD`, `XLEN`, `XDEL`, and exact `XTRIM` support
- Hash fields: `HSET`, `HGET`, multi-field operations, enumeration, deletion,
  `HINCRBY`, `HINCRBYFLOAT`, `HSTRLEN`, `HRANDFIELD`, and `HSCAN`
- Set membership, removal, cardinality, intersection cardinality, random selection, moves, and
  `SUNION`/`SINTER`/`SDIFF` with destination storing variants; `SSCAN`
- Sorted-set insertion, score/rank lookup, score and lex bounded ranges, counts,
  union/intersection/difference with store variants, range removal, `ZMSCORE`,
  `ZSCAN`, `ZPOPMIN`, `ZPOPMAX`, `ZMPOP`, `BZMPOP`, `BZPOPMIN`, and `BZPOPMAX`
- Bitmap operations on strings: `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`,
  `BITPOS`, `BITFIELD`, and `BITFIELD_RO`
- Geospatial storage and queries: `GEOADD`, `GEOPOS`, `GEOHASH`, `GEODIST`,
  `GEOSEARCH`, and `GEOSEARCHSTORE`
- Optional memory limit with OOM rejection and ten maxmemory policies
- `CONFIG GET/SET` for `maxmemory` and `maxmemory-policy`
- `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, and `PERSIST`
- `FLUSHDB` and `FLUSHALL` clear ReRu's single database
- `COPY`, `RENAME`, `RENAMENX`, `UNLINK`, `TOUCH`, and `DBSIZE`
- `KEYS`, `SCAN`, `RANDOMKEY`, `EXPIRETIME`, and `PEXPIRETIME` for keyspace discovery
- `MULTI`, `EXEC`, `DISCARD`, `WATCH`, and `UNWATCH`, including optimistic locking
  on primary and replica connections
- Pub/Sub: `PUBLISH`, channel and pattern subscriptions, unsubscribe, and basic
  `PUBSUB` introspection; `QUIT` closes a client connection
- Optional append-only persistence with restart and partial-tail recovery;
  `EXPIREAT` and `PEXPIREAT` for absolute key expiry
- Initial Redis RDB interoperability, `SAVE`, `BGSAVE`, `LASTSAVE`, `DUMP`, and
  `RESTORE` with `REPLACE`/`ABSTTL`; `INFO persistence` reports save status

175 Redis command metadata entries have an implementation path, with limitations
documented in the [compatibility status](docs/COMPATIBILITY.md). This is 38.1%
of the 459 entries in the pinned Redis 8.10.2 command metadata, **not** a
Redis compatibility percentage. ReRu replication uses the custom
`RERUREP1` framing protocol; primary and replicas must run matching builds.

Full RDB/AOF compatibility, PSYNC replication, clustering, authentication,
production-grade memory accounting, and many commands remain. See the [roadmap](docs/ROADMAP.md) and
[team implementation plan](docs/ReRu_Implementation_Plan.pdf).

Optional [Rea development workflow](docs/REA_WORKFLOW.md) documents analysis setup
and repeatable Redis interoperability checks. Rea is not a runtime dependency.
