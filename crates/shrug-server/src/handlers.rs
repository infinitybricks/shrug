use crate::routes::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use shrug_core::adapters::{openai, typesafe, ParsedRequest};
use shrug_core::error::ShrugError;
use shrug_core::model::{answer_all, Distribution, Kind};
use std::cell::RefCell;

pub const MAX_QUESTIONS: usize = 400;
pub const MAX_OPTIONS: usize = 256;
pub const MAX_NAME_LEN: usize = 256;
pub const MAX_VALUE_LEN: usize = 256;
pub const MAX_DIST_ENTRIES: usize = 256;

thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_entropy());
}

#[derive(Clone, Copy)]
enum Api {
    OpenAi,
    TypeSafe,
}

pub async fn decisions(state: State<AppState>, body: Bytes) -> Response {
    handle(Api::OpenAi, state, body)
}

pub async fn systemone(state: State<AppState>, body: Bytes) -> Response {
    handle(Api::TypeSafe, state, body)
}

fn handle(api: Api, State(_s): State<AppState>, body: Bytes) -> Response {
    // 1. Parse JSON.
    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return error_response(ShrugError::bad_request(format!("invalid JSON: {e}"))),
    };

    // 2. Adapt to the normalized model.
    let parsed = match api {
        Api::OpenAi => openai::parse_request(&value),
        Api::TypeSafe => typesafe::parse_request(&value),
    };
    let parsed = match parsed {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };

    // 3. Enforce input caps.
    if let Err(e) = enforce_caps(&parsed) {
        return error_response(e);
    }

    // 4. Draw.
    let answers = RNG.with(|r| answer_all(&mut *r.borrow_mut(), &parsed.questions));
    let answers = match answers {
        Ok(a) => a,
        Err(e) => return error_response(e),
    };

    // 5. Render.
    let rendered = match api {
        Api::OpenAi => openai::render_response(parsed.model.as_deref(), &answers),
        Api::TypeSafe => typesafe::render_response(parsed.model.as_deref(), &answers),
    };

    // 6. Access log (server-derived fields only; see logging.rs).
    crate::logging::record_access(api_path(api), parsed.questions.len(), 200);

    (StatusCode::OK, Json(rendered)).into_response()
}

fn api_path(api: Api) -> &'static str {
    match api {
        Api::OpenAi => "/v1/decisions",
        Api::TypeSafe => "/v1/systemone",
    }
}

pub(crate) fn enforce_caps(parsed: &ParsedRequest) -> Result<(), ShrugError> {
    if parsed.questions.len() > MAX_QUESTIONS {
        return Err(ShrugError::bad_request("too many questions"));
    }
    for q in &parsed.questions {
        if q.name.len() > MAX_NAME_LEN {
            return Err(ShrugError::bad_request("question name too long"));
        }
        let labels: &[String] = match &q.kind {
            Kind::Choice { options } => options,
            Kind::Score { levels } => levels,
            Kind::Predicate => &[],
        };
        if labels.len() > MAX_OPTIONS {
            return Err(ShrugError::bad_request("too many options/levels"));
        }
        if labels.iter().any(|l| l.len() > MAX_VALUE_LEN) {
            return Err(ShrugError::bad_request("option/level value too long"));
        }
        if let Some(Distribution::Weights(w)) = &q.distribution {
            if w.len() > MAX_DIST_ENTRIES {
                return Err(ShrugError::bad_request("too many distribution entries"));
            }
        }
    }
    Ok(())
}

fn error_response(e: ShrugError) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = json!({ "error": { "type": e.error_type(), "message": e.message } });
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shrug_core::adapters::ParsedRequest;
    use shrug_core::model::{Distribution, Kind, Question};
    use std::collections::BTreeMap;

    fn req(qs: Vec<Question>) -> ParsedRequest {
        ParsedRequest {
            model: None,
            questions: qs,
        }
    }
    fn choice(name: &str, n_opts: usize) -> Question {
        Question {
            name: name.into(),
            kind: Kind::Choice {
                options: (0..n_opts).map(|i| format!("o{i}")).collect(),
            },
            distribution: None,
        }
    }

    #[test]
    fn caps_accept_normal_request() {
        assert!(enforce_caps(&req(vec![choice("q", 3)])).is_ok());
    }

    #[test]
    fn too_many_questions_is_400() {
        let qs = (0..MAX_QUESTIONS + 1)
            .map(|i| choice(&format!("q{i}"), 2))
            .collect();
        assert_eq!(enforce_caps(&req(qs)).unwrap_err().http_status(), 400);
    }

    #[test]
    fn too_many_options_is_400() {
        assert_eq!(
            enforce_caps(&req(vec![choice("q", MAX_OPTIONS + 1)]))
                .unwrap_err()
                .http_status(),
            400
        );
    }

    #[test]
    fn long_name_is_400() {
        assert_eq!(
            enforce_caps(&req(vec![choice(&"x".repeat(MAX_NAME_LEN + 1), 2)]))
                .unwrap_err()
                .http_status(),
            400
        );
    }

    #[test]
    fn long_option_value_is_400() {
        let q = Question {
            name: "q".into(),
            kind: Kind::Choice {
                options: vec!["ok".into(), "y".repeat(MAX_VALUE_LEN + 1)],
            },
            distribution: None,
        };
        assert_eq!(enforce_caps(&req(vec![q])).unwrap_err().http_status(), 400);
    }

    #[test]
    fn too_many_distribution_entries_is_400() {
        let mut w = BTreeMap::new();
        for i in 0..MAX_DIST_ENTRIES + 1 {
            w.insert(format!("o{i}"), 1.0);
        }
        let q = Question {
            name: "q".into(),
            kind: Kind::Choice {
                options: (0..MAX_OPTIONS).map(|i| format!("o{i}")).collect(),
            },
            distribution: Some(Distribution::Weights(w)),
        };
        assert_eq!(enforce_caps(&req(vec![q])).unwrap_err().http_status(), 400);
    }
}
