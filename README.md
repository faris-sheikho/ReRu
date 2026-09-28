# ReRu

A Redis-compatible server written in Rust, built one CodeCrafters stage at a time.

Run the tests with `cargo test`. On Unix, start the server with
`./your_program.sh`; on Windows, use `cargo run`.

The first twenty milestones are complete. They cover server setup, RESP requests,
strings, expiry, lists, blocking list retrieval, and the `TYPE` command.

The server listens on `127.0.0.1:6379` by default. Pass `--port PORT` to
choose another port. It accepts RESP array requests and currently supports
`PING`, `ECHO`, `SET` (including `PX` and `EX`), `GET`, `RPUSH`, `LPUSH`,
`LRANGE`, `LLEN`, `LPOP`, `BLPOP`, and `TYPE`. All data is in memory and is lost
when the server stops. Further stages will add the remaining Redis data types
and persistence.
