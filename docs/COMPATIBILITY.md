# ReRu compatibility status

Updated: 15 October 2026. This inventory describes the implementation and local
tests; it does not certify complete compatibility with Redis.

## Implemented commands

| Area | Commands | Current scope |
| --- | --- | --- |
| Connection | `PING`, `ECHO` | PING with an optional message; RESP2 requests/replies |
| Strings/cache | `SET`, `GET`, `INCR`, `GETEX`, `GETDEL`, `MGET`, `MSET`, `MSETNX`, `APPEND`, `STRLEN`, `SETNX`, `SETEX`, `PSETEX`, `GETSET`, `INCRBY`, `DECR`, `DECRBY`, `GETRANGE`, `SETRANGE` | Binary values; SET NX/XX/GET/EX/PX/EXAT/PXAT/KEEPTTL; GETEX expiry/PERSIST; multi-key access, integer arithmetic and byte ranges |
| Lists | `RPUSH`, `LPUSH`, `LPUSHX`, `RPUSHX`, `LRANGE`, `LLEN`, `LINDEX`, `LSET`, `LINSERT`, `LTRIM`, `LREM`, `LPOS`, `LPOP`, `RPOP`, `RPOPLPUSH`, `LMOVE`, `BLPOP`, `BRPOP` | Multiple elements, negative indexes, edits, moves, position search, blocking left/right pops with timeout |
| Streams | `XADD`, `XRANGE`, `XREVRANGE`, `XREAD`, `XLEN`, `XDEL`, `XTRIM` | Entry IDs, forward/reverse ranges, deletion, exact MAXLEN/MINID trimming and blocking reads; no consumer groups |
| Hashes | `HSET`, `HMSET`, `HSETNX`, `HGET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HLEN`, `HKEYS`, `HVALS`, `HINCRBY`, `HINCRBYFLOAT`, `HSTRLEN`, `HRANDFIELD`, `HSCAN` | Binary fields/values, multiple field updates, missing-field nil replies, increments, random fields and cursor iteration |
| Sets | `SADD`, `SREM`, `SISMEMBER`, `SMISMEMBER`, `SMEMBERS`, `SCARD`, `SMOVE`, `SPOP`, `SRANDMEMBER`, `SUNION`, `SINTER`, `SINTERCARD`, `SDIFF`, `SUNIONSTORE`, `SINTERSTORE`, `SDIFFSTORE`, `SSCAN` | Binary members, deduplication, membership, cardinality, random selection, atomic moves, set algebra and cursor iteration; SINTERCARD LIMIT |
| Sorted sets | `ZADD`, `ZINCRBY`, `ZCARD`, `ZSCORE`, `ZRANK`, `ZREVRANK`, `ZRANGE`, `ZREVRANGE`, `ZRANGEBYSCORE`, `ZREVRANGEBYSCORE`, `ZRANGEBYLEX`, `ZREVRANGEBYLEX`, `ZCOUNT`, `ZLEXCOUNT`, `ZREM`, `ZREMRANGEBYSCORE`, `ZREMRANGEBYRANK`, `ZREMRANGEBYLEX`, `ZMSCORE`, `ZSCAN`, `ZPOPMIN`, `ZPOPMAX` | Score-ordered members, rank, score and lex ranges, ZADD NX/XX/GT/LT/CH/INCR, cursor iteration, range removal and ordered popping |
| Bitmaps | `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`, `BITPOS` | Bitmaps stored as strings, byte/bit count ranges, AND/OR/XOR/NOT |
| Keyspace | `TYPE`, `DEL`, `UNLINK`, `EXISTS`, `TOUCH`, `DBSIZE`, `KEYS`, `SCAN`, `RANDOMKEY`, `COPY`, `RENAME`, `RENAMENX`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, `EXPIREAT`, `PEXPIREAT`, `EXPIRETIME`, `PEXPIRETIME`, `PERSIST` | Multiple keys, glob and type filtered scanning, rename/copy with TTL, relative/absolute expiry conditions |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` | Queuing, atomic execution relative to other commands, runtime errors, optimistic locking |
| Pub/Sub | `PUBLISH`, `SUBSCRIBE`, `UNSUBSCRIBE`, `PSUBSCRIBE`, `PUNSUBSCRIBE`, `PUBSUB` | Binary channels/messages, glob patterns, RESP2 subscribed mode, CHANNELS/NUMSUB/NUMPAT; local process only |
| Connection close | `QUIT` | Replies OK and closes the connection |
| Configuration | `CONFIG GET`, `CONFIG SET` | Runtime `maxmemory` and `maxmemory-policy` only; exact names and basic glob patterns for GET |
| Observability | `INFO` | Partial `memory` and `stats` sections; estimated logical bytes and eviction count |

Total: **142 Redis command metadata entries**, or **30.9% name presence**
against 459 entries in the pinned Redis 8.10.2 metadata. This is neither a
behavior pass rate nor a percentage of Redis completed. Run
`powershell -File scripts/command_coverage.ps1 -RedisSource PATH_TO_REDIS_8_10_2`
from the repository root to reproduce the count. Many options and error cases
within those entries remain unsupported.

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
`SINTERCARD` supports multiple keys and `LIMIT`. `SPOP` and `SRANDMEMBER` use a randomized member order; count replies are capped
at one million elements to bound allocation. Set store variants replace the
destination and clear its previous TTL.
`HRANDFIELD` uses the same one-million count cap. `HINCRBYFLOAT` uses Rust `f64`
arithmetic, so last-digit results can differ from Redis long-double arithmetic.
`HSCAN`, `SSCAN`, and `ZSCAN` currently use sorted offset cursors, rebuilding
their index for each call. They apply `MATCH` after page selection and accept
`COUNT` as a page size; concurrent edits may repeat or skip elements. `HSCAN`
supports `NOVALUES`. `SETRANGE` currently caps growth at 64 MiB.

## Memory limit and eviction

`--maxmemory SIZE` accepts bytes or `k`/`kb`, `m`/`mb`, and `g`/`gb` suffixes.
The default `0` means unlimited. `--maxmemory-policy` accepts `noeviction`,
`allkeys-lru`, `allkeys-lrm`, `allkeys-lfu`, `allkeys-random`, `volatile-lru`,
`volatile-lrm`, `volatile-lfu`, `volatile-random`, and `volatile-ttl`. ReRu
estimates logical dataset bytes from keys, values, and fixed per-entry overhead;
this is **not** Redis allocator memory, process RSS, or an exact RAM ceiling.
Allkeys policies may evict any live key. Volatile policies only evict live keys
with expiry; a growing write returns OOM and rolls back if those candidates
cannot free enough space. `noeviction` also rolls back growing writes on OOM,
including their WATCH effects. Evictions invalidate WATCH and are included in
ReRu snapshots and replication. Loaded snapshots are trimmed to the limit when
an allkeys policy is selected; recency and frequency are not persisted.

LRU and LRM use exact in-memory order; LFU uses a simplified frequency counter
with minute-based decay. Redis uses sampled eviction and a different LFU
algorithm. Random policies pick from a shuffled candidate order, and
`volatile-ttl` picks the earliest expiry. Memory accounting scans the dataset;
bounded writes under noeviction and volatile policies copy it for rollback.
These paths need further performance work. `INFO memory` reports the logical
estimate as `used_memory` and `reru_estimated_dataset_bytes`; `INFO stats`
reports `evicted_keys`. Other INFO fields and sections are not implemented.
`CONFIG GET/SET` supports only the two memory settings; runtime changes are not
saved to a config file.

`COPY` supports same-database copy and `REPLACE`; `DB` selection is pending.
`UNLINK` removes keys immediately but frees memory synchronously, unlike
Redis's background reclamation. `DBSIZE` counts unexpired keys without an
active expiry sweep. `TOUCH` updates the in-memory access order and frequency.
`KEYS` and `SCAN` exclude expired keys. `SCAN` uses a sorted index offset as
its cursor, applies `MATCH` and `TYPE` after selecting each page, and treats
`COUNT` as a page-size hint. It rebuilds and sorts the live key list on every
call, so large keyspaces need a more scalable cursor implementation. Changes
to the keyspace during a scan can shift the index and repeat or skip keys.

Sorted sets keep a member lookup map and a score-ordered index. Score changes
update both indexes. Rank lookup is still linear in the number of members;
range scans advance through the ordered index. `ZRANGE` supports rank indexes,
`BYSCORE`, `BYLEX`, `REV`, `WITHSCORES`, and `LIMIT` on applicable ranges.
Lex ranges use the score-ordered index and should be used with equal scores,
the ordering condition documented by Redis. Bitmap growth is capped at 64 MiB per value, below Redis's
maximum; this is an explicit ReRu resource limit. `BITFIELD` is not available.
Strings and bitmaps share the same underlying value type.

`XDEL` and exact `XTRIM MAXLEN|MINID` retain a stream's last generated ID
when entries disappear. The ID survives ReRu snapshots and AOF replay.
Approximate trimming (`~`), `LIMIT`, consumer-group references, and other
stream options remain unimplemented. New stream snapshots use a revised type
tag; new builds read older tags, but older builds cannot read new snapshots.

## Persistence and replication

- Optional `--snapshot FILE` saves strings, lists, streams, hashes, sets, sorted sets and absolute expiry
  deadlines and restores them at startup. EXPIRE, PERSIST and DEL changes are
  included. This is the custom `RERUSNP1` format, not Redis RDB or AOF.
  New builds can load earlier snapshots. Older builds cannot load snapshots
  containing newer value types; use matching builds for these types.
- Optional `--appendonly FILE` writes the resulting state of changed keys as
  RESP2 `MULTI`/`EXEC` command batches and replays complete batches at startup.
  It preserves binary data, types, and absolute expiry across restarts, and
  truncates an incomplete final batch. Each batch is synced to disk before the
  command response. This is a simple single-file AOF subset; it does not
  implement Redis 7 multipart AOF manifests, background rewrite, configurable
  fsync policy, or arbitrary external AOF import. It cannot be combined with
  `--snapshot` or `--replicaof` yet. Files can grow without bound.
- Optional `--replicaof HOST PORT` provides read-only ReRu replicas, initial
  synchronization, subsequent full dataset snapshots and reconnection.
- Replication uses custom `RERUREP1` frames with changed-key metadata. Use matching
  builds on both ends. Redis PSYNC, partial resynchronization, acknowledgments,
  WAIT and failover are not implemented.
- Persistence errors do not roll back the in-memory command. Disk failure
  recovery and configurable durability policy need further work.

## Pub/Sub

Messages are delivered to current subscribers on the same server process.
Pattern subscriptions use byte-wise, case-sensitive `*`, `?`, bracket ranges,
and backslash escapes. A direct and matching pattern subscription on one client
receive separate messages, and `PUBLISH` counts both. Subscribed RESP2 clients
can issue subscription commands, PING and QUIT. Disconnects remove their
subscriptions. Delivery is at-most-once; messages are not persisted or
replicated, and clients on other ReRu servers do not receive them. Writes to a
subscriber socket are synchronous with a 500 ms timeout; slow-client buffering
and backpressure do not yet match Redis. RESET, sharded Pub/Sub, keyspace
notifications, and Pub/Sub ACLs remain pending.

## Verification for this increment

The local suite has **131 tests**: 10 unit tests and 121 server integration tests.
It covers binary values, wrong types, score ordering and updates, bitmap ranges,
SET conditions and expiry, multi-key atomicity, WATCH/MULTI/EXEC, snapshot
restart, AOF replay and partial tails, replica updates, and malformed snapshot payloads. Earlier keyspace,
list, stream, hash, set, and transaction regression tests remain. Eviction tests
cover OOM rollback, WATCH, transactions, policy selection, INFO counters,
access order, runtime configuration, and snapshot restart. Keyspace tests cover rename/copy TTL, noeviction rollback,
WATCH, TOUCH/LRU, and blocking-list wakeup. Formatting and Clippy are checked.

The suite includes an optional deterministic RESP2 differential test covering
representative strings, lists, hashes, sets, sorted sets, streams, and transactions.
It passed against a local Redis 7.0.15 instance with
`RERU_REDIS_REFERENCE_PORT=6391 cargo test --test redis_differential`.
Without the environment variable, that test skips its comparisons. The pinned
8.10.2 release and a much broader behavior corpus still need verification.

## Remaining major work

Additional sorted-set and bitmap commands, geospatial commands, stream consumer groups,
pub/sub, scripting, broader command options, Redis RDB/AOF, PSYNC, authentication,
ACLs, RESP3, cluster/failover, Redis-exact eviction internals, bounded resource usage and performance engineering
remain. Blocking clients do not yet receive Redis's FIFO waiter guarantees.
Hash field expiry, HSCAN, HINCRBYFLOAT, set scans, and broader list and set
command options are also pending.

Behavior references: [HSET](https://redis.io/docs/latest/commands/hset/),
[HINCRBY](https://redis.io/docs/latest/commands/hincrby/),
[SADD](https://redis.io/docs/latest/commands/sadd/) and
[SMEMBERS](https://redis.io/docs/latest/commands/smembers/).

Follow the [roadmap](ROADMAP.md) and [team plan](ReRu_Implementation_Plan.pdf).
The PDF records the earlier planning baseline; this inventory tracks subsequent
implementation progress.
