# Interoperability testing

ReRu is validated in three ways:

1. **Unit and integration tests** that run without external services.
2. **Differential tests** that compare ReRu against a live Redis server.
3. **File interoperability checks** that exchange RDB and AOF files with Redis.

The first group is what CI runs. The other two need a local Redis install and
are run by hand, or by a maintainer before a compatibility claim.

## Differential tests

Some integration tests compare exact RESP replies with a real Redis. They are
skipped unless `RERU_REDIS_REFERENCE_PORT` is set.

```sh
# Start a dedicated reference server (not on your normal port).
redis-server --port 6391 --save '' --appendonly no --daemonize yes

RERU_REDIS_REFERENCE_PORT=6391 cargo test

# Only the reference-heavy suites:
RERU_REDIS_REFERENCE_PORT=6391 cargo test \
  --test redis_differential --test rdb --test stream_groups \
  --test hyperloglog --test connection
```

On PowerShell:

```powershell
$env:RERU_REDIS_REFERENCE_PORT = '6391'
cargo test --test redis_differential --test rdb --test stream_groups --test hyperloglog --test connection
Remove-Item Env:RERU_REDIS_REFERENCE_PORT
```

Use a dedicated instance. The tests write keys and may flush databases.

## RDB interoperability

`scripts/test_rdb_interop.py` checks the full exchange path:

```
Redis -> RDB file -> ReRu load -> ReRu SAVE -> RDB file -> Redis load
```

It creates temporary servers and files, compares values, and validates the
exported file with `redis-check-rdb`.

```sh
python scripts/test_rdb_interop.py --reru target/debug/reru
```

On Windows, run it against a Linux binary through WSL:

```powershell
python scripts/test_rdb_interop.py --wsl Ubuntu --reru target/debug/reru.exe
```

## AOF interoperability

`scripts/test_aof_rewrite_interop.py` compacts ReRu's append-only file, checks it
with `redis-check-aof`, loads it into a dedicated Redis server, and compares
values, expiry deadlines, stream IDs, consumer groups, pending ownership,
deleted and trimmed payloads, explicit ID resets, HyperLogLog strings, and
writes issued after rewrite began.

```sh
python -B scripts/test_aof_rewrite_interop.py --reru target/debug/reru
```

## Reference version

The tests currently use **Redis 7.0.15** as the reference. The compatibility
target is **Redis 8.10.2**. Run the suite against the pinned version before
claiming coverage for its newer encodings and behaviors.

## Command-name coverage

`scripts/command_coverage.ps1` counts command names with an implementation path
against the pinned Redis metadata:

```powershell
powershell -File scripts/command_coverage.ps1 -RedisSource PATH_TO_REDIS_8_10_2
```

This is a name-presence count, not a behavior score. A command listed there may
still reject options or return different errors.

## Reporting results

When a change touches a command reply, error, expiry, or persistence, include
the commands you ran in the pull request. If you find a difference, add a
regression test before fixing it.
