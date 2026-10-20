//! Native CPU benchmark; browser/Wasm timings must be measured separately.
//! Run: cargo run --release -p event-core --example benchmark -- 1000000

use event_core::{Dataset, Level, Query};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let count = std::env::args()
        .nth(1)
        .map(|value| {
            value
                .parse::<usize>()
                .expect("event count must be an integer")
        })
        .unwrap_or(1_000_000);
    let started = Instant::now();
    let dataset = Dataset::demo(count);
    println!(
        "generate: {} events, {:.2} ms, {:.2} MiB compact records",
        dataset.len(),
        started.elapsed().as_secs_f64() * 1000.0,
        dataset.record_bytes() as f64 / 1_048_576.0,
    );
    for (name, query) in [
        ("all events", Query::default()),
        (
            "text timeout",
            Query {
                text: "timeout",
                ..Query::default()
            },
        ),
        (
            "combined",
            Query {
                text: "timeout",
                level: Some(Level::Error),
                service: Some(5),
                min_duration_ms: 1000,
                ..Query::default()
            },
        ),
    ] {
        let mut samples = Vec::with_capacity(30);
        let mut matches = 0;
        for _ in 0..3 {
            black_box(dataset.query(black_box(&query)));
        }
        for _ in 0..30 {
            let started = Instant::now();
            let result = dataset.query(black_box(&query));
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
            matches = black_box(result.len());
            black_box(result);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{name}: matches={matches}, median={:.2} ms, p95={:.2} ms (30 warm samples)",
            samples[15], samples[28],
        );
    }
}
