# ReRu

A Redis-compatible server written in Rust. **Work in progress:** the first 20
CodeCrafters stages are complete; ReRu is not a full Redis replacement.

## Run

```sh
cargo run
cargo test
```

The server listens on `127.0.0.1:6379`; use `--port PORT` to change the port.
On Unix, `./your_program.sh` builds and starts it.

## Current support

- RESP2, concurrent clients, in-memory strings, lists, and streams; `EX`/`PX` expiry
- `PING`, `ECHO`, `SET`, `GET`, `INCR`, `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP`, `TYPE`
- Initial `XADD`, `XRANGE`, `XREAD`, and `XLEN` support

Data is lost on restart. Persistence, replication, clustering, authentication,
and many commands remain. See the [roadmap](docs/ROADMAP.md).
