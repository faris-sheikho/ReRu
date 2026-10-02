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
| Keyspace | `TYPE`, `DEL`, `EXISTS`, `TTL`, `PTTL`, `EXPIRE`, `PEXPIRE`, `PERSIST` | Multiple deletion/existence keys; expiry across current data types; NX/XX/GT/LT expiry conditions |
| Transactions | `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH` | Queuing, atomic execution relative to other commands, runtime errors, optimistic locking |

Total: **28 public command names**. Supporting a command name does not mean that
every Redis option for that command is implemented.

WATCH tracks writes even if the resulting value is unchanged, missing-key
creation/deletion, and natural expiry. EXEC and DISCARD clear the connection's
watches; UNWATCH clears them outside MULTI. Inside MULTI, UNWATCH is queued.
Disconnects release registrations. Replica updates identify changed keys;
unrelated writes preserve watches, while a full resync invalidates all watches.

## Persistence and replication

- Optional `--snapshot FILE` saves strings, lists, streams and absolute expiry
  deadlines and restores them at startup. EXPIRE, PERSIST and DEL changes are
  included. This is the custom `RERUSNP1` format, not Redis RDB or AOF.
- Optional `--replicaof HOST PORT` provides read-only ReRu replicas, initial
  synchronization, subsequent full dataset snapshots and reconnection.
- Replication uses custom `RERUREP1` frames with changed-key metadata. Use matching
  builds on both ends. Redis PSYNC, partial resynchronization, acknowledgments,
  WAIT and failover are not implemented.
- Persistence errors do not roll back the in-memory command. Crash durability,
  fsync policies and disk failure recovery need further work.

## Verification for this increment

The local suite has **48 tests**: 3 unit tests and 45 server integration tests.
The new tests cover duplicate/binary keys, TTL replies, conditional expiry,
malformed arguments, WATCH cleanup and conflicts, blocking pop, writes inside
EXEC, replica WATCH and snapshot restart. Formatting and Clippy are checked.

These are ReRu regression tests. A differential suite against a running pinned
Redis server is still required before making a compatibility claim.

## Remaining major work

Hashes, sets, sorted sets, bitmaps, geospatial commands, stream consumer groups,
pub/sub, scripting, broader command options, Redis RDB/AOF, PSYNC, authentication,
ACLs, RESP3, cluster/failover, bounded resource usage and performance engineering
remain. Blocking clients do not yet receive Redis's FIFO waiter guarantees.

Follow the [roadmap](ROADMAP.md) and [team plan](ReRu_Implementation_Plan.pdf).
The PDF records the earlier planning baseline; this inventory tracks subsequent
implementation progress.
