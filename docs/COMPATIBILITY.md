# ReRu compatibility status

Updated: 14 October 2026. This inventory describes the implementation and local
tests; it does not certify complete compatibility with Redis.

## Implemented commands

| Area | Commands | Current scope |
| --- | --- | --- |
| Connection | `PING`, `ECHO` | PING with an optional message; RESP2 requests/replies |
| Strings/cache | `SET`, `GET`, `INCR`, `GETEX`, `GETDEL`, `MGET`, `MSET`, `MSETNX`, `APPEND`, `STRLEN` | Binary values; SET NX/XX/GET/EX/PX/EXAT/PXAT/KEEPTTL; GETEX expiry/PERSIST; multi-key access |
| Lists | `RPUSH`, `LPUSH`, `LRANGE`, `LLEN`, `LPOP`, `BLPOP` | Multiple elements, negative range indexes, blocking pop with timeout |
| Streams | `XADD`, `XRANGE`, `XREAD`, `XLEN` | Entry IDs, ranges and blocking reads; no consumer groups |
| Hashes | `HSET`, `HMSET`, `HSETNX`, `HGET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HLEN`, `HKEYS`, `HVALS`, `HINCRBY` | Binary fields/values, multiple field updates, missing-field nil replies, signed 64-bit increments |
| Sets | `SADD`, `SREM`, `SISMEMBER`, `SMISMEMBER`, `SMEMBERS`, `SCARD`, `SUNION`, `SINTER`, `SDIFF` | Binary members, deduplication, membership, cardinality and read-only set algebra |
| Sorted sets | `ZADD`, `ZINCRBY`, `ZCARD`, `ZSCORE`, `ZRANK`, `ZREVRANK`, `ZRANGE`, `ZREVRANGE`, `ZCOUNT`, `ZREM` | Score-ordered members, rank/index ranges, ZADD NX/XX/GT/LT/CH/INCR |
| Bitmaps | `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`, `BITPOS` | Bitmaps stored as strings, byte/bit count ranges, AND/OR/XOR/NOT |
| Keyspace | `TYPE`, `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, `PERSIST` | Multiple deletion/existence keys; expiry across current data types; NX/XX/GT/LT expiry conditions |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` | Queuing, atomic execution relative to other commands, runtime errors, optimistic locking |

Total: **71 public command names**. Supporting a command name does not mean that
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

## Memory limit and eviction

`--maxmemory SIZE` accepts bytes or `k`/`kb`, `m`/`mb`, and `g`/`gb` suffixes.
The default `0` means unlimited. `--maxmemory-policy` accepts `noeviction`
(default) or `allkeys-lru`. ReRu estimates logical dataset bytes from keys,
values, and fixed per-entry overhead; this is **not** Redis allocator memory,
process RSS, or an exact RAM ceiling. LRU is exact over access timestamps kept
in memory, whereas Redis uses a sampled approximation. An oversize write under
`noeviction` returns OOM and rolls back the command, including its WATCH effects.
Evictions are included in ReRu snapshots and replication. Loaded snapshots are
trimmed to the limit under `allkeys-lru`; recency is not stored in snapshots.
Memory accounting scans the dataset and noeviction rollback copies it, so
bounded-memory writes need further performance work. Redis's `volatile-*`,
LFU, random, and TTL policies, plus runtime `CONFIG SET`, remain unsupported.

Sorted sets keep a member lookup map and a score-ordered index. Score changes
update both indexes. Rank lookup is still linear in the number of members;
range scans advance through the ordered index. `ZRANGE` supports rank indexes,
`REV`, and `WITHSCORES`; `BYSCORE`, `BYLEX`, `LIMIT`, and newer sorted-set
commands remain. Bitmap growth is capped at 64 MiB per value, below Redis's
maximum; this is an explicit ReRu resource limit. `BITFIELD` is not available.
Strings and bitmaps share the same underlying value type.

## Persistence and replication

- Optional `--snapshot FILE` saves strings, lists, streams, hashes, sets, sorted sets and absolute expiry
  deadlines and restores them at startup. EXPIRE, PERSIST and DEL changes are
  included. This is the custom `RERUSNP1` format, not Redis RDB or AOF.
  New builds can load earlier snapshots. Older builds cannot load snapshots
  containing newer value types; use matching builds for these types.
- Optional `--replicaof HOST PORT` provides read-only ReRu replicas, initial
  synchronization, subsequent full dataset snapshots and reconnection.
- Replication uses custom `RERUREP1` frames with changed-key metadata. Use matching
  builds on both ends. Redis PSYNC, partial resynchronization, acknowledgments,
  WAIT and failover are not implemented.
- Persistence errors do not roll back the in-memory command. Crash durability,
  fsync policies and disk failure recovery need further work.

## Verification for this increment

The local suite has **85 tests**: 7 unit tests and 78 server integration tests.
It covers binary values, wrong types, score ordering and updates, bitmap ranges,
SET conditions and expiry, multi-key atomicity, WATCH/MULTI/EXEC, snapshot
restart, replica updates, and malformed snapshot payloads. Earlier keyspace,
list, stream, hash, set, and transaction regression tests remain. Eviction tests
cover OOM rollback, WATCH, transactions, access order, and snapshot restart. Formatting
and Clippy are checked.

These are ReRu regression tests. A differential suite against a running pinned
Redis server is still required before making a compatibility claim.

## Remaining major work

Additional sorted-set and bitmap commands, geospatial commands, stream consumer groups,
pub/sub, scripting, broader command options, Redis RDB/AOF, PSYNC, authentication,
ACLs, RESP3, cluster/failover, full eviction policies, bounded resource usage and performance engineering
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
