# ReRu 0.1.0-alpha.1

7 November 2026 — experimental preview for local evaluation.

## Highlights

- RESP2 server with strings, lists, hashes, sets, sorted sets, bitmaps, streams
  with consumer groups, and geospatial commands.
- Transactions and `WATCH`, local Pub/Sub, TTLs, ten eviction policies, and
  runtime memory configuration.
- Background expiry with bounded deletion batches, persisted and replicated
  deletions, startup index rebuilding, and expiry metrics.
- Custom snapshots, single-file AOF with online rewrite, a subset of Redis RDB,
  and authenticated ReRu-to-ReRu replication.
- Default-user `AUTH`, `ACL WHOAMI`, password-file configuration, and
  authenticated replication. Multi-user ACLs and TLS remain pending.
- `--bind ADDRESS` so the server can listen on a container network while still
  defaulting to loopback.

The command inventory has 203 of 459 Redis 8.10.2 metadata names (44.2%).
This measures names with implementation paths, not complete behavior or a
percentage of Redis rebuilt. See [compatibility](COMPATIBILITY.md).

## Open-source packaging

- **License:** AGPL-3.0-only. Modified versions and hosted forks must publish
  their source. The implementation is clean-room; no Redis source is translated.
- Contributing guide, code of conduct, security policy, changelog, architecture
  notes, development guide, and interoperability testing guide.
- CI runs formatting, Clippy, and tests on Linux, macOS, and Windows, plus the
  1.88 minimum toolchain and rustdoc. A dependency audit runs weekly.
- `Dockerfile`, dev container, `Makefile`, and `cargo` aliases. A new contributor
  can clone, run `make setup`, and run `make test`.
- 20 scoped issues with `good first issue`, `help wanted`, area, and difficulty
  labels.

## Build and try

```sh
git clone https://github.com/mansiverma897993/ReRu.git
cd ReRu
make setup
make test
make run
```

Without `make`:

```sh
cargo fetch
cargo test
cargo run -- --port 6379
```

Then:

```sh
redis-cli SET hello world EX 60
redis-cli GET hello
```

On Windows use `target\release\reru.exe`. The server binds to loopback unless
`--bind` is set. Data is in memory unless a persistence option is supplied. Run
`reru --help` and see the README for the mutually exclusive persistence modes.

## Verification

204 tests cover the implemented subset, with optional comparisons against a
live Redis 7.0.15 server. RDB file exchange is checked in both directions with
`redis-check-rdb`. Formatting, Clippy with warnings denied, and an optimized
build are release checks. Redis 8.10.2 behavior, large-scale performance, and
crash fault injection have not been comprehensively validated.

## Release limits

Full cache compatibility is unfinished: memory is estimated, command execution
is serialized, and expiry and persistence can block. Several memory and queue
limits remain unbounded. Cluster, Sentinel and failover, PSYNC, multi-user ACLs,
TLS, RESP3, scripting and functions, newer stream features, complete RDB/AOF
support, and many command options remain pending. Replication only connects
matching ReRu builds.

This preview is not a production Redis replacement.

## License

ReRu is licensed under the GNU Affero General Public License v3.0
(`AGPL-3.0-only`). See [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
