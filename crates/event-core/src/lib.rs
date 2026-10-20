//! The local-only event engine used by Traceglass, on native targets and WebAssembly.
//!
//! Generated datasets retain a compact record and a message-template index per event,
//! not a separate string, JSON value, or search document per row. [`Dataset::display`]
//! allocates strings only when a caller needs to render a row. Queries run in Rust and
//! return source indices in chronological order.
//!
//! Search is a case-insensitive ASCII substring search over **message, service and
//! level**. Whitespace-separated terms are ANDed; a term can match any of those fields.
//! Non-ASCII text matches exactly. Trace IDs are a separate field, not part of text
//! search. No imported content is executed or sent over the network.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

pub const HISTOGRAM_BINS: usize = 80;
pub const DEMO_START_MS: u64 = 1_792_454_400_000;
pub const DEMO_DURATION_MS: u64 = 3_600_000;

const SERVICES: [&str; 6] = [
    "orchestrator",
    "planner",
    "retriever",
    "tool-runner",
    "inference",
    "gateway",
];

const MESSAGES: [&str; 32] = [
    "Agent run accepted; scheduling parallel research tasks",
    "Planning step completed; next action selected",
    "Document retrieval completed from the local evidence index",
    "Tool invocation completed; structured result available",
    "Model response streamed; completion received",
    "Gateway request completed successfully",
    "Worker checkpoint persisted; execution continues",
    "Evidence sources merged into the active research context",
    "Cache hit; reusing validated tool response",
    "Parallel task joined; parent agent resumed",
    "Token budget updated after model completion",
    "Background indexing batch committed",
    "Retry scheduled after upstream rate limit",
    "Slow tool response exceeded the latency budget",
    "Retrieval returned low-confidence evidence",
    "Queue pressure detected; applying backpressure",
    "Context window nearing capacity; summarization requested",
    "Gateway connection recovered after transient failure",
    "Upstream request timeout; retry budget exhausted",
    "Tool execution failed; subprocess returned an error",
    "Model provider unavailable; circuit breaker opened",
    "Malformed tool response rejected by schema validation",
    "Evidence fetch failed; remote endpoint closed connection",
    "Worker heartbeat missed during queue saturation",
    "Trace span opened for agent planning",
    "Scheduler tick; evaluating ready task dependencies",
    "Local cache lookup completed",
    "Streaming chunk buffered for the active session",
    "Memory snapshot captured at task boundary",
    "Cancellation token checked before tool invocation",
    "Permission scope verified for local file access",
    "Prompt template expanded with current context",
];

/// Histogram and count-array order is Debug, Info, Warn, Error.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Level {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl Level {
    pub const ALL: [Self; 4] = [Self::Debug, Self::Info, Self::Warn, Self::Error];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// Also accepts the common WARNING alias.
    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("debug") {
            Some(Self::Debug)
        } else if value.eq_ignore_ascii_case("info") {
            Some(Self::Info)
        } else if value.eq_ignore_ascii_case("warn") || value.eq_ignore_ascii_case("warning") {
            Some(Self::Warn)
        } else if value.eq_ignore_ascii_case("error") {
            Some(Self::Error)
        } else {
            None
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A 24-byte generated record on common 32-bit and 64-bit targets.
///
/// `service` indexes [`Dataset::services`]. `trace_id` is a deterministic grouping
/// identifier; original imported trace strings remain accessible via `details`.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub timestamp_ms: u64,
    pub duration_ms: u32,
    pub trace_id: u32,
    message_ref: u32,
    pub service: u16,
    pub level: Level,
}

#[derive(Clone, Debug)]
pub struct EventDisplay {
    pub timestamp_ms: u64,
    pub duration_ms: u32,
    pub service: String,
    pub level: Level,
    pub trace_id: u32,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Query<'a> {
    pub text: &'a str,
    pub level: Option<Level>,
    pub service: Option<u16>,
    /// Inclusive epoch-millisecond lower bound.
    pub start_ms: Option<u64>,
    /// Inclusive epoch-millisecond upper bound.
    pub end_ms: Option<u64>,
    pub min_duration_ms: u32,
}

#[derive(Clone, Debug)]
pub struct QueryResult {
    pub indices: Vec<usize>,
    /// Counts for matched events across the *full* dataset's time range.
    /// The first timestamp maps to bin 0, the final timestamp to bin 79.
    /// A dataset with just one distinct timestamp maps to bin 0.
    pub histogram: [u32; HISTOGRAM_BINS],
    /// Counts for matched events in [`Level::ALL`] order.
    pub level_counts: [usize; 4],
    pub duration_sum_ms: u64,
    pub max_duration_ms: u32,
}

impl Default for QueryResult {
    fn default() -> Self {
        Self {
            indices: Vec::new(),
            histogram: [0; HISTOGRAM_BINS],
            level_counts: [0; 4],
            duration_sum_ms: 0,
            max_duration_ms: 0,
        }
    }
}

impl QueryResult {
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn mean_duration_ms(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.duration_sum_ms as f64 / self.len() as f64
        }
    }
}

#[derive(Debug)]
struct ImportedContent {
    message: String,
    original: Value,
}

#[derive(Debug)]
enum Source {
    Demo,
    Imported(Vec<ImportedContent>),
}

#[derive(Debug)]
pub struct Dataset {
    events: Vec<Event>,
    services: Vec<String>,
    source: Source,
}

impl Dataset {
    /// Generate deterministic synthetic events spanning one hour.
    ///
    /// A burst between 61% and 66% of the hour increases errors and durations.
    /// Adjacent sets of 12 events share a trace ID. No filesystem or network access
    /// is performed. Callers should cap `count` to fit their browser memory budget.
    pub fn demo(count: usize) -> Self {
        let denominator = count.saturating_sub(1).max(1) as u128;
        let mut events = Vec::with_capacity(count);
        for index in 0..count {
            let random = mix64(index as u64 ^ 0x7472_6163_6567_6c61);
            let position = (index as u128 * DEMO_DURATION_MS as u128 / denominator) as u64;
            let burst =
                (DEMO_DURATION_MS * 61 / 100..=DEMO_DURATION_MS * 66 / 100).contains(&position);
            let percentile = random % 100;
            let level = if percentile < if burst { 29 } else { 3 } {
                Level::Error
            } else if percentile < if burst { 55 } else { 13 } {
                Level::Warn
            } else if percentile >= 88 {
                Level::Debug
            } else {
                Level::Info
            };
            let message_ref = match level {
                Level::Info => (random >> 8) % 12,
                Level::Warn => 12 + (random >> 8) % 6,
                Level::Error => 18 + (random >> 8) % 6,
                Level::Debug => 24 + (random >> 8) % 8,
            } as u32;
            let duration_ms = if level == Level::Error {
                800 + ((random >> 24) % 12_000) as u32
            } else if level == Level::Warn {
                250 + ((random >> 24) % if burst { 5_000 } else { 1_500 }) as u32
            } else {
                2 + ((random >> 24) % 320) as u32
            };
            events.push(Event {
                timestamp_ms: DEMO_START_MS + position,
                duration_ms,
                trace_id: (index / 12) as u32,
                message_ref,
                service: ((random >> 40) % SERVICES.len() as u64) as u16,
                level,
            });
        }
        Self {
            events,
            services: SERVICES.iter().map(|value| (*value).to_owned()).collect(),
            source: Source::Demo,
        }
    }

    /// Parse newline-delimited JSON with [`ImportLimits::default`].
    ///
    /// Each nonblank line must contain an object. Required fields:
    ///
    /// - `timestamp_ms`, `timestamp`, `ts`, or `time`: nonnegative integer epoch
    ///   milliseconds, or an RFC 3339 string with an explicit `Z` / UTC offset.
    /// - `message` or `msg`: string.
    ///
    /// Optional fields:
    ///
    /// - `level` / `severity`: DEBUG, INFO (default), WARN / WARNING, ERROR.
    /// - `service` / `source`: nonempty string, default `imported`.
    /// - `duration_ms` / `duration`: nonnegative integer milliseconds, default 0.
    /// - `trace_id` / `traceId`: u32 or string (strings get a stable u32 display ID).
    ///
    /// Records are stably sorted by timestamp; equal-time rows retain input order.
    /// The complete original object, including string trace IDs and unknown fields,
    /// is available through [`Dataset::details`]. Import is all-or-nothing.
    pub fn import_jsonl(input: &str) -> Result<Self, ImportError> {
        Self::import_jsonl_with_limits(input, ImportLimits::default())
    }

    pub fn import_jsonl_with_limits(
        input: &str,
        limits: ImportLimits,
    ) -> Result<Self, ImportError> {
        if input.len() > limits.max_bytes {
            return Err(ImportError::new(
                None,
                format!("File exceeds the {} byte import limit.", limits.max_bytes),
            ));
        }
        let mut events = Vec::new();
        let mut contents = Vec::new();
        let mut services = Vec::new();
        let mut service_ids = HashMap::<String, u16>::new();

        for (line_index, line) in input.lines().enumerate() {
            let line_number = line_index + 1;
            // A UTF-8 BOM is accepted only at the beginning of the file.
            let line = if line_index == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                line
            };
            if line.trim().is_empty() {
                continue;
            }
            let error = |message: String| ImportError::new(Some(line_number), message);
            if line.len() > limits.max_line_bytes {
                return Err(error(format!(
                    "Line exceeds the {} byte limit.",
                    limits.max_line_bytes
                )));
            }
            if events.len() >= limits.max_events || events.len() >= u32::MAX as usize {
                return Err(error(format!(
                    "Import exceeds the {} event limit.",
                    limits.max_events
                )));
            }
            let original: Value = serde_json::from_str(line).map_err(|parse_error| {
                // Do not echo the user's log content into validation errors.
                error(format!("Invalid JSON: {parse_error}"))
            })?;
            let object = original
                .as_object()
                .ok_or_else(|| error("Each line must be a JSON object.".to_owned()))?;
            let field = |aliases: &[&str]| aliases.iter().find_map(|name| object.get(*name));
            let timestamp_value = field(&["timestamp_ms", "timestamp", "ts", "time"])
                .ok_or_else(|| error("Missing timestamp_ms (epoch milliseconds).".to_owned()))?;
            let timestamp_ms = match timestamp_value {
                Value::Number(value) => value.as_u64(),
                Value::String(value) => parse_rfc3339_ms(value),
                _ => None,
            }
            .ok_or_else(|| {
                error(
                    "Timestamp must be nonnegative integer epoch milliseconds or an RFC 3339 string with a timezone."
                        .to_owned(),
                )
            })?;
            let message = field(&["message", "msg"])
                .and_then(Value::as_str)
                .ok_or_else(|| error("Missing string message or msg.".to_owned()))?;
            if message.len() > limits.max_message_bytes {
                return Err(error(format!(
                    "Message exceeds the {} byte limit.",
                    limits.max_message_bytes
                )));
            }
            let level = match field(&["level", "severity"]) {
                None => Level::Info,
                Some(value) => value.as_str().and_then(Level::parse).ok_or_else(|| {
                    error("Level must be DEBUG, INFO, WARN, WARNING, or ERROR.".to_owned())
                })?,
            };
            let service_name = match field(&["service", "source"]) {
                None => "imported",
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| error("Service must be a nonempty string.".to_owned()))?,
            };
            if service_name.trim().is_empty() || service_name.len() > limits.max_service_bytes {
                return Err(error(format!(
                    "Service must be nonempty and at most {} bytes.",
                    limits.max_service_bytes
                )));
            }
            let duration_ms = match field(&["duration_ms", "duration"]) {
                None => 0,
                Some(value) => value
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        error(
                            "Duration must be integer milliseconds from 0 to 4294967295."
                                .to_owned(),
                        )
                    })?,
            };
            let trace_id = match field(&["trace_id", "traceId"]) {
                None => events.len() as u32,
                Some(Value::String(value)) => stable_string_id(value),
                Some(value) => value
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        error("Trace ID must be a string or an unsigned 32-bit integer.".to_owned())
                    })?,
            };
            let service = if let Some(id) = service_ids.get(service_name) {
                *id
            } else {
                if services.len() >= limits.max_services || services.len() >= u16::MAX as usize {
                    return Err(error(format!(
                        "Import exceeds the {} distinct service limit.",
                        limits.max_services
                    )));
                }
                let id = services.len() as u16;
                services.push(service_name.to_owned());
                service_ids.insert(service_name.to_owned(), id);
                id
            };
            events.push(Event {
                timestamp_ms,
                duration_ms,
                trace_id,
                message_ref: contents.len() as u32,
                service,
                level,
            });
            contents.push(ImportedContent {
                message: message.to_owned(),
                original,
            });
        }

        events.sort_by_key(|event| event.timestamp_ms);
        Ok(Self {
            events,
            services,
            source: Source::Imported(contents),
        })
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn services(&self) -> &[String] {
        &self.services
    }

    pub fn time_bounds(&self) -> Option<(u64, u64)> {
        Some((
            self.events.first()?.timestamp_ms,
            self.events.last()?.timestamp_ms,
        ))
    }

    pub fn event(&self, index: usize) -> Option<&Event> {
        self.events.get(index)
    }

    pub fn message(&self, index: usize) -> Option<&str> {
        self.events
            .get(index)
            .map(|event| self.event_message(event))
    }

    pub fn display(&self, index: usize) -> Option<EventDisplay> {
        let event = self.events.get(index)?;
        Some(EventDisplay {
            timestamp_ms: event.timestamp_ms,
            duration_ms: event.duration_ms,
            service: self.services[event.service as usize].clone(),
            level: event.level,
            trace_id: event.trace_id,
            message: self.event_message(event).to_owned(),
        })
    }

    /// Original JSON object for an imported event. Generated demo rows have no JSON.
    pub fn details(&self, index: usize) -> Option<&Value> {
        let event = self.events.get(index)?;
        match &self.source {
            Source::Demo => None,
            Source::Imported(contents) => Some(&contents[event.message_ref as usize].original),
        }
    }

    /// Size of the contiguous compact record buffer, excluding capacity and strings.
    pub fn record_bytes(&self) -> usize {
        self.events.len() * std::mem::size_of::<Event>()
    }

    /// Filter events and compute aggregate statistics in one pass.
    ///
    /// Time bounds narrow the scan with binary search. Demo message/service/level
    /// matches are computed once per unique value, never once per event. There is
    /// no allocation per scanned row. Matching indices and query token tables are
    /// the only query allocations.
    pub fn query(&self, query: &Query<'_>) -> QueryResult {
        let Some((first_time, last_time)) = self.time_bounds() else {
            return QueryResult::default();
        };
        if query
            .start_ms
            .zip(query.end_ms)
            .is_some_and(|(start, end)| start > end)
        {
            return QueryResult::default();
        }
        let start = query.start_ms.map_or(0, |time| {
            self.events
                .partition_point(|event| event.timestamp_ms < time)
        });
        let end = query.end_ms.map_or(self.len(), |time| {
            self.events
                .partition_point(|event| event.timestamp_ms <= time)
        });
        if start >= end {
            return QueryResult::default();
        }
        let terms: Vec<&str> = query.text.split_whitespace().collect();
        // Cache each term's membership in tiny dictionaries for generated rows.
        let service_terms: Vec<Vec<bool>> = self
            .services
            .iter()
            .map(|service| {
                terms
                    .iter()
                    .map(|term| contains_ascii_case_insensitive(service, term))
                    .collect()
            })
            .collect();
        let level_terms: Vec<Vec<bool>> = Level::ALL
            .iter()
            .map(|level| {
                terms
                    .iter()
                    .map(|term| contains_ascii_case_insensitive(level.as_str(), term))
                    .collect()
            })
            .collect();
        let demo_message_terms: Vec<Vec<bool>> = if matches!(self.source, Source::Demo) {
            MESSAGES
                .iter()
                .map(|message| {
                    terms
                        .iter()
                        .map(|term| contains_ascii_case_insensitive(message, term))
                        .collect()
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut result = QueryResult::default();
        // Unfiltered views need the whole index vector; selective searches can
        // grow from a bounded allocation rather than reserving all million rows.
        let unfiltered = terms.is_empty()
            && query.level.is_none()
            && query.service.is_none()
            && query.min_duration_ms == 0;
        result.indices.reserve(if unfiltered {
            end - start
        } else {
            (end - start).min(16_384)
        });
        for (relative_index, event) in self.events[start..end].iter().enumerate() {
            if query.level.is_some_and(|level| event.level != level)
                || query
                    .service
                    .is_some_and(|service| event.service != service)
                || event.duration_ms < query.min_duration_ms
            {
                continue;
            }
            if !terms.iter().enumerate().all(|(term_index, term)| {
                service_terms[event.service as usize][term_index]
                    || level_terms[event.level as usize][term_index]
                    || if demo_message_terms.is_empty() {
                        contains_ascii_case_insensitive(self.event_message(event), term)
                    } else {
                        demo_message_terms[event.message_ref as usize][term_index]
                    }
            }) {
                continue;
            }
            result.indices.push(start + relative_index);
            result.level_counts[event.level as usize] += 1;
            result.duration_sum_ms += u64::from(event.duration_ms);
            result.max_duration_ms = result.max_duration_ms.max(event.duration_ms);
            let bin = histogram_bin(event.timestamp_ms, first_time, last_time);
            result.histogram[bin] = result.histogram[bin].saturating_add(1);
        }
        result
    }

    fn event_message(&self, event: &Event) -> &str {
        match &self.source {
            Source::Demo => MESSAGES[event.message_ref as usize],
            Source::Imported(contents) => &contents[event.message_ref as usize].message,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImportLimits {
    pub max_bytes: usize,
    pub max_events: usize,
    pub max_line_bytes: usize,
    pub max_message_bytes: usize,
    pub max_services: usize,
    pub max_service_bytes: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
            max_events: 1_000_000,
            max_line_bytes: 256 * 1024,
            max_message_bytes: 16 * 1024,
            max_services: 256,
            max_service_bytes: 128,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportError {
    /// One-based physical line number; `None` for file-wide errors.
    pub line: Option<usize>,
    pub message: String,
}

impl ImportError {
    fn new(line: Option<usize>, message: String) -> Self {
        Self { line, message }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "Line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ImportError {}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn stable_string_id(value: &str) -> u32 {
    value.as_bytes().iter().fold(2_166_136_261, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
    })
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn histogram_bin(timestamp: u64, first: u64, last: u64) -> usize {
    if first == last {
        return 0;
    }
    // u128 avoids overflow for legitimate large epoch values; inclusive range
    // gives equal-width bins, with the maximum timestamp clamped to the last bin.
    (((timestamp - first) as u128 * HISTOGRAM_BINS as u128 / (last - first) as u128) as usize)
        .min(HISTOGRAM_BINS - 1)
}

/// Parse a bounded RFC 3339 timestamp, truncating submillisecond precision.
/// Leap seconds are deliberately rejected; JavaScript dates cannot represent them.
fn parse_rfc3339_ms(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !matches!(bytes.get(10), Some(b'T' | b't'))
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    fn number(bytes: &[u8]) -> Option<i64> {
        bytes.iter().try_fold(0_i64, |total, byte| {
            byte.is_ascii_digit()
                .then_some(total * 10 + i64::from(byte.wrapping_sub(b'0')))
        })
    }
    let year = number(&bytes[0..4])?;
    let month = number(&bytes[5..7])?;
    let day = number(&bytes[8..10])?;
    let hour = number(&bytes[11..13])?;
    let minute = number(&bytes[14..16])?;
    let second = number(&bytes[17..19])?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    if !(1..=month_days).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut position = 19;
    let mut milliseconds = 0;
    if bytes.get(position) == Some(&b'.') {
        position += 1;
        let fraction_start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            if position - fraction_start < 3 {
                milliseconds = milliseconds * 10 + i64::from(bytes[position] - b'0');
            }
            position += 1;
        }
        let digits = position - fraction_start;
        if digits == 0 {
            return None;
        }
        for _ in digits..3 {
            milliseconds *= 10;
        }
    }
    let offset_seconds = match bytes.get(position) {
        Some(b'Z' | b'z') if position + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-'))
            if position + 6 == bytes.len() && bytes[position + 3] == b':' =>
        {
            let hours = number(&bytes[position + 1..position + 3])?;
            let minutes = number(&bytes[position + 4..position + 6])?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            (hours * 3600 + minutes * 60) * if *sign == b'-' { -1 } else { 1 }
        }
        _ => return None,
    };
    // Proleptic Gregorian calendar, days since 1970-01-01.
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let timestamp = (days * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds) * 1000
        + milliseconds;
    u64::try_from(timestamp).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imported() -> Dataset {
        Dataset::import_jsonl(
            r#"{"timestamp_ms":0,"message":"Cache hit","service":"retriever","level":"debug","duration_ms":1}
{"timestamp_ms":1000,"message":"Upstream TIMEOUT","service":"gateway","level":"error","duration_ms":900}
{"timestamp_ms":2000,"message":"Upstream timeout","service":"gateway","level":"warn","duration_ms":400}
{"timestamp_ms":3000,"message":"Tool timeout","service":"tool-runner","level":"error","duration_ms":800}
{"timestamp_ms":4000,"message":"Request completed","service":"gateway","level":"info","duration_ms":20}"#,
        )
        .unwrap()
    }

    #[test]
    fn combines_all_filters_and_ands_text_terms_across_fields() {
        let dataset = imported();
        let result = dataset.query(&Query {
            text: "TIMEout GATEway",
            level: Some(Level::Error),
            service: Some(1),
            start_ms: Some(1000),
            end_ms: Some(3000),
            min_duration_ms: 500,
        });
        assert_eq!(result.indices, [1]);
        assert_eq!(result.level_counts, [0, 0, 0, 1]);
        assert_eq!(result.duration_sum_ms, 900);
        assert_eq!(result.max_duration_ms, 900);
        assert_eq!(result.mean_duration_ms(), 900.0);
        assert_eq!(result.histogram.iter().sum::<u32>(), 1);
        assert!(dataset
            .query(&Query {
                text: "timeout impossible",
                ..Query::default()
            })
            .is_empty());
    }

    #[test]
    fn inclusive_time_bounds_and_histogram_endpoints() {
        let dataset = imported();
        let result = dataset.query(&Query::default());
        assert_eq!(result.histogram[0], 1);
        assert_eq!(result.histogram[20], 1);
        assert_eq!(result.histogram[40], 1);
        assert_eq!(result.histogram[60], 1);
        assert_eq!(result.histogram[79], 1);
        assert_eq!(result.histogram.iter().sum::<u32>(), 5);
        let result = dataset.query(&Query {
            start_ms: Some(1000),
            end_ms: Some(1000),
            ..Query::default()
        });
        assert_eq!(result.indices, [1]);
        assert_eq!(result.histogram[20], 1);
        assert!(dataset
            .query(&Query {
                start_ms: Some(2000),
                end_ms: Some(1000),
                ..Query::default()
            })
            .is_empty());
        assert!(dataset
            .query(&Query {
                start_ms: Some(5000),
                ..Query::default()
            })
            .is_empty());
    }

    #[test]
    fn empty_and_single_timestamp_are_well_defined() {
        for dataset in [Dataset::demo(0), Dataset::import_jsonl("\n \n").unwrap()] {
            assert!(dataset.is_empty());
            assert_eq!(dataset.time_bounds(), None);
            assert!(dataset.query(&Query::default()).is_empty());
            assert_eq!(dataset.query(&Query::default()).mean_duration_ms(), 0.0);
            assert!(dataset.display(0).is_none());
        }
        let dataset = Dataset::demo(1);
        let result = dataset.query(&Query::default());
        assert_eq!(result.histogram[0], 1);
        assert_eq!(result.histogram.iter().sum::<u32>(), 1);
        assert_eq!(dataset.time_bounds(), Some((DEMO_START_MS, DEMO_START_MS)));
        assert_eq!(histogram_bin(u64::MAX, 0, u64::MAX), 79);
    }

    #[test]
    fn import_sorts_stably_preserves_original_and_supports_aliases() {
        let dataset = Dataset::import_jsonl(
            "\u{feff}{\"ts\":\"1970-01-01T09:00:02.125+09:00\",\"msg\":\"third\",\"source\":\"worker\",\"severity\":\"WARNING\",\"duration\":12,\"traceId\":\"span-abc\",\"payload\":{\"ok\":true}}\n\
             {\"timestamp\":1000,\"message\":\"first\"}\n\
             {\"time\":1000,\"message\":\"second\"}\n",
        ).unwrap();
        assert_eq!(dataset.len(), 3);
        assert_eq!(dataset.message(0), Some("first"));
        assert_eq!(dataset.message(1), Some("second"));
        assert_eq!(dataset.message(2), Some("third"));
        assert_eq!(dataset.event(2).unwrap().timestamp_ms, 2125);
        assert_eq!(dataset.event(2).unwrap().level, Level::Warn);
        assert_eq!(dataset.details(2).unwrap()["traceId"], "span-abc");
        assert_eq!(dataset.details(2).unwrap()["payload"]["ok"], true);
        assert_eq!(dataset.display(2).unwrap().service, "worker");
    }

    #[test]
    fn import_reports_physical_line_and_rejects_invalid_values() {
        let error = Dataset::import_jsonl("\n{\"timestamp_ms\":0,\"message\":\"ok\"}\nnot-json")
            .unwrap_err();
        assert_eq!(error.line, Some(3));
        assert!(error.to_string().starts_with("Line 3: Invalid JSON:"));
        for line in [
            "[]",
            r#"{"message":"missing time"}"#,
            r#"{"timestamp_ms":-1,"message":"negative"}"#,
            r#"{"timestamp_ms":1.2,"message":"fraction"}"#,
            r#"{"timestamp_ms":0,"message":42}"#,
            r#"{"timestamp_ms":0,"message":"x","level":"fatal"}"#,
            r#"{"timestamp_ms":0,"message":"x","service":""}"#,
            r#"{"timestamp_ms":0,"message":"x","duration_ms":-1}"#,
            r#"{"timestamp_ms":0,"message":"x","trace_id":{}}"#,
            r#"{"timestamp_ms":"2025-02-29T00:00:00Z","message":"invalid date"}"#,
            r#"{"timestamp_ms":"2026-10-20T00:00:00","message":"missing timezone"}"#,
        ] {
            assert!(
                Dataset::import_jsonl(line).is_err(),
                "unexpected acceptance: {line}"
            );
        }
    }

    #[test]
    fn import_limits_are_enforced_before_accepting_data() {
        let line = r#"{"timestamp_ms":0,"message":"hello","service":"gateway"}"#;
        for limits in [
            ImportLimits {
                max_bytes: 2,
                ..ImportLimits::default()
            },
            ImportLimits {
                max_events: 0,
                ..ImportLimits::default()
            },
            ImportLimits {
                max_line_bytes: 2,
                ..ImportLimits::default()
            },
            ImportLimits {
                max_message_bytes: 2,
                ..ImportLimits::default()
            },
            ImportLimits {
                max_services: 0,
                ..ImportLimits::default()
            },
            ImportLimits {
                max_service_bytes: 2,
                ..ImportLimits::default()
            },
        ] {
            assert!(Dataset::import_jsonl_with_limits(line, limits).is_err());
        }
    }

    #[test]
    fn rfc3339_handles_offsets_fractions_and_calendar_validation() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00.1Z"), Some(100));
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00.123999Z"), Some(123));
        assert_eq!(parse_rfc3339_ms("1970-01-01T09:30:00+09:30"), Some(0));
        assert_eq!(parse_rfc3339_ms("1969-12-31T23:30:00-00:30"), Some(0));
        assert_eq!(
            parse_rfc3339_ms("2000-02-29T00:00:00Z"),
            Some(951_782_400_000)
        );
        for invalid in [
            "1900-02-29T00:00:00Z",
            "2000-02-30T00:00:00Z",
            "2000-01-01T24:00:00Z",
            "2000-01-01T00:00:60Z",
            "2000-01-01T00:00:00.Z",
            "2000-01-01T00:00:00+24:00",
            "2000-01-01T00:00:00Zjunk",
            "1970-01-01T00:00:00+01:00",
        ] {
            assert_eq!(parse_rfc3339_ms(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn demo_is_compact_deterministic_and_has_a_measurable_anomaly() {
        let dataset = Dataset::demo(100_000);
        assert_eq!(std::mem::size_of::<Event>(), 24);
        assert_eq!(dataset.record_bytes(), 2_400_000);
        assert_eq!(
            dataset.time_bounds(),
            Some((DEMO_START_MS, DEMO_START_MS + DEMO_DURATION_MS))
        );
        assert!(dataset.details(0).is_none());
        let second = Dataset::demo(100_000);
        for index in [0, 1, 50_000, 99_999] {
            assert_eq!(dataset.message(index), second.message(index));
            assert_eq!(
                dataset.event(index).unwrap().duration_ms,
                second.event(index).unwrap().duration_ms
            );
        }
        let burst = dataset.query(&Query {
            start_ms: Some(DEMO_START_MS + DEMO_DURATION_MS * 61 / 100),
            end_ms: Some(DEMO_START_MS + DEMO_DURATION_MS * 66 / 100),
            ..Query::default()
        });
        let all = dataset.query(&Query::default());
        assert!(burst.level_counts[Level::Error as usize] as f64 / burst.len() as f64 > 0.20);
        assert!(all.level_counts[Level::Error as usize] as f64 / (all.len() as f64) < 0.08);
        assert!(
            dataset
                .query(&Query {
                    text: "TIMEOUT error",
                    ..Query::default()
                })
                .len()
                > 0
        );
        assert!(all.indices.windows(2).all(|pair| {
            dataset.event(pair[0]).unwrap().timestamp_ms
                <= dataset.event(pair[1]).unwrap().timestamp_ms
        }));
    }
}
