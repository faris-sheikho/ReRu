# ReRu

A Redis-compatible server written in Rust, built one CodeCrafters stage at a time.

Run the tests with `cargo test`. On Unix, start the server with
`./your_program.sh`; on Windows, use `cargo run`.

The first ten milestones are complete: local setup, port binding, PING,
multiple PINGs per connection, concurrent clients, ECHO, SET/GET, expiry,
list creation, and appending to a list.

The server listens on `127.0.0.1:6379` by default. Pass `--port PORT` to
choose another port. It accepts RESP array requests and currently supports
`PING`, `ECHO`, `SET` (including `PX` and `EX`), `GET`, and single-element
`RPUSH`. All data is in memory and is lost when the server stops. Further
CodeCrafters stages will add the remaining Redis data types and persistence.
