# ReRu

**A Redis-compatible cache and data structure server written in Rust.**

[![CI](https://github.com/mansiverma897993/ReRu/actions/workflows/ci.yml/badge.svg)](https://github.com/mansiverma897993/ReRu/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

ReRu speaks the RESP2 protocol, so existing Redis clients connect without
changes. It implements a growing subset of Redis commands and persistence
formats in safe, dependency-light Rust — one small crate, one runtime
dependency, no async executor.

> **Status: experimental preview (`0.1.0-alpha.1`).** ReRu is built for
> development, compatibility experiments, and learning. It is **not** a
> production Redis replacement. See [Project status](#project-status).

[Compatibility](docs/COMPATIBILITY.md) ·
[Roadmap](docs/ROADMAP.md) ·
[Architecture](docs/ARCHITECTURE.md) ·
[Release notes](docs/RELEASE_NOTES.md) ·
[Changelog](CHANGELOG.md)

## Contents

- [Why ReRu](#why-reru)
- [Quick start](#quick-start)
- [Connect a client](#connect-a-client)
- [Features](#features)
- [Server options](#server-options)
- [Persistence](#persistence)
- [Authentication](#authentication)
- [Docker](#docker)
- [Development](#development)
- [Contributing](#contributing)
- [Project status](#project-status)
- [License](#license)

## Why ReRu

- **Familiar protocol.** RESP2 over TCP. Use `redis-cli`, `redis-py`, `ioredis`,
  Lettuce, or any Redis client that supports the commands listed in the
  [compatibility inventory](docs/COMPATIBILITY.md).
- **Small and readable.** About 21k lines of straight-line Rust with one
  dependency. Every subsystem is a plain module with its own tests.
- **Batteries included.** TTLs, eviction, transactions, pub/sub, streams with
  consumer groups, persistence, replication, and geospatial queries.
- **Research-friendly.** Each command is implemented against the pinned Redis
  8.10.2 behavior reference, with differential tests against a live Redis where
  possible.

## Quick start

Install a Rust toolchain (1.88 or newer), then build and run:

```sh
git clone https://github.com/mansiverma897993/ReRu.git
cd ReRu
cargo build --release

./target/release/reru \
  --port 6379 \
  --maxmemory 64mb \
  --maxmemory-policy allkeys-lru \
  --maxclients 256
```

The server listens on `127.0.0.1` by default and prints its startup line to
stderr. The binary is `target/release/reru` on Unix and `target\release\reru.exe`
on Windows. Run `reru --help` for all flags.

## Connect a client

Use any Redis client pointed at `127.0.0.1:6379`:

```sh
redis-cli SET greeting "Hello from ReRu" EX 60
redis-cli GET greeting
redis-cli TTL greeting
redis-cli RPUSH queue a b c
redis-cli LRANGE queue 0 -1
```

In Python:

```python
import redis

client = redis.Redis(host="127.0.0.1", port=6379)
client.set("greeting", "Hello from ReRu", ex=60)
print(client.get("greeting"))
```

## Features

| Area | Implemented scope |
| --- | --- |
| Data structures | Strings, lists, hashes, sets, sorted sets, bitmaps, streams with consumer groups, geospatial commands |
| Approximate counting | HyperLogLog distinct counts and unions, with Redis dense/sparse binary interchange |
| Cache behavior | Conditional writes, TTLs, background expiry, ten eviction policies, configurable memory estimates |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` |
| Messaging | Local pub/sub, blocking reads, stream acknowledgements, pending-message recovery |
| Persistence | ReRu snapshots, single-file AOF with background compaction, a subset of Redis RDB |
| Replication | Read-only ReRu replicas using a snapshot-plus-stream protocol |
| Client connections | `HELLO 2`, `SELECT 0`, connection IDs/names/library metadata, `RESET` |
| Operations | Client limits, runtime memory configuration, partial `INFO` metrics |
| Authentication | Default-user passwords, `AUTH`, `ACL WHOAMI`, authenticated replication |

**203 of 459 command metadata entries** from Redis 8.10.2 have implementation
paths: **44.2% command-name coverage**. This counts command names, not complete
behavior. Supported options, differences, and tests are listed in the
[compatibility inventory](docs/COMPATIBILITY.md).

## Server options

| Option | Default | Description |
| --- | --- | --- |
| `--port PORT` | `6379` | TCP port to listen on |
| `--bind ADDRESS` | `127.0.0.1` | IP address to bind. Use `0.0.0.0` only inside a trusted network (for example a container) |
| `--maxclients COUNT` | `10000` | Maximum concurrent client connections |
| `--maxmemory SIZE` | `0` (unlimited) | Logical dataset memory limit, e.g. `64mb`, `1gb` |
| `--maxmemory-policy POLICY` | `noeviction` | Eviction policy |
| `--snapshot FILE` | off | Save and restore a ReRu snapshot |
| `--appendonly FILE` | off | Single-file AOF with replay and manual rewrite |
| `--dir DIR` | — | Directory for RDB files |
| `--dbfilename FILE` | `dump.rdb` | RDB filename |
| `--replicaof HOST PORT` | off | Run as a read-only replica |
| `--requirepass PASSWORD` | off | Require a default-user password |
| `--requirepass-file FILE` | off | Read the client password from a file |
| `--masterauth PASSWORD` | off | Password for the upstream primary |
| `--masterauth-file FILE` | off | Read the upstream password from a file |
| `--version`, `--help` | — | Print version or usage |

Eviction policies: `noeviction`, `allkeys-lru`, `allkeys-lrm`, `allkeys-lfu`,
`allkeys-random`, `volatile-lru`, `volatile-lrm`, `volatile-lfu`,
`volatile-random`, `volatile-ttl`.

## Persistence

Data stays in memory unless a persistence mode is selected. Choose one mode:

| Startup options | Behavior |
| --- | --- |
| `--snapshot ./reru.snap` | Save after writes and restore at startup; ReRu-specific format |
| `--appendonly ./reru.aof` | Append RESP transactions, fsync each batch, replay at startup |
| `--dir . --dbfilename dump.rdb` | Load supported Redis RDB data; save with `SAVE` or `BGSAVE` |

With AOF enabled, `redis-cli BGREWRITEAOF` compacts write history in a
background worker while preserving later writes. Check `INFO persistence` for
progress. Rewriting is manual; Redis multipart AOF and automatic rewrite
thresholds are not implemented.

`--replicaof 127.0.0.1 6379 --port 6380` starts a replica. Both servers must run
matching ReRu builds; replication is not Redis `PSYNC`. AOF mode cannot be
combined with replica mode.

## Authentication

Use `--requirepass-file ./password.txt` to require the default user's password,
then connect with `redis-cli --askpass`. Password files accept up to 4096 bytes
and strip one trailing line ending. Replicas use
`--masterauth-file ./primary-password.txt` to authenticate upstream; their own
client password is configured separately. `--requirepass` and `--masterauth`
also accept passwords directly.

Authentication does not encrypt traffic. TLS and multi-user ACL policies are
still pending.

## Docker

```sh
docker build -t reru .
docker run --rm -p 6379:6379 reru
```

The image binds to `0.0.0.0` inside the container so the port mapping works.
Keep that exposure inside a trusted network and set a password for anything
beyond local development.

## Development

```sh
cargo build                    # debug build
cargo test                     # 204 tests
cargo fmt --all -- --check     # formatting
cargo clippy --all-targets -- -D warnings   # lints
```

Reference-dependent tests compare behavior against a live Redis server and run
only when `RERU_REDIS_REFERENCE_PORT` is set. Set it to the port of a running
Redis instance to enable them. Interoperability checks currently use Redis
7.0.15; the full Redis 8.10.2 behavior suite has not been validated. See
[interoperability testing](docs/INTEROP_TESTING.md).

On Windows, `powershell -File scripts/package_windows.ps1` creates a preview ZIP
and SHA-256 checksum under `target/releases`.

### Project layout

```
src/            server modules (one per subsystem)
tests/          integration tests, one file per area
docs/           compatibility, roadmap, architecture, testing notes
scripts/        packaging and Redis interoperability helpers
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the pieces fit
together.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and the
[roadmap](docs/ROADMAP.md). Good first issues are labeled
[`good first issue`](https://github.com/mansiverma897993/ReRu/labels/good%20first%20issue).

- Read the [Code of Conduct](CODE_OF_CONDUCT.md).
- Report vulnerabilities privately per [SECURITY.md](SECURITY.md).
- Claiming a command? Add it to the [compatibility inventory](docs/COMPATIBILITY.md)
  and cover it with an integration test in `tests/`.

## Project status

ReRu is suitable for development and compatibility experiments. Remaining work
includes multi-user ACLs, TLS, RESP3, Lua and functions, newer stream features,
`PSYNC`, clustering and failover, complete RDB/AOF compatibility, and
performance tuning. Memory limits use a logical dataset estimate, not process
RSS. Commands are serialized, and each connection uses an operating-system
thread.

See the [roadmap](docs/ROADMAP.md) and
[release notes](docs/RELEASE_NOTES.md) for details.

## License

ReRu is released under the [MIT License](LICENSE). Dependency license notices
are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Acknowledgements

ReRu follows the [Redis command reference](https://redis.io/docs/latest/commands/)
and uses Redis 8.10.2 as its pinned behavior reference. Redis is a trademark of
Redis Ltd. ReRu is an independent project and is not affiliated with or endorsed
by Redis Ltd.
