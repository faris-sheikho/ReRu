# ReRu compatibility status

Updated: 18 October 2026. This inventory describes the implementation and local
tests; it does not certify complete compatibility with Redis.

## Implemented commands

| Area | Commands | Current scope |
| --- | --- | --- |
| Connection | `PING`, `ECHO` | PING with an optional message; RESP2 requests/replies |
| Strings/cache | `SET`, `GET`, `INCR`, `GETEX`, `GETDEL`, `MGET`, `MSET`, `MSETNX`, `APPEND`, `STRLEN`, `SETNX`, `SETEX`, `PSETEX`, `GETSET`, `INCRBY`, `INCRBYFLOAT`, `DECR`, `DECRBY`, `GETRANGE`, `SUBSTR`, `SETRANGE` | Binary values; SET NX/XX/GET/EX/PX/EXAT/PXAT/KEEPTTL; GETEX expiry/PERSIST; multi-key access, numeric arithmetic and byte ranges |
| Lists | `RPUSH`, `LPUSH`, `LPUSHX`, `RPUSHX`, `LRANGE`, `LLEN`, `LINDEX`, `LSET`, `LINSERT`, `LTRIM`, `LREM`, `LPOS`, `LPOP`, `RPOP`, `RPOPLPUSH`, `LMOVE`, `BLPOP`, `BRPOP`, `BLMOVE`, `BRPOPLPUSH`, `LMPOP`, `BLMPOP` | Multiple elements, negative indexes, edits, moves, position search, single/multi-key blocking pops and moves with timeout |
| Streams | `XADD`, `XRANGE`, `XREVRANGE`, `XREAD`, `XLEN`, `XDEL`, `XTRIM` | Entry IDs, forward/reverse ranges, deletion, exact MAXLEN/MINID trimming and blocking reads; no consumer groups |
| Hashes | `HSET`, `HMSET`, `HSETNX`, `HGET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HLEN`, `HKEYS`, `HVALS`, `HINCRBY`, `HINCRBYFLOAT`, `HSTRLEN`, `HRANDFIELD`, `HSCAN` | Binary fields/values, multiple field updates, missing-field nil replies, increments, random fields and cursor iteration |
| Sets | `SADD`, `SREM`, `SISMEMBER`, `SMISMEMBER`, `SMEMBERS`, `SCARD`, `SMOVE`, `SPOP`, `SRANDMEMBER`, `SUNION`, `SINTER`, `SINTERCARD`, `SDIFF`, `SUNIONSTORE`, `SINTERSTORE`, `SDIFFSTORE`, `SSCAN` | Binary members, deduplication, membership, cardinality, random selection, atomic moves, set algebra and cursor iteration; SINTERCARD LIMIT |
| Sorted sets | `ZADD`, `ZINCRBY`, `ZCARD`, `ZSCORE`, `ZRANK`, `ZREVRANK`, `ZRANGE`, `ZREVRANGE`, `ZRANGEBYSCORE`, `ZREVRANGEBYSCORE`, `ZRANGEBYLEX`, `ZREVRANGEBYLEX`, `ZCOUNT`, `ZLEXCOUNT`, `ZREM`, `ZREMRANGEBYSCORE`, `ZREMRANGEBYRANK`, `ZREMRANGEBYLEX`, `ZMSCORE`, `ZSCAN`, `ZPOPMIN`, `ZPOPMAX`, `ZMPOP`, `BZMPOP`, `BZPOPMIN`, `BZPOPMAX`, `ZUNION`, `ZUNIONSTORE`, `ZINTER`, `ZINTERSTORE`, `ZDIFF`, `ZDIFFSTORE`, `ZINTERCARD` | Score-ordered members, rank, score and lex ranges, ZADD NX/XX/GT/LT/CH/INCR, cursor iteration, range removal, ordered and multi-key blocking pops, weighted algebra and destination storing |
| Bitmaps | `SETBIT`, `GETBIT`, `BITCOUNT`, `BITOP`, `BITPOS`, `BITFIELD`, `BITFIELD_RO` | Bitmaps stored as strings, byte/bit count ranges, AND/OR/XOR/NOT; bitfield GET/SET/INCRBY, signed/unsigned widths, `#` offsets and WRAP/SAT/FAIL overflow modes |
| Geospatial | `GEOADD`, `GEOPOS`, `GEOHASH`, `GEODIST`, `GEOSEARCH`, `GEOSEARCHSTORE` | Geo points use sorted-set scores; NX/XX/CH, positions and distances, radius/box search, sort/count, decorated results, and destination storing |
| Keyspace | `TYPE`, `DEL`, `UNLINK`, `EXISTS`, `TOUCH`, `DBSIZE`, `KEYS`, `SCAN`, `RANDOMKEY`, `COPY`, `RENAME`, `RENAMENX`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, `EXPIREAT`, `PEXPIREAT`, `EXPIRETIME`, `PEXPIRETIME`, `PERSIST`, `FLUSHDB`, `FLUSHALL` | Multiple keys, glob and type filtered scanning, rename/copy with TTL, relative/absolute expiry conditions, whole-database flush |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` | Queuing, atomic execution relative to other commands, runtime errors, optimistic locking |
| Pub/Sub | `PUBLISH`, `SUBSCRIBE`, `UNSUBSCRIBE`, `PSUBSCRIBE`, `PUNSUBSCRIBE`, `PUBSUB` | Binary channels/messages, glob patterns, RESP2 subscribed mode, CHANNELS/NUMSUB/NUMPAT; local process only |
| Connection close | `QUIT` | Replies OK and closes the connection |
| Configuration | `CONFIG GET`, `CONFIG SET` | Runtime `maxmemory` and `maxmemory-policy`; GET also reports configured RDB dir/dbfilename |
| Observability | `INFO` | Partial `memory`, `stats`, and `persistence` sections |
| Server time | `TIME` | Epoch seconds and microseconds as RESP2 bulk strings |
| Persistence | `SAVE`, `BGSAVE`, `LASTSAVE`, `DUMP`, `RESTORE` | Initial Redis RDB interoperability; supported values, key expiry, checksums, REPLACE/ABSTTL |

Total: **175 Redis command metadata entries**, or **38.1% name presence**
against 459 entries in the pinned Redis 8.10.2 metadata. This is neither a
behavior pass rate nor a percentage of Redis completed. Run
`powershell -File scripts/command_coverage.ps1 -RedisSource PATH_TO_REDIS_8_10_2`
from the repository root to reproduce the count. Many options and error cases
within those entries remain unsupported.

`LMPOP` and `BLMPOP` choose the first nonempty key in argument order and return
the popped members in left or right pop order. `BLMOVE` and `BRPOPLPUSH` move
one element atomically and block on a missing source. Timeout `0` waits
indefinitely outside transactions; inside MULTI/EXEC these commands return
immediately. Waiting clients do not yet have Redis's FIFO ordering guarantee.
`ZMPOP`, `BZMPOP`, `BZPOPMIN`, and `BZPOPMAX` similarly choose the first
nonempty sorted-set key, pop in score order, and delete an emptied key.
Blocking forms wait for another write and return immediately inside MULTI/EXEC.

Geo search scans the sorted set, then filters and sorts candidates. This is
slower than Redis's geohash-area query for large datasets. `COUNT ANY` currently
uses the same post-filter limit as `COUNT`; Redis may stop early. The
`GEORADIUS` aliases and cross-dateline box behavior remain to be verified.

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
`INCRBYFLOAT` has the same precision limitation. Both reject nonfinite input
and overflow without changing the stored value.
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
`FLUSHDB` and `FLUSHALL` both clear ReRu's only database. `ASYNC` is accepted
but deletion is currently synchronous; flush invalidates WATCH and is persisted
through ReRu snapshots/AOF.
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
maximum; this is an explicit ReRu resource limit.
`BITFIELD` and `BITFIELD_RO` support integer widths up to `i64` and `u63`.
Writes that would grow beyond ReRu's 64 MiB string cap fail. Read-only
bitfield calls do not create a key; failed overflow operations leave it unchanged.
Strings and bitmaps share the same underlying value type.

Sorted-set union/intersection/difference accept sorted-set and plain-set inputs.
`ZUNION` and `ZINTER` support WEIGHTS, SUM/MIN/MAX/COUNT aggregation and
WITHSCORES; store variants replace the destination and clear its TTL.
`ZINTERCARD` supports LIMIT. COUNT aggregation was added to Redis in 8.8;
the local Redis 7.0.15 comparison covers the other algebra options.

`XDEL` and exact `XTRIM MAXLEN|MINID` retain a stream's last generated ID
when entries disappear. The ID survives ReRu snapshots and AOF replay.
Approximate trimming (`~`), `LIMIT`, consumer-group references, and other
stream options remain unimplemented. New stream snapshots use a revised type
tag; new builds read older tags, but older builds cannot read new snapshots.

## Persistence and replication

- `--dir DIR --dbfilename FILE` loads Redis RDB at startup. `SAVE` writes
  synchronously; `BGSAVE` clones the dataset under the command lock and writes
  from a worker thread. A unique temporary file is synced and renamed over the
  destination only after complete encoding. `LASTSAVE` and `INFO persistence`
  report successful save time, in-progress state and last save status.
- The RDB codec writes version 10 and accepts headers 6 through 15 only for
  supported encodings: plain strings/lists/sets/hashes/binary-score sorted sets,
  integer and LZF strings, intsets, hash/zset/set listpacks, and quicklist2
  lists. CRC64 is verified (zero file checksums are allowed by the format).
  Geo data and bitmaps round-trip through their underlying zsets and strings.
  Expiry is stored as an absolute timestamp; expired keys are omitted on load.
- RDB streams, zipmaps/ziplists/old quicklists, textual-score zsets, field TTL,
  modules, functions, newer specialized types, and nonzero databases are not
  supported. Loading these fails explicitly. Saving a dataset containing a
  stream fails without replacing the existing file. Header acceptance does not
  establish compatibility with every encoding of that Redis version.
- `DUMP`/`RESTORE` transfer the same supported value types. RESTORE supports
  millisecond TTL, `REPLACE` and `ABSTTL`; `IDLETIME`/`FREQ` remain unsupported.
  Restored writes participate in WATCH, transactions, maxmemory, AOF and
  ReRu replication. Invalid payloads leave existing destination data intact.
- RDB files and decoded string budgets are capped at 256 MiB, and key and
  collection counts at one million. There is no save schedule, shutdown save,
  automatic backup recovery, directory fsync, or AOF/RDB combination. SAVE
  and BGSAVE require explicit RDB configuration and are rejected inside MULTI.
  BGSAVE uses an in-memory copy, so large datasets can temporarily double RAM.
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

The local suite has **156 tests**: 13 unit tests and 143 server integration tests.
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

RDB integration tests cover save/restart, background saving, binary data,
TTL, invalid checksums, failed replacement, WATCH, transactions, OOM rollback,
and AOF restart. With `RERU_REDIS_REFERENCE_PORT` set, the RDB suite exchanges
DUMP payloads with Redis 7.0.15, including compressed and integer strings,
quicklist2, intset, hash listpack and sorted-set listpack encodings. Without
that variable, this second reference-dependent test also skips comparisons.
`scripts/test_rdb_interop.py` additionally creates dedicated temporary servers,
exchanges complete RDB files in both directions, and runs `redis-check-rdb`.
Run instructions are in [the development workflow](REA_WORKFLOW.md).

## Remaining major work

Additional sorted-set and bitmap commands, geo indexing, stream consumer groups,
sharded Pub/Sub, scripting, broader command options, full Redis RDB/AOF, PSYNC, authentication,
ACLs, RESP3, cluster/failover, Redis-exact eviction internals, bounded resource usage and performance engineering
remain. Blocking clients do not yet receive Redis's FIFO waiter guarantees.
Hash field expiry and broader hash, list and set
command options are also pending.

Behavior references: [HSET](https://redis.io/docs/latest/commands/hset/),
[HINCRBY](https://redis.io/docs/latest/commands/hincrby/),
[SADD](https://redis.io/docs/latest/commands/sadd/) and
[SMEMBERS](https://redis.io/docs/latest/commands/smembers/).

Follow the [roadmap](ROADMAP.md) and [team plan](ReRu_Implementation_Plan.pdf).
The PDF records the earlier planning baseline; this inventory tracks subsequent
implementation progress.
