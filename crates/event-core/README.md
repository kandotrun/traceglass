# event-core

Pure Rust event data and search for Traceglass. The same crate compiles for native
targets and `wasm32-unknown-unknown`; it has no browser, network, filesystem, clock,
or UI dependency. `serde_json` is its only direct dependency.

```rust
use event_core::{Dataset, Level, Query};

let data = Dataset::demo(1_000_000);
let result = data.query(&Query {
    text: "timeout",
    level: Some(Level::Error),
    min_duration_ms: 1_000,
    ..Query::default()
});

// Allocate display strings only for visible rows.
for index in result.indices.iter().take(30) {
    let row = data.display(*index).unwrap();
    println!("{} {} {}", row.timestamp_ms, row.service, row.message);
}
```

Demo records occupy 24 bytes each on the tested macOS target. Messages are shared
static templates; each event stores only a template index. The six services are
orchestrator, planner, retriever, tool-runner, inference, and gateway. Data spans
one hour; an incident between 61% and 66% of that hour raises error rates and
durations. All data is synthetic and deterministic.

`Query` combines all supplied filters. Timestamp boundaries and the minimum
duration are inclusive. Text terms separated by whitespace are ANDed and can
match the message, service, or level. Matching ignores ASCII case; non-ASCII
text matches exactly. Text search does not search trace IDs or raw imported
payload fields. The query caches matches against demo templates, services, and
levels before scanning records.

`QueryResult.indices` is in ascending timestamp order. `histogram` contains 80
bins spanning the full dataset's time bounds, after all filters; the final
timestamp falls into the final bin. A single distinct timestamp maps to the
first bin. `level_counts` uses `Level::ALL` order: Debug, Info, Warn, Error.
`duration_sum_ms`, `max_duration_ms`, and `mean_duration_ms()` refer to matching
events only.

## JSONL import

```json
{"timestamp":"2026-10-20T09:00:00.125+09:00","service":"retriever","level":"INFO","message":"Retrieved 12 documents","duration_ms":42,"trace_id":"research-001","payload":{"documents":12}}
{"timestamp_ms":1792454401125,"service":"gateway","level":"ERROR","message":"Upstream timeout","duration_ms":8000,"trace_id":"research-001"}
```

Use `Dataset::import_jsonl(&text)` for defaults or
`Dataset::import_jsonl_with_limits(&text, limits)` to select explicit bounds.
Import is all-or-nothing and errors include a one-based physical line number.
Blank lines and an initial UTF-8 BOM are accepted. Rows are sorted stably by
timestamp, so equal-time rows retain their input order.

| Field | Aliases | Validation / default |
|---|---|---|
| `timestamp_ms` | `timestamp`, `ts`, `time` | Required: nonnegative integer epoch milliseconds, or RFC 3339 with explicit timezone |
| `message` | `msg` | Required string |
| `service` | `source` | Nonempty string; defaults to `imported` |
| `level` | `severity` | DEBUG, INFO, WARN, WARNING, ERROR, case insensitive; defaults to INFO |
| `duration_ms` | `duration` | Unsigned 32-bit integer milliseconds; defaults to 0 |
| `trace_id` | `traceId` | Unsigned 32-bit integer or string; defaults to input row ordinal |

String trace IDs receive a deterministic, non-cryptographic 32-bit display ID.
This ID is a UI convenience and can collide; use the original `trace_id` from
`Dataset::details(index)` for identity-sensitive work. The original JSON object,
including unknown fields and exact string trace IDs, is retained for inspection.
Generated data has no original JSON object.

Default limits are 32 MiB per file, 1,000,000 events, 256 KiB per line, 16 KiB
per message, 256 distinct services, and 128 bytes per service name. Imported
content consumes more memory than generated data because original JSON and
messages are retained. RFC 3339 submillisecond digits are truncated; leap
seconds and pre-Unix-epoch instants are rejected.

## Validation and measurement

```sh
cargo test -p event-core
cargo run --release -p event-core --example benchmark -- 1000000
```

The benchmark prints generation time and 30 warm query samples, including median
and p95. These are native CPU timings. Browser WebAssembly execution, worker
message latency, rendering, and input-to-paint latency must be measured separately.
