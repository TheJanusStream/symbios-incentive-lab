//! Statistics across worlds: quantiles, rank correlation (Spearman) and
//! partial rank correlation (PRCC), all in f64.
//!
//! PRCC between an input and an output rank-transforms every column, regresses
//! the input's ranks and the output's ranks each on the ranks of every other
//! input (with an intercept, and with any extra control columns such as the
//! dummies of a categorical input), and correlates the two residuals: the
//! monotone association of that input with the output once the others are
//! held fixed (Marino et al. 2008).
//!
//! The rank regression ([`rank_regression`]) regresses the output's ranks on
//! the same columns at once; its R2 is the share of the output's rank
//! variance that additive monotone effects explain. A PRCC's standard error
//! under no association is taken as Fisher's for a partial correlation,
//! 1/sqrt(n - k - 2) with k the regression's terms ([`prcc_se`]).

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

/// The fit of a rank regression: the output's ranks on the ranks of every
/// input and on the controls, with an intercept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankFit {
    /// The share of the output's rank variance the regression explains.
    pub r2: f64,
    /// R2 adjusted for the terms: 1 - (1 - R2) (n - 1) / (n - k - 1); NaN
    /// when n - k - 1 is not positive.
    pub adjusted_r2: f64,
    /// The terms k besides the intercept: the columns that add something
    /// the others do not explain (a constant column adds nothing).
    pub terms: usize,
    /// The observations n.
    pub n: usize,
}

/// The rank regression of `output` on `inputs` (ranked) and `controls` (as
/// they are), with an intercept; NaN R2 for a constant output or fewer than
/// 3 observations.
pub fn rank_regression(inputs: &[Vec<f64>], controls: &[Vec<f64>], output: &[f64]) -> RankFit {
    let n = output.len();
    let ranked: Vec<Vec<f64>> = inputs.iter().map(|c| ranks(c)).collect();
    let columns: Vec<&[f64]> = ranked
        .iter()
        .map(Vec::as_slice)
        .chain(controls.iter().map(Vec::as_slice))
        .collect();
    let basis = orthonormal_basis(&columns, n);
    let terms = basis.len() - 1;
    let y = ranks(output);
    let mean = y.iter().sum::<f64>() / n.max(1) as f64;
    let total: f64 = y.iter().map(|v| (v - mean).powi(2)).sum();
    let left: f64 = residual(&y, &basis).iter().map(|v| v * v).sum();
    let r2 = if n < 3 || total <= 1e-12 {
        f64::NAN
    } else {
        1.0 - left / total
    };
    let df = n as f64 - terms as f64 - 1.0;
    let adjusted_r2 = if df > 0.0 {
        1.0 - (1.0 - r2) * (n as f64 - 1.0) / df
    } else {
        f64::NAN
    };
    RankFit {
        r2,
        adjusted_r2,
        terms,
        n,
    }
}

/// The standard error of a PRCC under no association, from `n`
/// observations and a regression of `terms` terms besides the intercept
/// (the input itself and the k - 1 columns partialled out): Fisher's
/// 1/sqrt(n - (k - 1) - 3) = 1/sqrt(n - k - 2) for a partial correlation, on
/// the z scale, where z is close to the correlation itself near 0. NaN when
/// n - k - 2 is not positive.
pub fn prcc_se(n: usize, terms: usize) -> f64 {
    let df = n as f64 - terms as f64 - 2.0;
    if df > 0.0 { 1.0 / df.sqrt() } else { f64::NAN }
}

/// The pooled standard deviation within groups: the square root of the
/// summed squared deviations from each group's mean over the summed degrees
/// of freedom (n_g - 1). Groups with a value that is not finite (NaN, or
/// infinite where a ratio's denominator was 0) or with fewer than two values
/// are skipped; NaN without any degree of freedom. With two values per group
/// it is sqrt(mean(d^2) / 2), d the groups' differences.
pub fn pooled_sd(groups: &[Vec<f64>]) -> f64 {
    let (mut squares, mut df) = (0.0, 0usize);
    for g in groups {
        if g.len() < 2 || g.iter().any(|x| !x.is_finite()) {
            continue;
        }
        let mean = g.iter().sum::<f64>() / g.len() as f64;
        squares += g.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
        df += g.len() - 1;
    }
    if df == 0 {
        f64::NAN
    } else {
        (squares / df as f64).sqrt()
    }
}

/// The medians of the chi-square distribution with 1 to 15 degrees of
/// freedom, for [`sd_consistency`].
const CHI_SQUARE_MEDIANS: [f64; 15] = [
    0.454_936_4,
    1.386_294_4,
    2.365_973_9,
    3.356_694_0,
    4.351_460_2,
    5.348_120_6,
    6.345_811_2,
    7.344_121_5,
    8.342_832_7,
    9.341_817_8,
    10.340_998_1,
    11.340_322_4,
    12.339_755_9,
    13.339_274_1,
    14.338_859_5,
];

/// The normal consistency factor of the median of sample SDs (n - 1) from
/// samples of `n` values: the median of such an SD over the population's SD,
/// sqrt(m / (n - 1)) with m the median of the chi-square distribution with
/// n - 1 degrees of freedom; 0.674 for 2 values, 0.888 for 4, 0.952 for 8.
/// Beyond 16 values m is taken from the Wilson-Hilferty approximation,
/// k (1 - 2 / (9k))^3, which puts the factor within 1.3e-4 there. NaN for
/// fewer than two values.
pub fn sd_consistency(n: usize) -> f64 {
    if n < 2 {
        return f64::NAN;
    }
    let k = n - 1;
    let median = CHI_SQUARE_MEDIANS.get(k - 1).copied().unwrap_or_else(|| {
        let k = k as f64;
        k * (1.0 - 2.0 / (9.0 * k)).powi(3)
    });
    (median / k as f64).sqrt()
}

/// A robust standard deviation within groups (docs/validation.md, 5.3): the
/// median over the groups of each group's sample SD divided by its normal
/// consistency factor ([`sd_consistency`]), so that under normal noise with
/// one SD every term has that SD as its median. Groups with a value that is
/// not finite or with fewer than two values are skipped, as in
/// [`pooled_sd`]; NaN without any group. A few groups with outlying spread,
/// such as a ratio whose denominator nears 0 in some worlds, move it little,
/// where they inflate the pooled SD.
pub fn robust_sd(groups: &[Vec<f64>]) -> f64 {
    let terms: Vec<f64> = groups
        .iter()
        .filter(|g| g.len() >= 2 && g.iter().all(|x| x.is_finite()))
        .map(|g| variance(g).sqrt() / sd_consistency(g.len()))
        .collect();
    quantile(&terms, 0.5)
}

/// The sample variance (n - 1) of the finite values; NaN for fewer than two.
pub fn variance(values: &[f64]) -> f64 {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return f64::NAN;
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64
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
    /// The rank regression explains a monotone output entirely, pure noise
    /// hardly at all (its adjusted R2 near 0), and a constant column adds no
    /// term.
    #[test]
    fn rank_regression_reads_monotone_fits() {
        let mut rng = Pcg64::seed_from_u64(5);
        let n = 300;
        let x1: Vec<f64> = (0..n).map(|_| rng.random()).collect();
        let x2: Vec<f64> = (0..n).map(|_| rng.random()).collect();
        let constant = vec![0.0; n];
        let exact: Vec<f64> = x1.iter().map(|v| v.powi(3)).collect();
        let fit = rank_regression(std::slice::from_ref(&x1), &[constant], &exact);
        assert!((fit.r2 - 1.0).abs() < 1e-9, "{fit:?}");
        assert_eq!((fit.terms, fit.n), (1, n));
        let noise: Vec<f64> = (0..n).map(|_| rng.random()).collect();
        let fit = rank_regression(&[x1.clone(), x2.clone()], &[], &noise);
        assert!(fit.r2 < 0.05 && fit.adjusted_r2.abs() < 0.03, "{fit:?}");
        // Half signal, half noise: in between, and adjusted below plain.
        let mixed: Vec<f64> = x1.iter().zip(&noise).map(|(a, b)| a + b).collect();
        let fit = rank_regression(&[x1, x2], &[], &mixed);
        assert!(fit.r2 > 0.3 && fit.r2 < 0.7, "{fit:?}");
        assert!(fit.adjusted_r2 < fit.r2);
        assert!(rank_regression(&[], &[], &[1.0; 5]).r2.is_nan());
    }

    /// The PRCC standard error is 1/sqrt(n - k - 2): with 191 worlds and 53
    /// terms, 1/sqrt(136) (docs/validation.md, 6.3). Inputs without an
    /// effect stay within 2 SE about 95% of the time; a real effect lies far
    /// beyond.
    #[test]
    fn prcc_significance_uses_two_standard_errors() {
        assert!((prcc_se(191, 53) - 1.0 / 136f64.sqrt()).abs() < 1e-12);
        assert!(prcc_se(10, 8).is_nan());
        let mut rng = Pcg64::seed_from_u64(21);
        let (n, k) = (150, 12);
        let (mut beyond, mut nulls) = (0, 0);
        for _ in 0..20 {
            let cols: Vec<Vec<f64>> = (0..k)
                .map(|_| (0..n).map(|_| rng.random()).collect())
                .collect();
            let y: Vec<f64> = (0..n)
                .map(|i| 2.0 * cols[0][i] + rng.random::<f64>())
                .collect();
            let p = prcc(&cols, &[], &y);
            let fit = rank_regression(&cols, &[], &y);
            let se = prcc_se(fit.n, fit.terms);
            assert_eq!(fit.terms, k);
            assert!(p[0] > 2.0 * se, "{p:?}");
            for r in &p[1..] {
                nulls += 1;
                beyond += usize::from(r.abs() > 2.0 * se);
            }
        }
        let rate = beyond as f64 / nulls as f64;
        assert!(rate < 0.1, "{beyond} of {nulls} null inputs beyond 2 SE");
    }

    /// Pooled within-group SD: two values per group give sqrt(mean d^2 / 2);
    /// groups of other sizes weigh by their degrees of freedom.
    #[test]
    fn pooled_sd_pools_within_groups() {
        let pairs = vec![vec![1.0, 3.0], vec![10.0, 11.0], vec![5.0, 5.0]];
        let expected = ((4.0 + 1.0 + 0.0) / 3.0 / 2.0_f64).sqrt();
        assert!((pooled_sd(&pairs) - expected).abs() < 1e-12);
        let mixed = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 6.0],
            vec![7.0],
            vec![f64::NAN, 1.0],
            vec![f64::INFINITY, 1.0],
        ];
        // (2 + 2) / (2 + 1): the groups with NaN or an infinite value are
        // skipped, so one ratio with a zero denominator cannot void the SD.
        assert!((pooled_sd(&mixed) - (4.0 / 3.0_f64).sqrt()).abs() < 1e-12);
        assert!(pooled_sd(&[vec![1.0]]).is_nan());
        assert!((variance(&[1.0, 2.0, 3.0, f64::NAN, f64::INFINITY]) - 1.0).abs() < 1e-12);
    }

    /// The consistency factors: the median of |Z| for two values, the
    /// tabled medians up to 16 values and the approximation beyond.
    #[test]
    fn consistency_factors_match_the_chi_square_medians() {
        assert!((sd_consistency(2) - 0.674_49).abs() < 1e-5);
        assert!((sd_consistency(4) - 0.888_064).abs() < 1e-6);
        assert!((sd_consistency(8) - 0.952_126).abs() < 1e-6);
        assert!((sd_consistency(16) - 0.977_714).abs() < 1e-6);
        // 32 values: the chi-square median with 31 degrees of freedom is
        // 30.33594.
        assert!((sd_consistency(32) - (30.335_94_f64 / 31.0).sqrt()).abs() < 1e-4);
        assert!(sd_consistency(1).is_nan());
    }

    /// The robust SD: the median of the scaled group SDs, which recovers the
    /// SD of normal noise on pairs and on fours, skips what pooled_sd skips,
    /// and hardly moves where a few groups spread wildly.
    #[test]
    fn robust_sd_resists_outlying_groups() {
        // Pairs with SDs 0.1, 0.2 and 0.3 (differences of sqrt 2 times the
        // SD): the median 0.2 over 0.6745.
        let s = std::f64::consts::SQRT_2;
        let pairs = vec![
            vec![0.0, 0.1 * s],
            vec![1.0, 1.0 + 0.2 * s],
            vec![2.0, 2.0 + 0.3 * s],
        ];
        assert!((robust_sd(&pairs) - 0.2 / sd_consistency(2)).abs() < 1e-12);
        let mut skipped = pairs.clone();
        skipped.extend([vec![7.0], vec![f64::NAN, 1.0], vec![f64::INFINITY, 1.0]]);
        assert!((robust_sd(&skipped) - robust_sd(&pairs)).abs() < 1e-12);
        assert!(robust_sd(&[vec![1.0]]).is_nan());
        // Normal noise with SD 1 (Box-Muller on a seeded generator).
        let mut rng = Pcg64::seed_from_u64(5);
        let mut normal = || {
            let (u, v): (f64, f64) = (rng.random(), rng.random());
            (-2.0 * (1.0 - u).ln()).sqrt() * (std::f64::consts::TAU * v).cos()
        };
        for n in [2, 4] {
            let mut groups: Vec<Vec<f64>> = (0..4000)
                .map(|_| (0..n).map(|_| normal()).collect())
                .collect();
            let sd = robust_sd(&groups);
            assert!((sd - 1.0).abs() < 0.05, "{n} values: {sd}");
            // A tenth of the groups with a hundredfold spread: the pooled SD
            // grows about tenfold, the robust one by a fraction.
            for g in groups.iter_mut().step_by(10) {
                g.iter_mut().for_each(|x| *x *= 100.0);
            }
            assert!(pooled_sd(&groups) > 20.0, "{n}: {}", pooled_sd(&groups));
            assert!(robust_sd(&groups) < 1.25, "{n}: {}", robust_sd(&groups));
        }
    }
}
