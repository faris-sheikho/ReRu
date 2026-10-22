# ReRu 0.1.0-alpha.1

22 October 2026 — experimental preview for local evaluation.

## Included

- Rust RESP2 server with strings, lists, hashes, sets, sorted sets, bitmaps,
  streams with consumer groups and geospatial commands.
- Transactions and WATCH, local Pub/Sub, TTLs, ten eviction policies and runtime
  memory configuration.
- Background expiry with bounded deletion batches; persisted/replicated deletions,
  startup index rebuilding, and expiry metrics.
- Configurable connection limits, connection statistics, `--help` and `--version`.
- Custom snapshots, initial AOF, ReRu-to-ReRu replication, and a subset of Redis
  RDB including SAVE/BGSAVE and DUMP/RESTORE.
- Manual BGREWRITEAOF compaction preserving concurrent writes, with AOF status
  fields and read-only appendonly/appendfsync configuration inspection.
- Default-user AUTH, ACL WHOAMI, password-file configuration and authenticated
  ReRu replication; multi-user ACL rules and TLS remain pending.

The command inventory has 192 of 459 Redis 8.10.2 metadata names (41.8%).
This measures names with implementation paths, not complete behavior or a
percentage of Redis rebuilt. See [compatibility](COMPATIBILITY.md).

### Stream increment

- Consumer-group lifecycle, blocking multi-stream delivery, NOACK, pending
  inspection, acknowledgement, manual claiming and automatic recovery.
- Group/consumer/stream diagnostics, including FULL pending-entry details.
- Snapshot, AOF rewrite/replay and ReRu replication of group state; deleted and
  trimmed pending messages survive restart. AOF consumer idle times can reset.
- Atomic XADD NOMKSTREAM/MAXLEN/MINID, retained-entry metadata and ID reset checks.
- Concurrent-consumer, persistence, memory rollback and real Redis comparisons.

## Build and try

```sh
cargo test
cargo build --release
./target/release/reru --version
./target/release/reru --maxmemory 64mb --maxmemory-policy allkeys-lru --maxclients 256
redis-cli SET hello world EX 60
redis-cli GET hello
```

On Windows use `target\release\reru.exe`. The server binds to loopback only.
Data is in memory unless a persistence option is supplied. Run `reru --help`
and see the README for the mutually exclusive persistence modes.
`powershell -File scripts/package_windows.ps1` builds a Windows x86_64 ZIP and
SHA-256 checksum in `target/releases`. The package is a local preview artifact;
creating it does not publish a GitHub Release.

## Verification

189 local tests cover the implemented subset, with optional real-server
comparisons enabled against Redis 7.0.15. RDB file exchange is additionally
checked in both directions with `redis-check-rdb`. Formatting, Clippy with
warnings denied, and an optimized Windows build are release checks.
Redis 8.10.2 behavior, large-scale performance and crash fault injection have
not been comprehensively validated.

## Release limits

Full cache compatibility is unfinished: memory is estimated, command execution
is serialized, expiry/persistence can block, and several memory/queue limits
remain unbounded. Cluster, Sentinel/failover, PSYNC, multi-user ACLs, TLS,
RESP3, scripting/functions, newer stream features, complete RDB/AOF support
and many command options remain pending. Replication only connects matching
ReRu builds. This preview is not a production Redis replacement.

No upstream Redis C source was translated for this increment. Repository-wide
license selection and attribution review remain release tasks before presenting
this as a redistributable, licensed open-source release.
