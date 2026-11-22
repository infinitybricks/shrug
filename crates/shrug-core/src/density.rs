use crate::error::ShrugError;
use std::collections::BTreeMap;

/// Resolve a weight map into a probability vector aligned to `options`.
/// Implements the frozen §A6 algorithm.
pub fn resolve_weights(
    options: &[String],
    weights: &BTreeMap<String, f64>,
) -> Result<Vec<f64>, ShrugError> {
    // Step 1: validate.
    let mut any_positive = false;
    for (name, &v) in weights {
        if !options.iter().any(|o| o == name) {
            return Err(ShrugError::bad_request(format!(
                "weight references unknown option '{name}'"
            )));
        }
        if !v.is_finite() || v < 0.0 {
            return Err(ShrugError::bad_request(format!(
                "weight for '{name}' must be a finite non-negative number"
            )));
        }
        if v > 0.0 {
            any_positive = true;
        }
    }
    if !any_positive {
        return Err(ShrugError::bad_request(
            "at least one weight must be greater than zero",
        ));
    }

    // Step 2: sums and omitted set.
    let s: f64 = weights.values().sum();
    let omitted: Vec<&String> = options
        .iter()
        .filter(|o| !weights.contains_key(*o))
        .collect();

    // Step 3: branch.
    let mut p = Vec::with_capacity(options.len());
    if !omitted.is_empty() && s <= 1.0 {
        // Branch A: specified keep their weight; omitted evenly share (1 - s).
        let share = (1.0 - s) / omitted.len() as f64;
        for o in options {
            p.push(*weights.get(o).unwrap_or(&share));
        }
    } else {
        // Branch B: renormalize specified by s; omitted get 0.
        for o in options {
            p.push(weights.get(o).map(|w| w / s).unwrap_or(0.0));
        }
    }
    Ok(p)
}

/// Validate a predicate `p_true` is a finite value in [0, 1].
pub fn resolve_p_true(p_true: f64) -> Result<f64, ShrugError> {
    if !p_true.is_finite() || !(0.0..=1.0).contains(&p_true) {
        return Err(ShrugError::bad_request("p_true must be a number in [0, 1]"));
    }
    Ok(p_true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn opts() -> Vec<String> {
        vec!["red".into(), "blue".into(), "green".into()]
    }
    fn w(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }
    fn approx(a: &[f64], b: &[f64]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 1e-9, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn all_specified_sum_one_is_noop() {
        let p =
            resolve_weights(&opts(), &w(&[("red", 0.5), ("blue", 0.3), ("green", 0.2)])).unwrap();
        approx(&p, &[0.5, 0.3, 0.2]);
    }

    #[test]
    fn omitted_split_remainder() {
        let p = resolve_weights(&opts(), &w(&[("red", 0.3)])).unwrap();
        approx(&p, &[0.30, 0.35, 0.35]);
    }

    #[test]
    fn two_specified_omitted_gets_remainder() {
        let p = resolve_weights(&opts(), &w(&[("red", 0.5), ("blue", 0.3)])).unwrap();
        approx(&p, &[0.5, 0.3, 0.2]);
    }

    #[test]
    fn all_specified_sum_under_one_renormalizes() {
        let p =
            resolve_weights(&opts(), &w(&[("red", 0.2), ("blue", 0.2), ("green", 0.2)])).unwrap();
        approx(&p, &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
    }

    #[test]
    fn sum_over_one_renormalizes_and_omitted_zero() {
        let p = resolve_weights(&opts(), &w(&[("red", 0.6), ("blue", 0.6)])).unwrap();
        approx(&p, &[0.5, 0.5, 0.0]);
    }

    #[test]
    fn single_option_over_one() {
        let p = resolve_weights(&opts(), &w(&[("red", 2.0)])).unwrap();
        approx(&p, &[1.0, 0.0, 0.0]);
    }

    #[test]
    fn explicit_zero_preserved() {
        let p = resolve_weights(&opts(), &w(&[("red", 0.0), ("blue", 0.3)])).unwrap();
        approx(&p, &[0.0, 0.3, 0.7]);
    }

    #[test]
    fn omitted_sum_exactly_one_gives_zero() {
        let p = resolve_weights(&opts(), &w(&[("red", 0.5), ("blue", 0.5)])).unwrap();
        approx(&p, &[0.5, 0.5, 0.0]);
    }

    #[test]
    fn unknown_option_is_400() {
        let e = resolve_weights(&opts(), &w(&[("purple", 1.0)])).unwrap_err();
        assert_eq!(e.http_status(), 400);
    }

    #[test]
    fn all_zero_is_400() {
        let e = resolve_weights(&opts(), &w(&[("red", 0.0), ("blue", 0.0)])).unwrap_err();
        assert_eq!(e.http_status(), 400);
    }

    #[test]
    fn negative_is_400() {
        let e = resolve_weights(&opts(), &w(&[("red", -0.1)])).unwrap_err();
        assert_eq!(e.http_status(), 400);
    }

    #[test]
    fn nan_or_inf_is_400() {
        assert_eq!(
            resolve_weights(&opts(), &w(&[("red", f64::NAN)]))
                .unwrap_err()
                .http_status(),
            400
        );
        assert_eq!(
            resolve_weights(&opts(), &w(&[("red", f64::INFINITY)]))
                .unwrap_err()
                .http_status(),
            400
        );
    }

    #[test]
    fn p_true_range() {
        assert_eq!(resolve_p_true(0.0).unwrap(), 0.0);
        assert_eq!(resolve_p_true(1.0).unwrap(), 1.0);
        assert_eq!(resolve_p_true(1.5).unwrap_err().http_status(), 400);
        assert_eq!(resolve_p_true(f64::NAN).unwrap_err().http_status(), 400);
    }
}
