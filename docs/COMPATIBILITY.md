# ReRu compatibility status

Updated: 2 October 2026. This inventory describes the implementation and local
tests; it does not certify complete compatibility with Redis.

## Implemented commands

| Area | Commands | Current scope |
| --- | --- | --- |
| Connection | `PING`, `ECHO` | PING with an optional message; RESP2 requests/replies |
| Strings | `SET`, `GET`, `INCR` | Binary values, SET EX/PX expiry, integer increment |
| Lists | `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP` | Multiple elements, negative range indexes, blocking pop with timeout |
| Streams | `XADD`, `XRANGE`, `XREAD`, `XLEN` | Entry IDs, ranges and blocking reads; no consumer groups |
| Hashes | `HSET`, `HMSET`, `HSETNX`, `HGET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HLEN`, `HKEYS`, `HVALS`, `HINCRBY` | Binary fields/values, multiple field updates, missing-field nil replies, signed 64-bit increments |
| Sets | `SADD`, `SREM`, `SISMEMBER`, `SMISMEMBER`, `SMEMBERS`, `SCARD`, `SUNION`, `SINTER`, `SDIFF` | Binary members, deduplication, membership, cardinality and read-only set algebra |
| Keyspace | `TYPE`, `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, `PERSIST` | Multiple deletion/existence keys; expiry across current data types; NX/XX/GT/LT expiry conditions |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` | Queuing, atomic execution relative to other commands, runtime errors, optimistic locking |

Total: **49 public command names**. Supporting a command name does not mean that
every Redis option for that command is implemented.

WATCH tracks writes even if the resulting value is unchanged, missing-key
creation/deletion, and natural expiry. EXEC and DISCARD clear the connection's
watches; UNWATCH clears them outside MULTI. Inside MULTI, UNWATCH is queued.
Disconnects release registrations. Replica updates identify changed keys;
unrelated writes preserve watches, while a full resync invalidates all watches.

Hashes and sets preserve key expiry when updated and disappear after the last
field/member is removed. No-op HSETNX, HDEL, SADD and SREM preserve watches;
HSET/HMSET writes invalidate watches even when values are unchanged. Hash/set
enumeration is sorted in ReRu for stable output; clients should treat ordering
as unspecified. HMSET is supported for older clients; prefer HSET for new code.

## Persistence and replication

- Optional `--snapshot FILE` saves strings, lists, streams, hashes, sets and absolute expiry
  deadlines and restores them at startup. EXPIRE, PERSIST and DEL changes are
  included. This is the custom `RERUSNP1` format, not Redis RDB or AOF.
  New builds can load earlier snapshots. Older builds cannot load snapshots
  containing hash or set values; use matching builds for these new types.
- Optional `--replicaof HOST PORT` provides read-only ReRu replicas, initial
  synchronization, subsequent full dataset snapshots and reconnection.
- Replication uses custom `RERUREP1` frames with changed-key metadata. Use matching
  builds on both ends. Redis PSYNC, partial resynchronization, acknowledgments,
  WAIT and failover are not implemented.
- Persistence errors do not roll back the in-memory command. Crash durability,
  fsync policies and disk failure recovery need further work.

## Verification for this increment

The local suite has **61 tests**: 4 unit tests and 57 server integration tests.
Hash/set tests cover binary fields/members, duplicate inputs, nil replies,
wrong types, arity, integer overflow, expiry preservation, set algebra,
no-op and real WATCH conflicts, MULTI/EXEC, concurrent increments, replica
updates, read-only replicas, snapshot restart and malformed snapshot payloads.
Earlier keyspace, list, stream and transaction regression tests remain.
Formatting and Clippy are checked.

These are ReRu regression tests. A differential suite against a running pinned
Redis server is still required before making a compatibility claim.

## Remaining major work

Sorted sets, bitmaps, geospatial commands, stream consumer groups,
pub/sub, scripting, broader command options, Redis RDB/AOF, PSYNC, authentication,
ACLs, RESP3, cluster/failover, bounded resource usage and performance engineering
remain. Blocking clients do not yet receive Redis's FIFO waiter guarantees.
Hash field expiry, HSCAN, HINCRBYFLOAT, set scans/random selection, SMOVE,
and set algebra STORE variants are also pending.

Behavior references: [HSET](https://redis.io/docs/latest/commands/hset/),
[HINCRBY](https://redis.io/docs/latest/commands/hincrby/),
[SADD](https://redis.io/docs/latest/commands/sadd/) and
[SMEMBERS](https://redis.io/docs/latest/commands/smembers/).

Follow the [roadmap](ROADMAP.md) and [team plan](ReRu_Implementation_Plan.pdf).
The PDF records the earlier planning baseline; this inventory tracks subsequent
implementation progress.
