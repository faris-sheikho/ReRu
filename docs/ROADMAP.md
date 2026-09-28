# ReRu roadmap

ReRu is an incremental Rust implementation of Redis-compatible behavior. The
first 20 CodeCrafters milestones and initial stream commands are implemented.
Redis 8.10.2 is the pinned compatibility reference. The current Rust code uses
upstream documentation for behavior; a direct source-translation license has
not been selected.

## Compatibility target

- Use a pinned Redis release for each compatibility test suite so behavior does
  not change during implementation.
- Match RESP replies, errors, binary strings, expiry, and command edge cases.
- Compare observable behavior against a real Redis server and keep regression
  tests for every command before marking it complete.
- Keep the server usable after every small, tested commit.

## Work remaining

1. Complete lists and streams, including blocking operations and stream IDs.
2. Add the remaining core data types: hashes, sets, sorted sets, bitmaps,
   geospatial indexes, and their command families.
3. Add transactions, optimistic locking, pub/sub, scripting, and keyspace
   commands.
4. Add RDB and AOF persistence with restart and recovery tests.
5. Add replication, failover, clustering, authentication, and configuration.
6. Add RESP3, performance and resource controls, operational tooling, and
   compatibility testing at scale.

## Current limits

- Data is held only in memory. A restart loses it.
- Only RESP array requests and RESP2 replies are supported.
- Each client uses one operating-system thread. There is no configured client
  or memory limit.
- Blocking list operations wake waiting clients, but Redis's FIFO waiter
  ordering is not implemented yet.
- This project is not a production Redis replacement at this stage.

Upstream references: <https://github.com/redis/redis> and
<https://redis.io/docs/latest/commands/>. Redis 7.2 and earlier use BSD-3-Clause;
Redis 8 offers RSALv2, SSPLv1, or AGPLv3. Any direct source translation must
retain the license and attribution required by the selected release.
