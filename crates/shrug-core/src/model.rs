use crate::density::{resolve_p_true, resolve_weights};
use crate::draw::{draw_categorical, draw_predicate};
use crate::error::ShrugError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Choice { options: Vec<String> },
    Score { levels: Vec<String> },
    Predicate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Distribution {
    Weights(BTreeMap<String, f64>),
    PTrue(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub name: String,
    pub kind: Kind,
    pub distribution: Option<Distribution>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Choice {
        name: String,
        choice: String,
        probabilities: Vec<(String, f64)>,
        confidence: f64,
    },
    Score {
        name: String,
        index: usize,
        label: String,
        probabilities: Vec<(String, f64)>,
        confidence: f64,
    },
    Predicate {
        name: String,
        probability: f64,
    },
}

/// Draw answers for every question. Rejects duplicate names and
/// distribution/kind mismatches with 400.
pub fn answer_all<R: rand::Rng + ?Sized>(
    rng: &mut R,
    questions: &[Question],
) -> Result<Vec<Answer>, ShrugError> {
    let mut seen = std::collections::HashSet::new();
    for q in questions {
        if !seen.insert(&q.name) {
            return Err(ShrugError::bad_request(format!(
                "duplicate question name '{}'",
                q.name
            )));
        }
    }

    questions.iter().map(|q| answer_one(rng, q)).collect()
}

fn answer_one<R: rand::Rng + ?Sized>(rng: &mut R, q: &Question) -> Result<Answer, ShrugError> {
    match &q.kind {
        Kind::Choice { options } => {
            let p = categorical_p(options, q.distribution.as_ref(), &q.name)?;
            let idx = draw_categorical(rng, &p);
            let probabilities = zip_labels(options, &p);
            Ok(Answer::Choice {
                name: q.name.clone(),
                choice: options[idx].clone(),
                confidence: p[idx],
                probabilities,
            })
        }
        Kind::Score { levels } => {
            let p = categorical_p(levels, q.distribution.as_ref(), &q.name)?;
            let idx = draw_categorical(rng, &p);
            let probabilities = zip_labels(levels, &p);
            Ok(Answer::Score {
                name: q.name.clone(),
                index: idx,
                label: levels[idx].clone(),
                confidence: p[idx],
                probabilities,
            })
        }
        Kind::Predicate => {
            let p_true = match q.distribution.as_ref() {
                None => None,
                Some(Distribution::PTrue(t)) => Some(resolve_p_true(*t)?),
                Some(Distribution::Weights(_)) => {
                    return Err(ShrugError::bad_request(format!(
                        "predicate question '{}' accepts only p_true, not weights",
                        q.name
                    )))
                }
            };
            Ok(Answer::Predicate {
                name: q.name.clone(),
                probability: draw_predicate(rng, p_true),
            })
        }
    }
}

fn categorical_p(
    options: &[String],
    dist: Option<&Distribution>,
    qname: &str,
) -> Result<Vec<f64>, ShrugError> {
    match dist {
        None => Ok(vec![1.0 / options.len() as f64; options.len()]),
        Some(Distribution::Weights(w)) => resolve_weights(options, w),
        Some(Distribution::PTrue(_)) => Err(ShrugError::bad_request(format!(
            "question '{qname}' accepts weights, not p_true"
        ))),
    }
}

fn zip_labels(labels: &[String], p: &[f64]) -> Vec<(String, f64)> {
    labels.iter().cloned().zip(p.iter().copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;
    use std::collections::BTreeMap;

    fn choice_q(name: &str, dist: Option<Distribution>) -> Question {
        Question {
            name: name.into(),
            kind: Kind::Choice {
                options: vec!["red".into(), "blue".into(), "green".into()],
            },
            distribution: dist,
        }
    }

    #[test]
    fn choice_answer_shape_and_confidence() {
        let mut rng = SmallRng::seed_from_u64(3);
        let mut w = BTreeMap::new();
        w.insert("red".to_string(), 1.0); // force red (sum>=1, others 0)
        let qs = vec![choice_q("q1", Some(Distribution::Weights(w)))];
        let answers = answer_all(&mut rng, &qs).unwrap();
        match &answers[0] {
            Answer::Choice {
                name,
                choice,
                probabilities,
                confidence,
            } => {
                assert_eq!(name, "q1");
                assert_eq!(choice, "red");
                assert_eq!(probabilities.len(), 3);
                assert!((confidence - 1.0).abs() < 1e-9);
            }
            _ => panic!("expected choice"),
        }
    }

    #[test]
    fn predicate_answer_has_no_probabilities() {
        let mut rng = SmallRng::seed_from_u64(3);
        let qs = vec![Question {
            name: "p".into(),
            kind: Kind::Predicate,
            distribution: Some(Distribution::PTrue(1.0)),
        }];
        match &answer_all(&mut rng, &qs).unwrap()[0] {
            Answer::Predicate { name, probability } => {
                assert_eq!(name, "p");
                assert!(*probability > 0.5);
            }
            _ => panic!("expected predicate"),
        }
    }

    #[test]
    fn weights_on_predicate_is_400() {
        let mut rng = SmallRng::seed_from_u64(1);
        let mut w = BTreeMap::new();
        w.insert("red".to_string(), 1.0);
        let qs = vec![Question {
            name: "p".into(),
            kind: Kind::Predicate,
            distribution: Some(Distribution::Weights(w)),
        }];
        assert_eq!(answer_all(&mut rng, &qs).unwrap_err().http_status(), 400);
    }

    #[test]
    fn p_true_on_choice_is_400() {
        let mut rng = SmallRng::seed_from_u64(1);
        let qs = vec![choice_q("q1", Some(Distribution::PTrue(0.5)))];
        assert_eq!(answer_all(&mut rng, &qs).unwrap_err().http_status(), 400);
    }

    #[test]
    fn duplicate_names_is_400() {
        let mut rng = SmallRng::seed_from_u64(1);
        let qs = vec![choice_q("dup", None), choice_q("dup", None)];
        assert_eq!(answer_all(&mut rng, &qs).unwrap_err().http_status(), 400);
    }

    #[test]
    fn score_answer_reports_index_and_label() {
        let mut rng = SmallRng::seed_from_u64(2);
        let qs = vec![Question {
            name: "s".into(),
            kind: Kind::Score {
                levels: vec!["low".into(), "high".into()],
            },
            distribution: None,
        }];
        match &answer_all(&mut rng, &qs).unwrap()[0] {
            Answer::Score {
                index,
                label,
                probabilities,
                ..
            } => {
                assert!(*index < 2);
                assert_eq!(label, &probabilities[*index].0);
            }
            _ => panic!("expected score"),
        }
    }
}
