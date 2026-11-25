//! Statistics over seeds: the spread of one metric across runs, and the
//! paired difference of two designs run on the same seeds with its 95%
//! confidence interval. The command-line runner and the game app share them,
//! so both report the same numbers for the same runs.

use serde::Serialize;

/// Mean, standard deviation, minimum and maximum over seeds, skipping NaN.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Stats {
    /// Mean.
    pub mean: f32,
    /// Sample standard deviation (0 for a single value).
    pub sd: f32,
    /// Smallest value.
    pub min: f32,
    /// Largest value.
    pub max: f32,
}

impl Stats {
    /// Statistics of the values that are not NaN; every field is NaN when
    /// there are none.
    pub fn of(values: impl Iterator<Item = f32>) -> Self {
        let v: Vec<f64> = values.filter(|x| !x.is_nan()).map(f64::from).collect();
        if v.is_empty() {
            return Self {
                mean: f32::NAN,
                sd: f32::NAN,
                min: f32::NAN,
                max: f32::NAN,
            };
        }
        let n = v.len() as f64;
        let mean = v.iter().sum::<f64>() / n;
        let var = if v.len() > 1 {
            v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        Self {
            mean: mean as f32,
            sd: var.sqrt() as f32,
            min: v.iter().copied().fold(f64::INFINITY, f64::min) as f32,
            max: v.iter().copied().fold(f64::NEG_INFINITY, f64::max) as f32,
        }
    }
}

/// The two-sided 95% critical value of Student's t with `df` degrees of
/// freedom: a table up to 30, and beyond it the Cornish-Fisher expansion
/// around the normal quantile, within 0.001 of the exact value there. NaN for
/// no degrees of freedom.
pub fn t_critical(df: usize) -> f32 {
    const TABLE: [f32; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    if df == 0 {
        return f32::NAN;
    }
    if let Some(t) = TABLE.get(df - 1) {
        return *t;
    }
    let z = 1.959_964_f64;
    let n = df as f64;
    let (z3, z5) = (z.powi(3), z.powi(5));
    (z + (z3 + z) / (4.0 * n) + (5.0 * z5 + 16.0 * z3 + 3.0 * z) / (96.0 * n * n)) as f32
}

/// Two designs compared seed by seed: their means, the mean of the paired
/// differences B - A, and its 95% confidence interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PairedDifference {
    /// Mean of A over the seeds.
    pub mean_a: f32,
    /// Mean of B over the seeds.
    pub mean_b: f32,
    /// Mean of the paired differences B - A.
    pub diff: f32,
    /// Lower end of the 95% confidence interval of `diff` (NaN with fewer
    /// than two pairs).
    pub low: f32,
    /// Upper end of the 95% confidence interval of `diff`.
    pub high: f32,
    /// Pairs in which both values exist.
    pub pairs: usize,
}

impl PairedDifference {
    /// Compares `b[i]` with `a[i]` for every seed `i`. A pair with a NaN on
    /// either side counts in neither the difference nor the interval; the
    /// means of A and B skip their own NaN values.
    pub fn of(a: &[f32], b: &[f32]) -> Self {
        let diffs: Vec<f32> = a.iter().zip(b).map(|(x, y)| y - x).collect();
        let d = Stats::of(diffs.iter().copied());
        let pairs = diffs.iter().filter(|v| !v.is_nan()).count();
        let half = if pairs > 1 {
            t_critical(pairs - 1) * d.sd / (pairs as f32).sqrt()
        } else {
            f32::NAN
        };
        Self {
            mean_a: Stats::of(a.iter().copied()).mean,
            mean_b: Stats::of(b.iter().copied()).mean,
            diff: d.mean,
            low: d.mean - half,
            high: d.mean + half,
            pairs,
        }
    }

    /// Whether the 95% confidence interval excludes zero.
    pub fn excludes_zero(&self) -> bool {
        self.low > 0.0 || self.high < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_skip_nan() {
        let s = Stats::of([1.0, f32::NAN, 3.0].into_iter());
        assert_eq!(s.mean, 2.0);
        assert!((s.sd - std::f32::consts::SQRT_2).abs() < 1e-6);
        assert_eq!((s.min, s.max), (1.0, 3.0));
        assert!(Stats::of(std::iter::empty()).mean.is_nan());
    }

    #[test]
    fn critical_values_follow_the_table() {
        assert_eq!(t_critical(1), 12.706);
        assert_eq!(t_critical(7), 2.365);
        assert!(t_critical(0).is_nan());
        // Exact quantiles beyond the table.
        for (df, exact) in [(31, 2.0395), (40, 2.0211), (60, 2.0003), (120, 1.9799)] {
            assert!((t_critical(df) - exact).abs() < 1e-3, "df {df}");
        }
        assert!(t_critical(100_000) > 1.9599 && t_critical(100_000) < 1.961);
    }

    #[test]
    fn paired_differences_have_an_interval() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [1.5, 2.4, 3.6, 4.5];
        let d = PairedDifference::of(&a, &b);
        assert_eq!(d.pairs, 4);
        assert!((d.diff - 0.5).abs() < 1e-6);
        assert!((d.mean_a - 2.5).abs() < 1e-6 && (d.mean_b - 3.0).abs() < 1e-6);
        // Differences 0.5, 0.4, 0.6, 0.5: sd 0.0816, half-width 3.182 * sd / 2.
        let half = 3.182 * 0.081_649_66 / 2.0;
        assert!((d.high - d.diff - half).abs() < 1e-4);
        assert!(d.excludes_zero());
        let single = PairedDifference::of(&[1.0], &[2.0]);
        assert!(single.low.is_nan() && !single.excludes_zero());
        let with_gap = PairedDifference::of(&[1.0, f32::NAN, 3.0], &[2.0, 5.0, 4.0]);
        assert_eq!(with_gap.pairs, 2);
        assert_eq!(with_gap.diff, 1.0);
    }
}
