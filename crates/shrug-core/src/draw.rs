use rand::Rng;

/// Draw an index from a categorical distribution `p` (assumed to sum to ~1.0).
/// Uses inverse-CDF sampling with a single uniform draw.
pub fn draw_categorical<R: Rng + ?Sized>(rng: &mut R, p: &[f64]) -> usize {
    let u: f64 = rng.gen::<f64>(); // [0, 1)
    let mut acc = 0.0;
    for (i, &pi) in p.iter().enumerate() {
        acc += pi;
        if u < acc {
            return i;
        }
    }
    // Floating-point tail guard: return the last positive-mass index.
    p.iter()
        .enumerate()
        .rev()
        .find(|(_, &pi)| pi > 0.0)
        .map(|(i, _)| i)
        .unwrap_or(p.len() - 1)
}

/// Draw a predicate "probability of true".
/// `None` → Uniform(0,1). `Some(t)` → a value biased so that P(result > 0.5) == t:
/// with probability `t` emit a value in (0.5, 1.0]; otherwise in [0.0, 0.5).
pub fn draw_predicate<R: Rng + ?Sized>(rng: &mut R, p_true: Option<f64>) -> f64 {
    match p_true {
        None => rng.gen::<f64>(),
        Some(t) => {
            let pick_true: f64 = rng.gen::<f64>();
            let u: f64 = rng.gen::<f64>(); // [0, 1)
            if pick_true < t {
                1.0 - u * 0.5 // (0.5, 1.0]
            } else {
                u * 0.5 // [0.0, 0.5)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    fn empirical_frequencies_converge() {
        // Fixed seed + fixed N so the assertion is reproducible, not flaky.
        let mut rng = SmallRng::seed_from_u64(0xC0FFEE);
        let p = [0.5, 0.3, 0.2];
        let n = 200_000;
        let mut counts = [0u64; 3];
        for _ in 0..n {
            counts[draw_categorical(&mut rng, &p)] += 1;
        }
        let tol = 0.01;
        for i in 0..3 {
            let freq = counts[i] as f64 / n as f64;
            assert!((freq - p[i]).abs() < tol, "p[{i}]={} freq={freq}", p[i]);
        }
    }

    #[test]
    fn zero_probability_never_drawn() {
        let mut rng = SmallRng::seed_from_u64(7);
        let p = [0.0, 1.0, 0.0];
        for _ in 0..10_000 {
            assert_eq!(draw_categorical(&mut rng, &p), 1);
        }
    }

    #[test]
    fn predicate_default_is_uniform_half_above() {
        let mut rng = SmallRng::seed_from_u64(42);
        let n = 200_000;
        let above = (0..n)
            .filter(|_| draw_predicate(&mut rng, None) > 0.5)
            .count();
        let freq = above as f64 / n as f64;
        assert!((freq - 0.5).abs() < 0.01, "freq={freq}");
    }

    #[test]
    fn predicate_biased_matches_p_true() {
        let mut rng = SmallRng::seed_from_u64(99);
        let n = 200_000;
        let t = 0.7;
        let above = (0..n)
            .filter(|_| draw_predicate(&mut rng, Some(t)) > 0.5)
            .count();
        let freq = above as f64 / n as f64;
        assert!((freq - t).abs() < 0.01, "freq={freq}");
    }

    #[test]
    fn predicate_extremes() {
        let mut rng = SmallRng::seed_from_u64(1);
        for _ in 0..5_000 {
            assert!(draw_predicate(&mut rng, Some(1.0)) > 0.5);
            assert!(draw_predicate(&mut rng, Some(0.0)) <= 0.5);
        }
    }

    #[test]
    fn predicate_in_unit_interval() {
        let mut rng = SmallRng::seed_from_u64(5);
        for _ in 0..5_000 {
            let v = draw_predicate(&mut rng, Some(0.4));
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
