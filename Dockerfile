# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# Cache the dependency build layer separately from the source.
RUN mkdir src && printf 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --create-home --shell /usr/sbin/nologin reru
COPY --from=builder /build/target/release/reru /usr/local/bin/reru
USER reru
WORKDIR /home/reru
EXPOSE 6379
ENTRYPOINT ["reru"]
CMD ["--bind", "0.0.0.0", "--port", "6379"]
