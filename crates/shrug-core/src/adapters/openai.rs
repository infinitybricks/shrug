use crate::adapters::ParsedRequest;
use crate::error::ShrugError;
use crate::model::{Answer, Distribution, Kind, Question};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Req {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    questions: Vec<Q>,
    #[serde(default)]
    shrug: Option<ShrugExt>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Q {
    Choice {
        name: String,
        #[serde(default)]
        choices: Vec<Labeled>,
    },
    Predicate {
        name: String,
    },
    Score {
        name: String,
        #[serde(default)]
        levels: Vec<Labeled>,
    },
}

#[derive(Deserialize)]
struct Labeled {
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
struct ShrugExt {
    #[serde(default)]
    distributions: BTreeMap<String, DistSpec>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DistSpec {
    Weights { weights: BTreeMap<String, f64> },
    PTrue { p_true: f64 },
}

pub fn parse_request(body: &Value) -> Result<ParsedRequest, ShrugError> {
    let req: Req = serde_json::from_value(body.clone())
        .map_err(|e| ShrugError::bad_request(format!("invalid request: {e}")))?;

    let mut dists = req.shrug.map(|s| s.distributions).unwrap_or_default();

    let mut questions = Vec::with_capacity(req.questions.len());
    for q in req.questions {
        let (name, kind) = match q {
            Q::Choice { name, choices } => {
                let options = choices
                    .into_iter()
                    .map(|c| c.value.or(c.label).unwrap_or_default())
                    .collect();
                (name, Kind::Choice { options })
            }
            Q::Score { name, levels } => {
                let levels = levels
                    .into_iter()
                    .map(|c| c.label.or(c.value).unwrap_or_default())
                    .collect();
                (name, Kind::Score { levels })
            }
            Q::Predicate { name } => (name, Kind::Predicate),
        };
        let distribution = dists.remove(&name).map(to_distribution);
        questions.push(Question {
            name,
            kind,
            distribution,
        });
    }

    // Any distribution left over references a question not in the request.
    if let Some(orphan) = dists.keys().next() {
        return Err(ShrugError::bad_request(format!(
            "distribution references unknown question '{orphan}'"
        )));
    }

    Ok(ParsedRequest {
        model: req.model,
        questions,
    })
}

fn to_distribution(spec: DistSpec) -> Distribution {
    match spec {
        DistSpec::Weights { weights } => Distribution::Weights(weights),
        DistSpec::PTrue { p_true } => Distribution::PTrue(p_true),
    }
}

pub fn render_response(_model: Option<&str>, answers: &[Answer]) -> Value {
    let answers: Vec<Value> = answers
        .iter()
        .map(|a| match a {
            Answer::Choice {
                name,
                choice,
                probabilities,
                confidence,
            } => json!({
                "type": "choice", "name": name, "choice": choice,
                "probabilities": probabilities.iter()
                    .map(|(v, p)| json!({ "value": v, "probability": p }))
                    .collect::<Vec<_>>(),
                "confidence": confidence,
            }),
            Answer::Score {
                name,
                index,
                probabilities,
                confidence,
                ..
            } => json!({
                "type": "score", "name": name, "score": index,
                "probabilities": probabilities.iter().enumerate()
                    .map(|(i, (label, p))| json!({ "value": i, "label": label, "probability": p }))
                    .collect::<Vec<_>>(),
                "confidence": confidence,
            }),
            Answer::Predicate { name, probability } => json!({
                "type": "predicate", "name": name, "probability": probability,
            }),
        })
        .collect();

    json!({
        "answers": answers,
        "usage": { "input_tokens": 0, "input_tokens_details": { "cached_tokens": 0, "cache_write_tokens": 0 } }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Distribution, Kind};

    fn load(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/tests/fixtures/openai/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_questions_and_distributions() {
        let parsed = parse_request(&load("request.json")).unwrap();
        assert_eq!(parsed.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(parsed.questions.len(), 3);

        let color = &parsed.questions[0];
        assert_eq!(color.name, "color");
        assert!(
            matches!(&color.kind, Kind::Choice { options } if options == &["red","blue","green"])
        );
        assert!(
            matches!(&color.distribution, Some(Distribution::Weights(w)) if (w["red"] - 0.5).abs() < 1e-9)
        );

        assert!(
            matches!(parsed.questions[1].distribution, Some(Distribution::PTrue(t)) if (t - 0.7).abs() < 1e-9)
        );
        assert!(
            matches!(&parsed.questions[2].kind, Kind::Score { levels } if levels == &["dim","bright"])
        );
    }

    #[test]
    fn renders_response_shape() {
        let answers = vec![
            Answer::Choice {
                name: "color".into(),
                choice: "red".into(),
                probabilities: vec![
                    ("red".into(), 0.5),
                    ("blue".into(), 0.3),
                    ("green".into(), 0.2),
                ],
                confidence: 0.5,
            },
            Answer::Predicate {
                name: "is_daytime".into(),
                probability: 0.7,
            },
            Answer::Score {
                name: "brightness".into(),
                index: 1,
                label: "bright".into(),
                probabilities: vec![("dim".into(), 0.4), ("bright".into(), 0.6)],
                confidence: 0.6,
            },
        ];
        let rendered = render_response(Some("gpt-6-luna"), &answers);
        assert_eq!(rendered, load("response.json"));
    }

    #[test]
    fn unknown_distribution_question_name_is_400() {
        let mut body = load("request.json");
        body["shrug"]["distributions"]["nope"] = serde_json::json!({ "p_true": 0.5 });
        assert_eq!(parse_request(&body).unwrap_err().http_status(), 400);
    }
}
