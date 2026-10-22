# ReRu roadmap

ReRu is an incremental Rust implementation of Redis-compatible behavior. The
first 20 CodeCrafters milestones, expanded list and set operations, initial stream commands, transactions with
optimistic locking, initial keyspace commands, core hashes and sets, initial
sorted sets and bitmaps, common cache-oriented string commands, and optional
logical-memory bounds with ten eviction policies, runtime memory configuration,
partial INFO memory/stats, keyspace and collection scans, and expiry inspection are implemented. Optional
ReRu snapshots, basic append-only persistence, ReRu-to-ReRu replication, and
local Pub/Sub are tested. See the
[compatibility status](COMPATIBILITY.md) for the current command inventory.
The latest increment adds stream consumer groups, pending-message recovery,
XINFO diagnostics, atomic XADD trimming and group persistence/replication.
The preceding increment added manual BGREWRITEAOF compaction with concurrent-write
preservation, AOF status/configuration inspection, and recovery tests. It also
adds default-user AUTH, ACL WHOAMI, password-file startup configuration and
authenticated connections from replicas to their primary.
The preceding increment added deadline-indexed background expiry, deletion propagation
and persistence, configurable client limits, and client/expiry INFO metrics.
Version 0.1.0-alpha.1 is an experimental release candidate, not production readiness.
The preceding increment added an initial Redis RDB codec, SAVE/BGSAVE/LASTSAVE,
DUMP/RESTORE, and full-file interoperability checks. It supports common
collection encodings, checksums and key expiry; unsupported types fail explicitly.
The preceding increment added sorted-set multi-key and blocking pops plus initial
geospatial storage and search, plus single-database flush operations. Earlier
increments added floating-point string increments, the legacy SUBSTR alias,
server TIME, blocking list moves and
multi-key pops, stream deletion/trimming/reverse ranges, set intersection
cardinality, sorted-set lex ranges, bitfields and sorted-set algebra. A small
differential suite has passed against Redis 7.0.15, including these string and
list operations; testing against the pinned version and
across every supported option remains pending.
Redis 8.10.2 is the pinned compatibility reference. The current Rust code uses
upstream documentation for behavior; a direct source-translation license has
not been selected.

## Compatibility target

- Use a pinned Redis release for each compatibility test suite so behavior does
  not change during implementation.
- Match RESP replies, errors, binary strings, expiry, and command edge cases.
- Compare observable behavior against a real Redis server and keep regression
  tests for every command before marking it complete.
- Track command-name presence separately from verified behavior and subsystem
  coverage. The current 192/459 metadata entries (41.8%) do not establish
  30–40% implementation of Redis.
- Keep the server usable after every small, tested commit.

## Work remaining

1. Complete remaining list options and newer stream features: reference-aware
   trimming/deletion, IDMP, negative acknowledgements and CLAIM reads. Add FIFO
   waiter ordering and disconnect cancellation for blocking commands.
2. Expand sorted sets, bitmaps, and geospatial indexing; extend hashes and
   sets with field expiry and remaining command options. Existing scans, random
   access, moves and set algebra still need broader compatibility tests.
3. Expand transaction compatibility testing, Pub/Sub coverage, scripting, and
   the remaining keyspace commands.
4. Expand RDB to streams, legacy encodings and modern Redis types; add save
   scheduling and broader crash recovery. Add automatic AOF rewrite, configuration,
   multipart manifests, and interoperability.
5. Add Redis PSYNC replication, failover, clustering, multi-user ACLs, TLS, and
   configuration.
6. Bring maxmemory accounting and eviction internals closer to Redis, then add RESP3, performance and resource controls, operational tooling, and
   compatibility testing at scale.

## Current limits

- By default data is held only in memory. `--snapshot FILE` saves after each
  write using a ReRu-specific format; it is not Redis RDB. `--appendonly FILE`
  offers single-file RESP command batches with fsync per batch; it lacks Redis
  multipart AOF and automatic rewrite scheduling and cannot be combined with snapshots or replicas.
  Manual BGREWRITEAOF compacts the file, but cloning and final installation still
  block commands and need performance work.
- Explicit `--dir`/`--dbfilename` enables Redis RDB startup loading and manual
  SAVE/BGSAVE. The supported subset exchanges files with Redis 7.0.15;
  stream/legacy/new specialized encodings and automatic saves remain pending.
- Replicas use full snapshot transfer on each write and resynchronize after a
  disconnect. They do not support Redis PSYNC, offsets, or backlog recovery.
  Custom replication frames carry changed-key metadata for replica `WATCH`;
  a full resynchronization invalidates every active watch on that replica.
- Only RESP array requests and RESP2 replies are supported.
- Each client uses one operating-system thread, bounded by `--maxclients`
  (default 10000). The optional maxmemory ceiling uses estimated logical dataset size,
  not process memory. Bitmap values have a 64 MiB growth cap.
- Ten maxmemory policies are available. ReRu's memory accounting, LRU/LFU
  algorithms, INFO output, and most CONFIG parameters are not Redis exact.
- Blocking list operations wake waiting clients, but Redis's FIFO waiter
  ordering is not implemented yet.
- This project is not a production Redis replacement at this stage.

Upstream references: <https://github.com/redis/redis> and
<https://redis.io/docs/latest/commands/>. Redis 7.2 and earlier use BSD-3-Clause;
Redis 8 offers RSALv2, SSPLv1, or AGPLv3. Any direct source translation must
retain the license and attribution required by the selected release.
