# ReRu

**A Redis-compatible cache and data structure server written in Rust.**

ReRu implements Redis commands and persistence incrementally, with binary-safe
RESP2 communication. The current version,
`0.1.0-alpha.1`, is an **experimental preview**, not a production Redis replacement.

[Compatibility](docs/COMPATIBILITY.md) · [Roadmap](docs/ROADMAP.md) · [Release notes](docs/RELEASE_NOTES.md)

## Quick start

Install a current stable Rust toolchain, then build and run:

```sh
git clone https://github.com/mansiverma897993/ReRu.git
cd ReRu
cargo build --release
cargo run --release -- --port 6379 --maxmemory 64mb --maxmemory-policy allkeys-lru --maxclients 256
```

Connect with `redis-cli` or a client configured for the supported RESP2 commands:

```sh
redis-cli SET greeting "Hello from ReRu" EX 60
redis-cli GET greeting
redis-cli TTL greeting
```

The server listens on `127.0.0.1` only. The compiled executable is
`target/release/reru` on Unix or `target\release\reru.exe` on Windows.
Run it with `--help` for startup options.

## Features

| Area | Implemented scope |
| --- | --- |
| Data structures | Strings, lists, hashes, sets, sorted sets, bitmaps, streams with consumer groups and geospatial commands |
| Approximate counting | HyperLogLog distinct counts and unions, with Redis binary-format interchange |
| Cache behavior | Conditional writes, TTLs, background expiry, ten eviction policies and configurable memory estimates |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH` and `UNWATCH` |
| Messaging | Local Pub/Sub, blocking reads, stream acknowledgements and pending-message recovery |
| Persistence | ReRu snapshots, single-file AOF with background compaction, and a subset of Redis RDB |
| Replication | Read-only ReRu replicas using a custom full-snapshot protocol |
| Client connections | HELLO 2, SELECT 0, connection IDs/names/library metadata and RESET |
| Operations | Client limits, runtime memory configuration and partial `INFO` metrics |
| Authentication | Default-user passwords, `AUTH`, `ACL WHOAMI` and authenticated ReRu replication |

**203 of 459 command metadata entries** from Redis 8.10.2 have implementation
paths: **44.2% command-name coverage**. This is not a measure of complete Redis
behavior. Supported options, differences and tests are listed in the
[compatibility inventory](docs/COMPATIBILITY.md).

## Persistence

Data stays in memory unless a persistence mode is selected. Choose one mode:

| Startup options | Behavior |
| --- | --- |
| `--snapshot ./reru.snap` | Save after writes and restore at startup; ReRu-specific format |
| `--appendonly ./reru.aof` | Append RESP transactions, fsync each batch, and replay at startup |
| `--dir . --dbfilename dump.rdb` | Load supported Redis RDB data; save explicitly with `SAVE` or `BGSAVE` |

With AOF enabled, `redis-cli BGREWRITEAOF` compacts write history in a background
worker while preserving subsequent writes. Check `INFO persistence` for progress
and status. Rewriting is manual; Redis multipart AOF and automatic rewrite
thresholds are not implemented.

`--replicaof 127.0.0.1 6379 --port 6380` starts a replica. Both servers must run
matching ReRu builds; replication is not Redis PSYNC. AOF mode cannot be combined
with replica mode.

## Authentication

Use `--requirepass-file ./password.txt` to require the default user's password.
Connect with `redis-cli --askpass`. Password files accept up to 4096 bytes and
strip one trailing line ending. Replicas use `--masterauth-file ./primary-password.txt`
to authenticate upstream; their own client password is configured separately.
The `--requirepass` and `--masterauth` flags also accept passwords directly.
Authentication does not encrypt traffic; TLS and multi-user ACL policies remain pending.

## Development

```sh
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

The suite contains **204 tests**, including authentication, restart, expiry, eviction,
transaction, replication, consumer-group, HyperLogLog, connection lifecycle and AOF rewrite coverage. Five reference-dependent tests
require `RERU_REDIS_REFERENCE_PORT` to run their Redis comparisons. Current
interoperability checks use Redis 7.0.15; the complete Redis 8.10.2 behavior suite
has not been validated. See the [development workflow](docs/REA_WORKFLOW.md)
for reference-server and RDB file exchange checks.

On Windows, `powershell -File scripts/package_windows.ps1` creates a preview ZIP
and SHA-256 checksum under `target/releases`.

## Project status

ReRu is suitable for development and compatibility experiments. Remaining work
includes multi-user ACLs, TLS, RESP3, Lua/functions, newer stream features,
PSYNC, clustering/failover, complete RDB/AOF compatibility and performance tuning.
Memory limits use a logical dataset estimate, not process RSS. Commands are
serialized and each connection uses an operating-system thread.

See the [roadmap](docs/ROADMAP.md) and [team implementation plan](docs/ReRu_Implementation_Plan.pdf)
for the remaining work. Repository license selection is pending; dependency
license notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
