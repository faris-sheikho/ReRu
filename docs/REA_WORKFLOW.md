# Rea and Redis interoperability workflow

Rea 6.7.0 is optional development tooling. It is not linked into ReRu and does
not automatically translate Redis into Rust. Redis source is available, so the
Rea skill recommends normal source inspection for implementation questions.
Use Rea when a claim needs evidence from a shipped binary or runtime artifact.

## Local setup verified on 18 October 2026

- Installed the bundled reverse-engineering skill and registered the Rea MCP
  server for the coding client, preserving its existing configuration and backup.
- The system Node 22.18.0 is below Rea's supported minimum. The MCP registration
  uses an already available Node 24.19.0 runtime through its scoped PATH.
  No system Node replacement is required.
- `doctor --client codex` reported healthy. The exact configured stdio launcher
  completed MCP initialization as Rea 6.7.0 and listed 207 tools. Restart the
  desktop client to load that registration into its tool list.
- A native decompiler provider is not configured. Binary decompilation is not
  available merely because MCP setup succeeded. Redis source analysis and the
  provider-free Rust artifact inspection work without one.

On another machine, inspect and apply the setup plan:

```powershell
./scripts/rea.ps1 setup --client codex --dry-run
./scripts/rea.ps1 setup --client codex --yes
./scripts/rea.ps1 doctor --client codex
```

The wrapper pins Rea 6.7.0 and selects Node 24 for CLI use. Setup's generated
MCP launcher uses the client's environment; ensure that environment also has a
supported Node version. Do not commit personal client configuration or local
paths into the repository.

## Evidence collected for this increment

`inspect-rust-binary` inspected a development build at `target/debug/reru.exe`.
The report was retained locally in `target/rea-reru-artifact.json`:

- Evidence ID: `ev_cfab23bb0492a2f23c6304de1f935acf22be0f5ffc3f821929680ce2f3202d8a`.
- SHA-256: `05c6c22cf7fd8798e234cc9796d0654f6fe65567db49edf49296f513add268d8`.
- Observed: PE, AMD64, little endian, 2,131,968 bytes.
- Unknown: no cargo-auditable `.dep-v0` dependency section was present.
  This does not establish dependency absence or Redis compatibility.

That digest identifies an intermediate debug build, not the final release.
Re-run the inspection after building an artifact to record its own identity:

```powershell
./scripts/rea.ps1 inspect-rust-binary target/release/reru.exe --format json
```

The RDB implementation's behavioral evidence comes from the tests below.
The Rea metadata inspection does not validate RDB behavior or recover source.

## Repeatable behavior checks

```powershell
cargo test --offline
$env:RERU_REDIS_REFERENCE_PORT = '6391'
cargo test --offline --test redis_differential --test rdb --test stream_groups
Remove-Item Env:RERU_REDIS_REFERENCE_PORT
python scripts/test_rdb_interop.py --wsl Ubuntu --reru target/debug/reru.exe
python -B scripts/test_aof_rewrite_interop.py --wsl Ubuntu --reru target/debug/reru.exe
```

The reference-port tests require a dedicated Redis instance on the chosen port.
The Python script creates its own temporary servers and files, tests Redis →
RDB → ReRu → RDB → Redis, and checks the exported file with `redis-check-rdb`.
On Linux, omit `--wsl Ubuntu` and use `target/debug/reru`.
The AOF script compacts ReRu's file, validates it using `redis-check-aof`, loads
it in a dedicated Redis server and compares values, expiry deadlines, stream
IDs, consumer groups, pending ownership, deleted/trimmed message payloads,
explicit ID resets, and writes issued after rewrite began.
The verified reference in this increment is Redis 7.0.15. Repeat against the
pinned Redis 8.10.2 before claiming coverage for its newer encodings.

Keep a feature ledger with supported behavior, test evidence and known gaps.
Command-name coverage is measured separately by `scripts/command_coverage.ps1`.
An installed analysis tool or a larger command count is not a compatibility score.

References: [Rea](https://github.com/morluto/rea),
[Redis RDB format constants](https://github.com/redis/redis/blob/7.0/src/rdb.h),
[SAVE](https://redis.io/docs/latest/commands/save/),
[DUMP](https://redis.io/docs/latest/commands/dump/),
[RESTORE](https://redis.io/docs/latest/commands/restore/).
