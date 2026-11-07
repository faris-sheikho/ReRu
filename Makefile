# ReRu development tasks. Run `make` or `make help` for the list.
SHELL := /bin/sh
REFERENCE_PORT ?= 6391

.DEFAULT_GOAL := help
.PHONY: help setup build run test check lint fmt fmt-check release redis redis-stop test-redis clean

help:
	@echo "ReRu development commands"
	@echo ""
	@echo "  make setup       Fetch dependencies"
	@echo "  make build       Debug build"
	@echo "  make run         Run on 127.0.0.1:6379 (ARGS=... to override)"
	@echo "  make test        Run all tests"
	@echo "  make check       fmt-check + lint + test"
	@echo "  make fmt         Format the code"
	@echo "  make lint        Clippy with warnings denied"
	@echo "  make release     Optimized build"
	@echo "  make redis       Start a reference Redis on port 6391"
	@echo "  make test-redis  Run tests against the reference Redis"
	@echo "  make clean       Remove build output"

setup:
	cargo fetch
	@echo "Dependencies fetched. Next: make test"

build:
	cargo build

run:
	cargo run -- $(ARGS)

test:
	cargo test

check: fmt-check lint test

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets -- -D warnings

release:
	cargo build --release

redis:
	@command -v redis-server >/dev/null 2>&1 || { echo "redis-server not found; install Redis to run differential tests"; exit 1; }
	redis-server --port $(REFERENCE_PORT) --save "" --appendonly no --daemonize yes
	@echo "reference Redis listening on 127.0.0.1:$(REFERENCE_PORT)"

redis-stop:
	redis-cli -p $(REFERENCE_PORT) shutdown nosave || true

test-redis:
	RERU_REDIS_REFERENCE_PORT=$(REFERENCE_PORT) cargo test

clean:
	cargo clean
