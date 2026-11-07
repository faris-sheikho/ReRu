## Summary

<!-- What changed and why. Link the issue. -->

Closes #

## Type of change

- [ ] Bug fix
- [ ] New command or capability
- [ ] Compatibility fix (behavior now matches Redis)
- [ ] Documentation
- [ ] Refactor or internal change
- [ ] CI or tooling

## How was it tested?

<!-- Exact commands and new tests. -->

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

<!-- If you ran differential tests: -->
<!-- RERU_REDIS_REFERENCE_PORT=6391 cargo test -->

## Compatibility notes

<!-- If behavior differs from Redis, say how. Which options are unsupported? -->

## Checklist

- [ ] The change is covered by a test, or a test is not applicable (explain why)
- [ ] `docs/COMPATIBILITY.md` is updated for any command change
- [ ] `CHANGELOG.md` has an entry under `Unreleased`
- [ ] No new dependency was added without discussion
- [ ] Formatting, clippy, and tests pass locally
- [ ] No secrets, credentials, or machine-specific paths are included
