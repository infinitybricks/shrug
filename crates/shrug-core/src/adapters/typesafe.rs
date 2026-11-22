use crate::adapters::ParsedRequest;
use crate::error::ShrugError;
use crate::model::{Answer, Distribution, Kind, Question};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Req {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    questions: Map<String, Value>,
    #[serde(default)]
    shrug: Option<ShrugExt>,
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
    for (name, q) in req.questions {
        let qtype = q.get("type").and_then(Value::as_str).unwrap_or_default();
        let kind = match qtype {
            "choice" => {
                let options = q
                    .get("criteria")
                    .and_then(Value::as_object)
                    .map(|m| m.keys().cloned().collect::<Vec<_>>())
                    .ok_or_else(|| {
                        ShrugError::bad_request(format!("choice '{name}' needs object criteria"))
                    })?;
                Kind::Choice { options }
            }
            "score" => {
                let levels = q
                    .get("criteria")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|v| v.as_str().unwrap_or_default().to_string())
                            .collect::<Vec<_>>()
                    })
                    .ok_or_else(|| {
                        ShrugError::bad_request(format!("score '{name}' needs array criteria"))
                    })?;
                Kind::Score { levels }
            }
            "noul" => Kind::Predicate,
            other => {
                return Err(ShrugError::bad_request(format!(
                    "unsupported question type '{other}'"
                )))
            }
        };
        let distribution = dists.remove(&name).map(|s| match s {
            DistSpec::Weights { weights } => Distribution::Weights(weights),
            DistSpec::PTrue { p_true } => Distribution::PTrue(p_true),
        });
        questions.push(Question {
            name,
            kind,
            distribution,
        });
    }

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

pub fn render_response(model: Option<&str>, answers: &[Answer]) -> Value {
    let mut map = Map::new();
    for a in answers {
        let (name, value) = match a {
            Answer::Choice {
                name,
                choice,
                probabilities,
                confidence,
            } => {
                let probs: Map<String, Value> = probabilities
                    .iter()
                    .map(|(k, p)| (k.clone(), json!(p)))
                    .collect();
                (
                    name.clone(),
                    json!({ "type": "choice", "choice": choice, "confidence": confidence, "probabilities": probs }),
                )
            }
            Answer::Score {
                name,
                index,
                probabilities,
                confidence,
                ..
            } => {
                let legend: Vec<&String> = probabilities.iter().map(|(l, _)| l).collect();
                let probs: Vec<f64> = probabilities.iter().map(|(_, p)| *p).collect();
                (
                    name.clone(),
                    json!({ "type": "score", "score": index, "confidence": confidence, "legend": legend, "probabilities": probs }),
                )
            }
            Answer::Predicate { name, probability } => {
                (name.clone(), json!({ "type": "noul", "noul": probability }))
            }
        };
        map.insert(name, value);
    }
    json!({ "model": model, "answers": Value::Object(map), "usage": { "input_tokens": 0, "output_tokens": 0 } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Distribution, Kind};

    fn load(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/tests/fixtures/typesafe/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_object_questions_and_criteria() {
        let parsed = parse_request(&load("request.json")).unwrap();
        assert_eq!(parsed.model.as_deref(), Some("system-one"));
        assert_eq!(parsed.questions.len(), 3);
        let color = parsed.questions.iter().find(|q| q.name == "color").unwrap();
        assert!(
            matches!(&color.kind, Kind::Choice { options } if options == &["red","blue","green"])
        );
        assert!(
            matches!(&color.distribution, Some(Distribution::Weights(w)) if (w["red"] - 0.5).abs() < 1e-9)
        );
        let bright = parsed
            .questions
            .iter()
            .find(|q| q.name == "brightness")
            .unwrap();
        assert!(matches!(&bright.kind, Kind::Score { levels } if levels == &["dim","bright"]));
        let day = parsed
            .questions
            .iter()
            .find(|q| q.name == "is_daytime")
            .unwrap();
        assert!(matches!(day.kind, Kind::Predicate));
    }

    #[test]
    fn renders_object_keyed_answers() {
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
        assert_eq!(
            render_response(Some("system-one"), &answers),
            load("response.json")
        );
    }

    #[test]
    fn unknown_distribution_question_name_is_400() {
        let mut body = load("request.json");
        body["shrug"]["distributions"]["nope"] = serde_json::json!({ "p_true": 0.5 });
        assert_eq!(parse_request(&body).unwrap_err().http_status(), 400);
    }
}
