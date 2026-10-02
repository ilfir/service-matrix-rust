# Service Matrix Rust

Rust implementation of the C# Service Matrix API. The public routes and JSON contracts are preserved, with the explicitly documented consistency fixes in [PLAN.md](PLAN.md).

## Local development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run
```

The service listens on port `8080` by default and serves Swagger UI at `http://localhost:8080/`.

For compatibility with ASP.NET Core clients, endpoint paths, query-parameter names, and JSON
request property names are matched without regard to ASCII case. Search limits may be sent as JSON
numbers (`5`) or numeric strings (`"5"`). Boolean update values may likewise be sent as booleans or
`"true"`/`"false"` strings.

Search matrices may contain empty-string cells (`""`). Empty cells are treated as blocked grid
positions and are never used in a word path. Rows must still be non-empty and rectangular, and
each non-empty cell must contain exactly one Unicode character.

## Coverage

```sh
./scripts/coverage.sh
```

The overall production-library line-coverage gate is 90%. The algorithm module additionally requires 100% line and branch coverage. The tiny binary entrypoint (`src/main.rs`) is excluded as process/platform glue; its configuration, storage, routing, health, and API behavior are covered through library and integration tests.

## Docker

```sh
docker build --build-arg SERVICE_MATRIX_GIT_SHA="$(git rev-parse HEAD)" -t service-matrix-rust .
docker run --rm -p 8080:8080 \
  -v "$PWD/data:/app/data" \
  -v "$PWD/resources:/app/resources" \
  service-matrix-rust
```

Both mounted directories must be writable by container UID `10001` because update, merge, and cleanup operations persist changes.

## C# compatibility check

With isolated C# and Rust instances running against the same dictionary snapshot:

```sh
./scripts/compare_csharp.sh http://127.0.0.1:18081 http://127.0.0.1:18082
```

The script compares search paths, include/exclude lists, and lookup words while allowing the documented lookup-source-label correction.
