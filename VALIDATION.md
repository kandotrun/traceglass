# Validation

Recorded on 2026-10-20. This document describes the tested application and the
scope of the measurements, rather than promising identical results on other
devices.

## Automated checks

Eight `event-core` tests passed. They cover deterministic generation, search and
combined filters, histogram boundaries, JSONL import, and input limits.

Four Playwright tests passed against both the local release build and a deployed
Workers Static Assets build. The recorded public run used Chromium 141 in a
normal browser window, device scale factor 2, and completed in 44.2 seconds at [traceglass.2-38.com](https://traceglass.2-38.com).

1. Boot the real WebAssembly module and canvas; switch to a million events;
   search, filter severity, inspect a row, show zero results, and reset.
2. Import five JSONL records containing Japanese text; find `根拠` and inspect
   `SOURCE_TIMEOUT`; reject an invalid file while preserving existing data.
   No non-GET network requests occurred during the import test.
3. Search and open/close Text view at a 390 × 844 CSS-pixel viewport.
4. Measure queries over one million events and capture the rendered application.

The input fixture contains synthetic records. These checks do not establish
general accessibility conformance, universal browser support, or physical
mobile-device performance.

## Browser query measurements

Raw query samples: [`validation/browser-personal.json`](validation/browser-personal.json).
The report records the tested URL and timestamp. Earlier sanitized reports are retained for comparison.

| Metric | Recorded result |
| --- | ---: |
| Synthetic events | 1,000,000 |
| Warm query samples | 30 |
| Median query time | 4.2 ms |
| p95 query time | 7.3 ms |
| WebAssembly linear memory | 54,853,632 bytes (52.3 MiB) |

The test runs six queries—empty text, `timeout`, `retry`, `gateway`, `error`, and
`evidence`—for six rounds, discarding the first round. Rust filtering and
aggregation are timed with `performance.now()`. Input delivery, rendering,
network transfer, and first-load compilation are excluded.

The generated records are 24 bytes each on the tested targets and share message
templates. Imported JSONL retains original JSON and distinct messages, so its
memory use and performance may differ substantially. WebAssembly linear memory
is not total browser memory. Frame rate and input-to-paint latency were not
measured. The report includes a single startup observation, which is not a
controlled cold-start benchmark.

Reproduce locally after starting the app:

```sh
npx playwright test tests/performance.spec.ts --headed
```

Use `TRACEGLASS_URL` to point the same suite at another deployment.

## Rendering and delivery

- A deployed WebAssembly asset returned HTTP 200 with
  `Content-Type: application/wasm`; its SHA-256 matched the local release build.
- The recorded application selected the `BrowserWebGpu` backend.
- A manual million-event `timeout` search displayed 7,327 matches.
- Timeline selection, severity filtering, row selection, and repeated scrolling
  were exercised in a visible browser window.
- Desktop and mobile-width screenshots were visually inspected.

The initially tested WebAssembly binary was 14,731,094 bytes before compression,
with SHA-256 `5f1d16d9761fbc9ae7fc598bf5048edef9a23351b564ebf55aaba41f211fdbaa`.
This identifies that validation build, not every future build.

Small inspector glyphs initially disappeared after scrolling. Changing the
metadata to a 13 px monospace block resolved the observed reproduction, including
repeated long scrolling. The underlying GPUI cause was not established.
No speculative full-window refresh workaround remains.

On the tested system, headless Chromium sometimes captured an older composited
frame after DOM state had updated. The published screenshots therefore come
from a normal browser window.

## Current limits

- Queries and file parsing run synchronously in WebAssembly on the main thread.
- JSONL imports are limited to 32 MiB and one million records.
- Search covers message, service, and severity; it does not search trace IDs or
  arbitrary JSON fields.
- Text view exposes the first 20 results. Full scrolling and timeline interaction
  are provided by the canvas UI.
- WebGL2 fallback, Safari, Firefox, iOS, and physical mobile devices have not been
  verified.
- Imported data stays in memory for the current tab and is discarded on reload.
