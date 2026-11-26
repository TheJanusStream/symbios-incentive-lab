//! Run summaries: every metric averaged over the summary window, with how
//! much it moved across that window.

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

use super::{
    DailyMetrics, INFOS, METRIC_COUNT, MetricId, nan_mean, serialize_breakdowns, serialize_values,
};
use crate::content::STYLE_COUNT;
use crate::population::ARCHETYPE_COUNT;

/// A metric moves within its summary window when its relative change across
/// the window ([`Summary::slope`]) exceeds this share of its mean, and also
/// [`MOVING_STANDARD_ERRORS`] standard errors of that change.
pub const MOVING_THRESHOLD: f32 = 0.1;

/// How many standard errors the relative change has to exceed, besides
/// [`MOVING_THRESHOLD`], for a metric to count as moving: a change the noise
/// could produce is not a trend.
pub const MOVING_STANDARD_ERRORS: f32 = 2.0;

/// Whether a relative change across the summary window with this standard
/// error counts as moving: its size exceeds [`MOVING_THRESHOLD`] and
/// [`MOVING_STANDARD_ERRORS`] standard errors. NaN slopes never move; a NaN
/// standard error is treated as unknown and leaves the size rule alone.
pub fn is_moving_slope(slope: f32, standard_error: f32) -> bool {
    let size = slope.abs();
    size > MOVING_THRESHOLD
        && (standard_error.is_nan() || size > MOVING_STANDARD_ERRORS * standard_error)
}

/// The mean relative change of several runs and its standard error: across
/// the runs (their standard deviation over the square root of their number)
/// when there are several, the run's own regression standard error when
/// there is one. Runs without a slope are skipped; NaN for none.
fn mean_slope_and_error(pairs: impl Iterator<Item = (f32, f32)>) -> (f32, f32) {
    let pairs: Vec<(f64, f32)> = pairs
        .filter(|(slope, _)| slope.is_finite())
        .map(|(slope, se)| (f64::from(slope), se))
        .collect();
    match pairs.as_slice() {
        [] => (f32::NAN, f32::NAN),
        [(slope, se)] => (*slope as f32, *se),
        _ => {
            let n = pairs.len() as f64;
            let mean = pairs.iter().map(|p| p.0).sum::<f64>() / n;
            let var = pairs.iter().map(|p| (p.0 - mean).powi(2)).sum::<f64>() / (n - 1.0);
            (mean as f32, (var / n).sqrt() as f32)
        }
    }
}

/// Whether a metric moves across several runs' summaries (the seeds of one
/// design): the rule of [`is_moving_slope`] on the mean of their relative
/// changes with its standard error across the runs, or, for one run, its
/// regression's own standard error. The same as [`Summary::is_moving`] on
/// [`Summary::mean`] of the summaries.
pub fn is_moving_over<'a>(summaries: impl IntoIterator<Item = &'a Summary>, id: MetricId) -> bool {
    let (slope, se) = mean_slope_and_error(
        summaries
            .into_iter()
            .map(|s| (s.slope(id), s.slope_error(id))),
    );
    is_moving_slope(slope, se)
}

/// A run summary: every metric averaged over the summary window
/// (docs/model.md, section 9), skipping days without a value, and its
/// relative change across the window with that change's standard error.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// First day of the window.
    pub first_day: u32,
    /// Days averaged.
    pub days: u32,
    pub(crate) values: Vec<f32>,
    pub(crate) slopes: Vec<f32>,
    pub(crate) slope_errors: Vec<f32>,
    /// Mean wellbeing of users by archetype over the window.
    pub wellbeing_by_archetype: [f32; ARCHETYPE_COUNT],
    /// Payout share by style over the window.
    pub payout_share_by_style: [f32; STYLE_COUNT],
}

impl Summary {
    /// Averages the last `window` days of `history`.
    pub fn from_history(history: &[DailyMetrics], window: usize) -> Self {
        let start = history.len().saturating_sub(window.max(1));
        let days = &history[start..];
        let values: Vec<f32> = (0..METRIC_COUNT)
            .map(|m| nan_mean(days.iter().map(|d| d.values[m])))
            .collect();
        let (slopes, slope_errors) = INFOS
            .iter()
            .zip(&values)
            .map(|(info, &mean)| {
                let points = days
                    .iter()
                    .map(|d| (d.day as f32, d.values[info.id.index()]));
                relative_change_with_error(points, days.len(), mean, info.format.scale_floor())
            })
            .unzip();
        let wellbeing_by_archetype =
            std::array::from_fn(|a| nan_mean(days.iter().map(|d| d.wellbeing_by_archetype[a])));
        let payout_share_by_style =
            std::array::from_fn(|s| nan_mean(days.iter().map(|d| d.payout_share_by_style[s])));
        Self {
            first_day: days.first().map_or(0, |d| d.day),
            days: days.len() as u32,
            values,
            slopes,
            slope_errors,
            wellbeing_by_archetype,
            payout_share_by_style,
        }
    }

    /// The mean of several summaries (of seeds, for example), skipping NaN;
    /// its slopes are the means of theirs, with standard errors across the
    /// summaries when there are several (a single summary keeps its own).
    pub fn mean(summaries: &[Summary]) -> Option<Summary> {
        let first = summaries.first()?;
        let values = (0..METRIC_COUNT)
            .map(|m| nan_mean(summaries.iter().map(|s| s.values[m])))
            .collect();
        let (slopes, slope_errors) = (0..METRIC_COUNT)
            .map(|m| {
                mean_slope_and_error(summaries.iter().map(|s| (s.slopes[m], s.slope_errors[m])))
            })
            .unzip();
        Some(Summary {
            first_day: first.first_day,
            days: first.days,
            values,
            slopes,
            slope_errors,
            wellbeing_by_archetype: std::array::from_fn(|a| {
                nan_mean(summaries.iter().map(|s| s.wellbeing_by_archetype[a]))
            }),
            payout_share_by_style: std::array::from_fn(|i| {
                nan_mean(summaries.iter().map(|s| s.payout_share_by_style[i]))
            }),
        })
    }

    /// The averaged value of a metric; NaN when no day had one.
    pub fn get(&self, id: MetricId) -> f32 {
        self.values[id.index()]
    }

    /// The averaged value of a metric, `None` when no day had one.
    pub fn get_opt(&self, id: MetricId) -> Option<f32> {
        let v = self.get(id);
        (!v.is_nan()).then_some(v)
    }

    /// How much a metric moved across the summary window, relative to its
    /// mean: the least-squares line through the window's daily values, its
    /// rise from the window's first day to its last, divided by the larger of
    /// the metric's absolute mean and its format's floor
    /// ([`crate::Format::scale_floor`]: 1 for counts, 0.01 otherwise, so a
    /// metric near zero is judged by its absolute change). Positive when the
    /// metric rose. NaN with fewer than three days that have a value.
    pub fn slope(&self, id: MetricId) -> f32 {
        self.slopes[id.index()]
    }

    /// The standard error of [`Summary::slope`], in the same units: for one
    /// run, the least-squares standard error of the line's slope scaled like
    /// the slope (it takes the days as independent, so it is optimistic for a
    /// series whose noise persists from day to day); for a mean of runs, the
    /// standard error of the mean slope across them. NaN when unknown.
    pub fn slope_error(&self, id: MetricId) -> f32 {
        self.slope_errors[id.index()]
    }

    /// Whether a metric moved across the window by more than
    /// [`MOVING_THRESHOLD`] of its mean and by more than
    /// [`MOVING_STANDARD_ERRORS`] standard errors ([`Summary::slope_error`]):
    /// a summary of a series that is still changing.
    pub fn is_moving(&self, id: MetricId) -> bool {
        is_moving_slope(self.slope(id), self.slope_error(id))
    }
}

/// One value per metric, written as a map from metric key to value.
struct ByKey<'a>(&'a [f32]);

impl Serialize for ByKey<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        serialize_values(&mut map, self.0)?;
        map.end()
    }
}

/// The relative change of [`Summary::slope`] from `(day, value)` points
/// (NaN values skipped) over a window of `window_days` days.
#[cfg(test)]
pub(crate) fn relative_change(
    points: impl Iterator<Item = (f32, f32)>,
    window_days: usize,
    mean: f32,
    floor: f32,
) -> f32 {
    relative_change_with_error(points, window_days, mean, floor).0
}

/// [`relative_change`] and its standard error ([`Summary::slope_error`]):
/// the least-squares standard error of the slope, `sqrt(rss / (n - 2) /
/// sxx)`, scaled like the change. Both NaN with fewer than three points.
pub(crate) fn relative_change_with_error(
    points: impl Iterator<Item = (f32, f32)>,
    window_days: usize,
    mean: f32,
    floor: f32,
) -> (f32, f32) {
    let points: Vec<(f64, f64)> = points
        .filter(|(_, y)| y.is_finite())
        .map(|(x, y)| (f64::from(x), f64::from(y)))
        .collect();
    if points.len() < 3 || !mean.is_finite() {
        return (f32::NAN, f32::NAN);
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return (f32::NAN, f32::NAN);
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    let rss: f64 = points
        .iter()
        .map(|p| (p.1 - my - slope * (p.0 - mx)).powi(2))
        .sum();
    let slope_se = (rss / (n - 2.0) / sxx).sqrt();
    let span = window_days.saturating_sub(1) as f64;
    let scale = f64::from(mean.abs().max(floor));
    (
        (slope * span / scale) as f32,
        (slope_se * span / scale) as f32,
    )
}

impl Serialize for Summary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(METRIC_COUNT + 7))?;
        map.serialize_entry("first_day", &self.first_day)?;
        map.serialize_entry("days", &self.days)?;
        serialize_values(&mut map, &self.values)?;
        let moving: Vec<&str> = INFOS
            .iter()
            .filter(|info| self.is_moving(info.id))
            .map(|info| info.key)
            .collect();
        map.serialize_entry("slope", &ByKey(&self.slopes))?;
        map.serialize_entry("slope_se", &ByKey(&self.slope_errors))?;
        map.serialize_entry("moving", &moving)?;
        let zeros = [0.0; STYLE_COUNT];
        serialize_breakdowns(
            &mut map,
            &self.wellbeing_by_archetype,
            &self.payout_share_by_style,
            &zeros,
        )?;
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_steady_rise_has_its_relative_change() {
        // 0.40 to 0.49 over ten days: change 0.09 on a mean of 0.445.
        let points = (0..10).map(|d| (d as f32, 0.4 + 0.01 * d as f32));
        let r = relative_change(points, 10, 0.445, 0.01);
        assert!((r - 0.09 / 0.445).abs() < 1e-5, "{r}");
        assert!(r > MOVING_THRESHOLD);
        // A flat series does not move; a falling one is negative.
        let flat = relative_change((0..10).map(|d| (d as f32, 0.3)), 10, 0.3, 0.01);
        assert!(flat.abs() < 1e-6);
        let fall = relative_change((0..10).map(|d| (d as f32, 9.0 - d as f32)), 10, 4.5, 1.0);
        assert!((fall + 2.0).abs() < 1e-5, "{fall}");
    }

    #[test]
    fn near_zero_metrics_are_judged_by_their_absolute_change() {
        // Mean 0.0005 rising by 0.0009: relative to the floor 0.01, a 9% move.
        let points = (0..10).map(|d| (d as f32, 0.0001 * d as f32));
        let r = relative_change(points, 10, 0.000_45, 0.01);
        assert!((r - 0.09).abs() < 1e-5, "{r}");
        // Too few values, or none, give NaN, which is never moving.
        let sparse = [(0.0, 1.0), (5.0, f32::NAN), (9.0, 2.0)];
        assert!(relative_change(sparse.into_iter(), 10, 1.5, 0.01).is_nan());
    }

    #[test]
    fn summaries_record_slopes_and_moving_flags() {
        let history: Vec<DailyMetrics> = (0..40)
            .map(|d| {
                let mut m = DailyMetrics::empty(d);
                m.set(MetricId::UserShare, 0.5 + 0.005 * d as f32);
                m.set(MetricId::ToxicShare, 0.1);
                m
            })
            .collect();
        let s = Summary::from_history(&history, 30);
        // Days 10 to 39: 0.55 to 0.695, change 0.145 on a mean of 0.6225.
        assert!((s.slope(MetricId::UserShare) - 0.145 / 0.6225).abs() < 1e-4);
        assert!(s.is_moving(MetricId::UserShare));
        assert!(!s.is_moving(MetricId::ToxicShare));
        assert!(!s.is_moving(MetricId::SurveySatisfaction));
        let mean = Summary::mean(&[s.clone(), s.clone()]).expect("two summaries");
        assert_eq!(
            mean.slope(MetricId::UserShare),
            s.slope(MetricId::UserShare)
        );
        let text = serde_json::to_string(&s).unwrap_or_default();
        assert!(text.contains("\"moving\":[\"user_share\"]"), "{text}");
        assert!(text.contains("\"slope_se\":{"), "{text}");
    }

    /// A noisy daily series around `level` with `trend` added per day: a
    /// fixed pseudo-random noise of amplitude `noise`.
    fn noisy(days: u32, level: f32, trend: f32, noise: f32, phase: u32) -> Vec<DailyMetrics> {
        (0..days)
            .map(|d| {
                let mut m = DailyMetrics::empty(d);
                let k = (d
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(phase.wrapping_mul(40_503)))
                    % 1000;
                let jitter = (k as f32 / 999.0 - 0.5) * 2.0 * noise;
                m.set(MetricId::UserShare, level + trend * d as f32 + jitter);
                m
            })
            .collect()
    }

    #[test]
    fn the_regression_gives_the_slope_its_standard_error() {
        // A perfect line has no error; scattered points have some.
        let line = (0..10).map(|d| (d as f32, 0.4 + 0.01 * d as f32));
        let (r, se) = relative_change_with_error(line, 10, 0.445, 0.01);
        assert!(
            (r - 0.09 / 0.445).abs() < 1e-5 && se.abs() < 1e-5,
            "{r} {se}"
        );
        // Residuals +-0.1 alternating around a flat line over 4 days:
        // rss 0.04, sxx 5, se of the slope sqrt(0.04 / 2 / 5) = 0.0632,
        // times 3 days over the mean 1.
        let zigzag = [(0.0, 1.1), (1.0, 0.9), (2.0, 1.1), (3.0, 0.9)];
        let (r, se) = relative_change_with_error(zigzag.into_iter(), 4, 1.0, 0.01);
        assert!((r + 0.12).abs() < 1e-5, "{r}");
        let expected = ((0.04f64 - 5.0 * 0.04f64.powi(2)) / 2.0 / 5.0).sqrt() * 3.0;
        assert!((f64::from(se) - expected).abs() < 1e-5, "{se} {expected}");
    }

    #[test]
    fn a_noisy_flat_series_is_not_moving_but_a_clear_trend_is() {
        // Flat at 0.1 with noise of +-0.05: single runs can show a large
        // relative change by chance, but it does not beat two standard
        // errors, and across seeds the slopes average out.
        let flat: Vec<Summary> = (0..8)
            .map(|seed| Summary::from_history(&noisy(30, 0.1, 0.0, 0.05, seed), 30))
            .collect();
        for s in &flat {
            assert!(
                !s.is_moving(MetricId::UserShare),
                "{}",
                s.slope(MetricId::UserShare)
            );
        }
        assert!(
            flat.iter()
                .any(|s| s.slope(MetricId::UserShare).abs() > MOVING_THRESHOLD),
            "some runs pass the size rule alone"
        );
        assert!(!is_moving_over(&flat, MetricId::UserShare));
        let mean = Summary::mean(&flat).expect("eight summaries");
        assert!(!mean.is_moving(MetricId::UserShare));
        // A rise of 0.3 over 30 days on a mean of about 0.25, through the
        // same noise, moves in every run and across them.
        let trend: Vec<Summary> = (0..8)
            .map(|seed| Summary::from_history(&noisy(30, 0.1, 0.01, 0.05, seed), 30))
            .collect();
        for s in &trend {
            assert!(
                s.is_moving(MetricId::UserShare),
                "{}",
                s.slope(MetricId::UserShare)
            );
        }
        assert!(is_moving_over(&trend, MetricId::UserShare));
        assert!(
            Summary::mean(&trend)
                .expect("eight")
                .is_moving(MetricId::UserShare)
        );
    }

    #[test]
    fn the_moving_rule_needs_size_and_significance() {
        assert!(is_moving_slope(0.2, 0.05) && is_moving_slope(-0.2, 0.05));
        assert!(!is_moving_slope(0.2, 0.15), "within two standard errors");
        assert!(!is_moving_slope(0.05, 0.001), "too small");
        assert!(!is_moving_slope(f32::NAN, 0.0));
        assert!(is_moving_slope(0.2, f32::NAN), "unknown error: size alone");
        // Across seeds: slopes 0.3, -0.1, 0.4, -0.2 average 0.1 with a large
        // spread; 0.3, 0.32, 0.28, 0.3 clearly move.
        let with = |slopes: &[f32]| -> Vec<Summary> {
            slopes
                .iter()
                .map(|&v| {
                    let mut s = Summary::from_history(&noisy(30, 0.5, 0.0, 0.0, 0), 30);
                    s.slopes[MetricId::UserShare.index()] = v;
                    s.slope_errors[MetricId::UserShare.index()] = 0.0;
                    s
                })
                .collect()
        };
        assert!(!is_moving_over(
            &with(&[0.3, -0.1, 0.4, -0.2]),
            MetricId::UserShare
        ));
        assert!(is_moving_over(
            &with(&[0.3, 0.32, 0.28, 0.3]),
            MetricId::UserShare
        ));
        let one = with(&[0.3]);
        assert!(
            is_moving_over(&one, MetricId::UserShare),
            "one run: its own error"
        );
    }
}
