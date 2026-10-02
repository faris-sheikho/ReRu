# ReRu

A Redis-compatible server written in Rust. **Work in progress:** ReRu is not a
complete Redis replacement.

## Run

```sh
cargo run
cargo test
cargo run -- --port 6380 --snapshot ./reru.snap
cargo run -- --port 6381 --replicaof 127.0.0.1 6380
```

The server listens on `127.0.0.1:6379`; use `--port PORT` to change the port.
On Unix, `./your_program.sh` builds and starts it. `--snapshot` saves the dataset
after writes and restores it on startup. Its file format is specific to ReRu.
`--replicaof` starts a read-only ReRu replica that resynchronizes from the primary.

## Current support

- RESP2, concurrent clients, in-memory strings, lists, and streams; `EX`/`PX` expiry
- `PING`, `ECHO`, `SET`, `GET`, `INCR`, `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP`, `TYPE`
- Initial `XADD`, `XRANGE`, `XREAD`, and `XLEN` support
- `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, and `PERSIST`
- `MULTI`, `EXEC`, `DISCARD`, `WATCH`, and `UNWATCH`, including optimistic locking
  on primary and replica connections

28 public command names are supported, with limitations documented in the
[compatibility status](docs/COMPATIBILITY.md). ReRu replication uses the custom
`RERUREP1` framing protocol; primary and replicas must run matching builds.

Redis RDB/AOF files, PSYNC replication, clustering, authentication,
and many commands remain. See the [roadmap](docs/ROADMAP.md) and
[team implementation plan](docs/ReRu_Implementation_Plan.pdf).
