//! Statistics across worlds: quantiles, rank correlation (Spearman) and
//! partial rank correlation (PRCC), all in f64.
//!
//! PRCC between an input and an output rank-transforms every column, regresses
//! the input's ranks and the output's ranks each on the ranks of every other
//! input (with an intercept, and with any extra control columns such as the
//! dummies of a categorical input), and correlates the two residuals: the
//! monotone association of that input with the output once the others are
//! held fixed (Marino et al. 2008).

/// The `q` quantile of the values that are not NaN, interpolating linearly
/// between order statistics (type 7 of Hyndman and Fan); NaN for none.
pub fn quantile(values: &[f64], q: f64) -> f64 {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let h = (v.len() - 1) as f64 * q.clamp(0.0, 1.0);
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    v[lo] + (h - lo as f64) * (v[hi] - v[lo])
}

/// Ranks from 1, ties sharing their average rank.
pub fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut out = vec![0.0; values.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && values[order[j + 1]] == values[order[i]] {
            j += 1;
        }
        let rank = (i + j) as f64 / 2.0 + 1.0;
        for &k in &order[i..=j] {
            out[k] = rank;
        }
        i = j + 1;
    }
    out
}

/// Pearson correlation; NaN when either side is constant or the lengths
/// differ or are below 3.
pub fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n != y.len() || n < 3 {
        return f64::NAN;
    }
    let (mx, my) = (
        x.iter().sum::<f64>() / n as f64,
        y.iter().sum::<f64>() / n as f64,
    );
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    let scale = (sxx * syy).sqrt();
    if scale <= 1e-12 * (1.0 + sxx.max(syy)) {
        f64::NAN
    } else {
        sxy / scale
    }
}

/// Spearman's rank correlation: Pearson's on the average ranks.
pub fn spearman(x: &[f64], y: &[f64]) -> f64 {
    pearson(&ranks(x), &ranks(y))
}

/// Partial rank correlation of every input column with the output.
/// `inputs[j]` is input `j`'s values over the observations; `controls` are
/// further columns, used as they are (not ranked), that every regression
/// also holds fixed. NaN for an input whose ranks the other columns explain
/// entirely.
pub fn prcc(inputs: &[Vec<f64>], controls: &[Vec<f64>], output: &[f64]) -> Vec<f64> {
    let ranked: Vec<Vec<f64>> = inputs.iter().map(|c| ranks(c)).collect();
    let y = ranks(output);
    (0..ranked.len())
        .map(|j| {
            let others: Vec<&[f64]> = ranked
                .iter()
                .enumerate()
                .filter(|&(k, _)| k != j)
                .map(|(_, c)| c.as_slice())
                .chain(controls.iter().map(Vec::as_slice))
                .collect();
            let basis = orthonormal_basis(&others, y.len());
            pearson(&residual(&ranked[j], &basis), &residual(&y, &basis))
        })
        .collect()
}

/// An orthonormal basis of the span of a constant column and `columns`, by
/// modified Gram-Schmidt; columns that add (almost) nothing are dropped.
fn orthonormal_basis(columns: &[&[f64]], n: usize) -> Vec<Vec<f64>> {
    let mut basis: Vec<Vec<f64>> = vec![vec![1.0 / (n as f64).sqrt(); n]];
    for column in columns {
        let scale = column.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut v = column.to_vec();
        for b in &basis {
            let dot: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
            v.iter_mut().zip(b).for_each(|(x, y)| *x -= dot * y);
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-9 * scale.max(1.0) {
            v.iter_mut().for_each(|x| *x /= norm);
            basis.push(v);
        }
    }
    basis
}

/// `v` minus its projection on the orthonormal `basis`.
fn residual(v: &[f64], basis: &[Vec<f64>]) -> Vec<f64> {
    let mut r = v.to_vec();
    for b in basis {
        let dot: f64 = r.iter().zip(b).map(|(x, y)| x * y).sum();
        r.iter_mut().zip(b).for_each(|(x, y)| *x -= dot * y);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_pcg::Pcg64;

    #[test]
    fn quantiles_interpolate() {
        let v = [3.0, 1.0, 2.0, 4.0, f64::NAN];
        assert_eq!(quantile(&v, 0.5), 2.5);
        assert_eq!(quantile(&v, 0.0), 1.0);
        assert_eq!(quantile(&v, 1.0), 4.0);
        assert!((quantile(&v, 0.05) - 1.15).abs() < 1e-12);
        assert!(quantile(&[], 0.5).is_nan());
    }

    #[test]
    fn ties_share_their_average_rank() {
        assert_eq!(ranks(&[10.0, 20.0, 10.0, 5.0]), vec![2.5, 4.0, 2.5, 1.0]);
    }

    #[test]
    fn spearman_reads_monotone_relations() {
        let x: Vec<f64> = (1..=20).map(f64::from).collect();
        let cubic: Vec<f64> = x.iter().map(|v| v.powi(3)).collect();
        assert!((spearman(&x, &cubic) - 1.0).abs() < 1e-12);
        let falling: Vec<f64> = x.iter().map(|v| (-v).exp()).collect();
        assert!((spearman(&x, &falling) + 1.0).abs() < 1e-12);
        // A textbook example without ties: d^2 sums to 2 over 5 pairs, so
        // rho = 1 - 6 * 2 / (5 * 24) = 0.9.
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [2.0, 1.0, 3.0, 4.0, 5.0];
        assert!((spearman(&a, &b) - 0.9).abs() < 1e-12);
        assert!(spearman(&a, &[1.0; 5]).is_nan());
    }

    /// y depends strongly on x1, weakly and negatively on x2, not at all on
    /// x3; x4 is correlated with x1 but has no effect of its own. Spearman
    /// sees x4 through x1; PRCC does not.
    #[test]
    fn prcc_separates_correlated_inputs() {
        let mut rng = Pcg64::seed_from_u64(11);
        let n = 400;
        let mut cols = vec![Vec::new(); 4];
        let mut y = Vec::new();
        for _ in 0..n {
            let x1: f64 = rng.random();
            let x2: f64 = rng.random();
            let x3: f64 = rng.random();
            let x4 = x1 + 0.3 * rng.random::<f64>();
            let noise: f64 = rng.random::<f64>() - 0.5;
            y.push((3.0 * x1).exp() - 2.0 * x2 + 0.5 * noise);
            for (c, v) in cols.iter_mut().zip([x1, x2, x3, x4]) {
                c.push(v);
            }
        }
        let p = prcc(&cols, &[], &y);
        let s: Vec<f64> = cols.iter().map(|c| spearman(c, &y)).collect();
        assert!(p[0] > 0.9, "{p:?}");
        assert!(p[1] < -0.5, "{p:?}");
        assert!(p[2].abs() < 0.15, "{p:?}");
        assert!(p[3].abs() < 0.15, "{p:?}");
        assert!(s[3] > 0.7, "Spearman sees the proxy: {s:?}");
    }

    /// A control column holds a categorical effect fixed: without it the
    /// group shift hides x's effect, with it PRCC recovers it.
    #[test]
    fn prcc_holds_controls_fixed() {
        let n = 200;
        let x: Vec<f64> = (0..n).map(|i| f64::from(i % 50)).collect();
        let group: Vec<f64> = (0..n).map(|i| f64::from(i / 50 % 2)).collect();
        let other: Vec<f64> = (0..n).map(|i| f64::from((i * 37) % 101)).collect();
        let y: Vec<f64> = x
            .iter()
            .zip(&group)
            .zip(&other)
            .map(|((x, g), o)| x * 0.1 + 100.0 * g + 0.001 * o)
            .collect();
        let with = prcc(&[x.clone(), other.clone()], &[group], &y);
        assert!(with[0] > 0.99, "{with:?}");
        let without = prcc(&[x, other], &[], &y);
        assert!(without[0] < with[0]);
    }
}
