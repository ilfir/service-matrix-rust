# Rust Service Matrix Backend Implementation Plan

## 1. Objective

Build a Docker-ready Rust alternative to the existing C# backend in `/Users/admin/projects/service-matrix`. The C# implementation is the sole source of truth for routes, JSON contracts, defaults, word-search behavior, dictionary files, persistence, Swagger exposure, CORS, logging, and version metadata.

The Rust service must be deployable alongside the C# service and allow an existing client to switch backends without changing its requests. Confirmed defects in the C# implementation may be corrected only where identified below; unrelated behavior must remain compatible.

## 2. Compatibility Baseline

Before implementing Rust, capture the observable C# behavior in executable fixtures:

- Run the C# API against isolated copies of its tracked dictionary files.
- Record status codes, content types, JSON property casing, validation errors, default values, and representative responses.
- Save canonical requests and responses for every endpoint, including Latin and Cyrillic searches.
- Treat the C# source and its contract, controller, integration, end-to-end, load, and helper tests as the acceptance baseline.

Expected compatibility exceptions:

- Refresh the in-memory cache immediately after update, merge, and cleanup operations.
- Reject jagged matrices and cells that do not contain exactly one Unicode character.
- Report the actual lookup source instead of labeling every result `Dictionary`.
- Return meaningful merge counts.
- Make cleanup update the merged dictionary consumed by subsequent searches.

## 3. Target Architecture

Use a single Rust binary with these logical modules:

- **HTTP layer:** Axum routes, request extraction, validation, response mapping, CORS, Swagger/OpenAPI, and request logging.
- **Application layer:** search, update, list, merge, cleanup, lookup, version, and health use cases.
- **Search engine:** deterministic eight-direction backtracking with path tracking and cell-reuse prevention.
- **Dictionary store:** startup loading, immutable read snapshots, synchronized mutations, cache refresh, and atomic file writes.
- **Configuration:** port, environment name, build SHA, data directory, resource directory, and logging level.
- **Error layer:** typed validation, storage, configuration, and internal errors mapped to C#-compatible HTTP responses.

Recommended libraries:

- Rust 2024 edition.
- Axum and Tokio for asynchronous HTTP handling.
- Serde and `serde_json` for the C#-compatible JSON contract.
- Tower HTTP for CORS and tracing middleware.
- Tracing and `tracing-subscriber` for structured logs.
- Utoipa and `utoipa-swagger-ui` for generated OpenAPI and Swagger UI.
- `thiserror` for typed internal errors.
- `tempfile` for atomic-write and storage tests.
- `cargo-llvm-cov` for enforced line and branch coverage reports.

Shared state must hold dictionary data behind an `Arc<RwLock<...>>`. A separate mutation lock must serialize update, merge, and cleanup operations so concurrent requests cannot lose writes. Searches and lookups should work from short-lived immutable snapshots and must not hold locks during CPU-intensive processing.

## 4. Public API Contract

Preserve the C# route names and camelCase JSON serialization.

### `POST /words/Search`

Request:

```json
{
  "maxLength": 5,
  "maxWords": 10,
  "minLength": 1,
  "lettersMatrix": [["a", "b"], ["c", "d"]]
}
```

Defaults when numeric properties are omitted:

- `maxLength`: `5`
- `maxWords`: `10`
- `minLength`: `1`

Validation:

- `maxLength` and `minLength` must be between 1 and 100 inclusive.
- `maxWords` must be at least 1.
- `lettersMatrix` is required and must contain at least one row.
- Every row must be nonempty and have the same column count.
- Every cell must contain exactly one Unicode character.
- Invalid input returns `400` with the C# error envelope: `success`, `error`, and optional `details`.

Successful response remains an object keyed by found word. Each word maps path indexes to an object whose key is the original matrix character and whose value is `"row column"` using zero-based coordinates.

### `POST /words/Update`

Accept `{ "words": [...], "include": true|false }`. `words` is required but may be empty; `include` defaults to `false`. Add case-insensitively unique entries to `data/include.txt` or `data/exclude.txt`, atomically replace the target file, refresh the cache, and return the number of newly added words as a bare JSON integer.

### `GET /words/List?include=true|false`

Return the cached include list when `include=true` and exclude list when false. Default `include` to `true`. Respond with a JSON array of strings.

### `POST /words/Merge`

Combine definitions, the current merged dictionary, and included words case-insensitively; remove excluded words; atomically rewrite `resources/merged.txt`; refresh the cache; and return:

```json
{
  "addedCount": 0,
  "removedCount": 0
}
```

`addedCount` is the number of included words newly added to the base set. `removedCount` is the number of existing words removed by the exclude set.

### `GET /words/CleanMerge`

Read `resources/merged.txt`, retain words with lengths from 8 through 24 that contain neither spaces nor hyphens, sort by descending length with a stable lexical tie-breaker, atomically rewrite `resources/merged.txt`, and refresh the cache. Preserve the C# success envelope and message format containing `BEFORE: <count> words, AFTER: <count> words.`

### `GET /words/LookupWord?word=...&exactMatch=false`

Require a nonblank `word`. Search definitions, merged, included, and excluded collections case-insensitively. Use equality when `exactMatch=true` and substring matching otherwise. Return an array of `{ "word": "...", "location": "..." }` with locations `Dictionary`, `Merged`, `Included`, or `Excluded`.

### `GET /version`

Preserve the response envelope:

```json
{
  "success": true,
  "data": {
    "sha": "...",
    "frameworkDescription": "...",
    "environmentName": "Development"
  }
}
```

Inject the Git SHA during the Docker build, return `unknown` when it is invalid or unavailable, describe the Rust runtime/service in `frameworkDescription`, and default the environment name to `Development`.

### Documentation, CORS, and health

- Serve Swagger UI at `/`, matching the C# service.
- Serve OpenAPI JSON at `/swagger/v1/swagger.json`.
- Preserve the C# allow-any-origin, allow-any-method, allow-any-header CORS policy.
- Add `GET /health` for Docker health checks; return `200` only after all required dictionary files have loaded.

## 5. Search Algorithm Requirements

- Load candidates from `definitions.txt` and `merged.txt`, exclude words in `exclude.txt`, and deduplicate case-insensitively.
- Apply inclusive `minLength` and `maxLength` filtering before searching.
- Precompute a normalized character matrix, first-character index, and neighbor list for every cell.
- Search all eight adjacent directions, including diagonals.
- Never reuse a matrix cell within one word path.
- Try starting cells and neighbors in row-major order for deterministic paths.
- Preserve original matrix character casing in the response.
- Compare Unicode characters case-insensitively, including the Cyrillic data exercised by the C# tests.
- Stop collecting results after candidates have been evaluated, then order by descending word length and lexical tie-breaker before applying `maxWords`.
- Return an empty JSON object when no words match.
- Propagate request cancellation and enforce a bounded-search safeguard derived from matrix and word sizes.

## 6. Storage and Configuration

Required files:

- `resources/definitions.txt`
- `resources/merged.txt`
- `data/include.txt`
- `data/exclude.txt`

Configuration variables:

- `SERVICE_MATRIX_PORT`, default `8080`
- `SERVICE_MATRIX_DATA_DIR`, default `/app/data` in Docker and `./data` locally
- `SERVICE_MATRIX_RESOURCES_DIR`, default `/app/resources` in Docker and `./resources` locally
- `SERVICE_MATRIX_ENVIRONMENT`, default `Development`
- `SERVICE_MATRIX_GIT_SHA`, default `unknown`
- `RUST_LOG`, default `info`

Startup must fail with a clear log message if a required directory or file is unavailable. File parsing preserves one word per line and supports UTF-8. Mutations must write a temporary file in the destination directory, flush it, and rename it over the target so readers never observe partial contents. Update the in-memory snapshot only after the file replacement succeeds.

## 7. Docker Design

- Use a multi-stage Dockerfile: an official Rust builder image followed by a minimal Debian runtime image.
- Compile a locked release build and copy only the binary, CA certificates if required, seed dictionaries, and runtime metadata.
- Create and run as a non-root user.
- Set the working directory to `/app`, expose `8080`, and bind the server to `0.0.0.0:8080`.
- Declare `/app/data` and `/app/resources` as the persistent mount locations. Both must be writable because merge and cleanup update resources.
- Inject `SERVICE_MATRIX_GIT_SHA` through a build argument.
- Add a Docker health check against `/health`.
- Provide documented `docker build` and `docker run` commands, including bind mounts for persistent dictionary files.

## 8. Step-by-Step Implementation Tasks

1. Create isolated C# baseline fixtures and record the contract for all seven existing endpoints.
2. Scaffold the Rust crate, module layout, configuration loader, error types, logging initialization, and shutdown handling.
3. Define Serde/Utoipa DTOs with explicit camelCase names and C# default values.
4. Implement request validation and C#-compatible `400` and `500` error envelopes.
5. Implement file discovery, UTF-8 line loading, case-insensitive dictionary indexes, snapshots, mutation locking, and atomic writes.
6. Port the optimized C# backtracking search, including first-letter indexing, precomputed neighbors, original-character paths, and iteration bounds.
7. Implement search candidate filtering, deterministic result ordering, and `maxWords` truncation.
8. Implement the update and list use cases with immediate cache refresh.
9. Implement merge with accurate counts, atomic persistence, and cache refresh.
10. Implement cleanup against the active merged dictionary and refresh the cache.
11. Implement exact and partial lookup with accurate source labels.
12. Implement the Axum handlers and route names matching the C# controller.
13. Add allow-all CORS, structured request logging, sanitized exception handling, and graceful shutdown.
14. Add version and health endpoints.
15. Generate the OpenAPI document and mount Swagger UI at the C# locations.
16. Add unit, integration, contract, concurrency, and performance tests; wire coverage reporting into the standard test command and CI.
17. Add the multi-stage Dockerfile, `.dockerignore`, environment defaults, build SHA injection, non-root runtime, volumes, and health check.
18. Run side-by-side differential tests against the C# container and classify every mismatch as an approved fix or regression.
19. Exercise update, merge, cleanup, restart, and search through a mounted Docker volume to verify persistence.
20. Document local development, testing, Docker operation, compatibility exceptions, and rollback to the C# container.

## 9. Test Plan and Acceptance Criteria

### Unit tests

- Horizontal, vertical, and diagonal paths.
- Multiple possible starting cells and deterministic path selection.
- Repeated letters without cell reuse.
- Missing words, empty results, original casing, Latin, and Cyrillic input.
- Candidate length filtering, exclusion, deduplication, and result limits.
- Cleanup filtering and ordering.
- Atomic-write success and failure behavior.
- Accurate update and merge counts.

### HTTP contract tests

- Every C# route accepts the same method and parameter placement.
- JSON uses the same property casing and response shapes.
- Omitted numeric search fields receive C# defaults.
- Empty update lists return `0`.
- Missing lookup words and invalid search requests return `400` envelopes.
- Internal storage errors return sanitized `500` envelopes.
- Swagger, OpenAPI, CORS, version, and health endpoints behave as specified.

### Integration and concurrency tests

- Updates immediately appear in list and lookup responses.
- Merge and cleanup immediately affect searches.
- Concurrent searches succeed while a mutation is committed.
- Concurrent mutations do not lose data or produce malformed files.
- Restarting with mounted volumes preserves changes.
- Startup fails predictably when required dictionary files are missing or invalid.

### Coverage gates

- Measure coverage with `cargo-llvm-cov` across all production Rust source files; test code, generated OpenAPI output, and third-party dependencies are not part of the denominator.
- Require at least 90% line coverage for the complete service. The coverage command must fail when total line coverage falls below 90%.
- Require 100% line coverage and 100% branch coverage for the word-search algorithm module. Parse the machine-readable coverage report in CI and fail if either algorithm metric is below 100%.
- Cover every algorithm outcome, including empty words, absent first letters, all start positions, every neighbor direction, visited-cell rejection, character mismatch, successful completion, backtracking, iteration-limit termination, edge and corner cells, Unicode case handling, and deterministic path/result construction.
- Publish HTML and machine-readable coverage reports as CI artifacts so uncovered lines and branches can be inspected.
- Do not permit coverage exclusions in the algorithm module. Any exclusion elsewhere must be limited to genuinely unreachable platform glue and documented inline with justification.

### Performance targets

- A helper search over a 50×50 matrix completes in under one second on the test host.
- A complete 50×50 API search completes within the C# test limit of ten seconds.
- Fifty representative search requests complete within thirty seconds.
- Concurrent read tests complete without failures or corrupted responses.

### Final acceptance

- An existing C# client can change only its base URL and use the Rust service.
- Side-by-side contract tests pass except for the five approved consistency fixes.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` pass.
- Coverage gates pass with at least 90% line coverage service-wide and 100% line and branch coverage in the word-search algorithm module.
- The release image builds reproducibly, runs as non-root, becomes healthy, and persists mutations across restarts.
- No files in `/Users/admin/projects/service-matrix` are modified.

## 10. Out of Scope

- Changing route names or redesigning response bodies.
- Authentication, authorization, rate limiting, or a database.
- Modifying the C# service or its dictionary files.
- Kubernetes, cloud deployment, or production traffic migration beyond the Docker-ready artifact and rollback documentation.
