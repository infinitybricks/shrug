//! Access logging.
//!
//! A non-blocking, JSON-formatted tracing subscriber plus a pure, unit-tested
//! `access_line` formatter. Only server-derived fields (never a request header
//! or request body) can ever enter an access record.

use serde_json::json;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;

static GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Install a non-blocking stdout subscriber. The hot path only enqueues.
pub fn init() {
    let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());
    let _ = GUARD.set(guard);
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

/// Pure formatter for one access record. Only server-derived fields — never a
/// request header or body — can ever enter this line.
pub fn access_line(endpoint: &str, question_count: usize, status: u16) -> String {
    json!({
        "event": "access",
        "endpoint": endpoint,
        "question_count": question_count,
        "status": status,
    })
    .to_string()
}

/// Emit one access record via the non-blocking subscriber. Uses structured
/// fields (not a pre-serialized string) so the JSON subscriber emits clean,
/// non-double-encoded keys. `access_line` remains the pure, unit-tested
/// formatter (useful for non-JSON sinks and for the redaction tests).
pub fn record_access(endpoint: &str, question_count: usize, status: u16) {
    tracing::info!(
        target: "access",
        event = "access",
        endpoint,
        question_count,
        status,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_line_has_server_fields_only() {
        let line = access_line("/v1/decisions", 3, 200);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["endpoint"], "/v1/decisions");
        assert_eq!(v["question_count"], 3);
        assert_eq!(v["status"], 200);
    }

    #[test]
    fn access_line_contains_no_request_contents() {
        // The formatter only accepts server-derived fields; request contents
        // (headers, body) can never appear.
        let line = access_line("/v1/systemone", 1, 200).to_lowercase();
        assert!(!line.contains("authorization"));
        assert!(!line.contains("token"));
    }
}
