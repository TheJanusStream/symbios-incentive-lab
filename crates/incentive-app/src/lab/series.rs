//! Chart data from a run's history, value formatting, and the metric lists
//! the views start from.

use incentive_core::{
    ARCHETYPE_COUNT, DailyMetrics, ExplainedItem, FeedExplanation, Format, MetricId, STYLE_COUNT,
};

/// A metric's daily values as chart points `[day, value]`, skipping days
/// without a value (the weekly survey, retention before day 30).
pub fn metric_points(history: &[DailyMetrics], id: MetricId) -> Vec<[f64; 2]> {
    points_by(history, |d| d.get(id))
}

/// Daily points of any per-day value, skipping NaN.
pub fn points_by(history: &[DailyMetrics], value: impl Fn(&DailyMetrics) -> f32) -> Vec<[f64; 2]> {
    history
        .iter()
        .filter_map(|d| {
            let v = value(d);
            (!v.is_nan()).then_some([f64::from(d.day), f64::from(v)])
        })
        .collect()
}

/// The share of impressions of one style, day by day.
pub fn impression_points(history: &[DailyMetrics], style: usize) -> Vec<[f64; 2]> {
    points_by(history, |d| {
        d.impression_share_by_style
            .get(style.min(STYLE_COUNT - 1))
            .copied()
            .unwrap_or(f32::NAN)
    })
}

/// The mean wellbeing of current users of one archetype, day by day.
pub fn wellbeing_points(history: &[DailyMetrics], archetype: usize) -> Vec<[f64; 2]> {
    points_by(history, |d| {
        d.wellbeing_by_archetype
            .get(archetype.min(ARCHETYPE_COUNT - 1))
            .copied()
            .unwrap_or(f32::NAN)
    })
}

/// Days in the trailing window of a smoothed chart.
pub const SMOOTH_DAYS: u32 = 7;

/// The trailing mean of chart points: each point becomes the mean of the
/// points whose day lies in the `window_days` days ending on its own day.
/// Points that are already sparser than the window (the weekly survey) stay
/// as they are.
pub fn smoothed(points: &[[f64; 2]], window_days: u32) -> Vec<[f64; 2]> {
    let window = f64::from(window_days.max(1));
    let mut start = 0;
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            while points[start][0] <= p[0] - window {
                start += 1;
            }
            // Summed afresh each time (at most a window of points), so no
            // rounding error builds up along a long run.
            let span = &points[start..=i];
            let sum: f64 = span.iter().map(|q| q[1]).sum();
            [p[0], sum / span.len() as f64]
        })
        .collect()
}

/// A "nice" step (1, 2 or 5 times a power of ten) at least `raw` long.
pub fn nice_step(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let magnitude = 10f64.powf(raw.log10().floor());
    let norm = raw / magnitude;
    let factor = if norm <= 1.0 + 1e-9 {
        1.0
    } else if norm <= 2.0 + 1e-9 {
        2.0
    } else if norm <= 5.0 + 1e-9 {
        5.0
    } else {
        10.0
    };
    factor * magnitude
}

/// An axis tick label: shares in percent, counts with thousands separators,
/// other values with as many decimals as the tick step needs.
pub fn axis_label(format: Format, value: f64, step: f64) -> String {
    let value = if value.abs() < step.abs() * 1e-6 {
        0.0
    } else {
        value
    };
    let decimals_for = |step: f64| -> usize {
        if step >= 1.0 || step <= 0.0 {
            0
        } else {
            ((-step.log10() - 1e-9).ceil().max(0.0) as usize).min(4)
        }
    };
    match format {
        Format::Share => format!("{:.*}%", decimals_for(step * 100.0), value * 100.0),
        Format::Count if step >= 1.0 => thousands(value.round() as i64),
        _ => format!("{:.*}", decimals_for(step), value),
    }
}

/// The mean of a metric over the last `days` days that have a value; NaN
/// when none has.
pub fn tail_mean(history: &[DailyMetrics], id: MetricId, days: usize) -> f32 {
    let start = history.len().saturating_sub(days);
    let (sum, n) = history[start..]
        .iter()
        .filter_map(|d| d.get_opt(id))
        .fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(v), n + 1));
    if n == 0 {
        f32::NAN
    } else {
        (sum / f64::from(n)) as f32
    }
}

/// The latest value of a metric, looking back past days without one.
pub fn latest(history: &[DailyMetrics], id: MetricId) -> f32 {
    history
        .iter()
        .rev()
        .find_map(|d| d.get_opt(id))
        .unwrap_or(f32::NAN)
}

/// A metric's value formatted for display: shares as percentages, counts
/// with thousands separators.
pub fn fmt_metric(id: MetricId, v: f32) -> String {
    fmt_format(id.info().format, v)
}

/// A value formatted by a metric format.
pub fn fmt_format(format: Format, v: f32) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    match format {
        Format::Share => format!("{:.1}%", v * 100.0),
        Format::Score => format!("{v:.3}"),
        Format::Count => thousands(v.round() as i64),
        Format::Decimal | Format::Money => {
            if v.abs() >= 100.0 {
                format!("{v:.0}")
            } else if v.abs() >= 10.0 {
                format!("{v:.1}")
            } else {
                format!("{v:.3}")
            }
        }
    }
}

/// A difference B - A formatted with its sign; shares in percentage points.
pub fn fmt_diff(id: MetricId, d: f32) -> String {
    if !d.is_finite() {
        return "-".into();
    }
    let sign = if d >= 0.0 { "+" } else { "-" };
    match id.info().format {
        Format::Share => format!("{sign}{:.1} pt", d.abs() * 100.0),
        format => format!("{sign}{}", fmt_format(format, d.abs())),
    }
}

fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Whether a metric lives in `[0, 1]` (a share or a score).
pub fn is_unit(id: MetricId) -> bool {
    matches!(id.info().format, Format::Share | Format::Score)
}

/// The metrics the compare table and the multi-seed checks report.
pub const KEY_METRICS: [MetricId; 16] = [
    MetricId::EngagementIndex,
    MetricId::HarmIndex,
    MetricId::BenefitIndex,
    MetricId::DiscourseQuality,
    MetricId::ReportedDailyActives,
    MetricId::SurveySatisfaction,
    MetricId::Retention30,
    MetricId::HumanDailyActives,
    MetricId::UserShare,
    MetricId::ToxicShare,
    MetricId::OutrageShare,
    MetricId::MisinformationShare,
    MetricId::HarassmentPer1000,
    MetricId::WellbeingEverUsers,
    MetricId::PolarisationSociety,
    MetricId::CreatorsLiving,
];

/// The platform's metrics charted before the player picks others.
pub const DEFAULT_OBSERVABLE: [MetricId; 7] = [
    MetricId::ReportedDailyActives,
    MetricId::ImpressionsPerActive,
    MetricId::LikesPerActive,
    MetricId::RepliesPerActive,
    MetricId::Retention30,
    MetricId::SurveySatisfaction,
    MetricId::Reports,
];

/// The true metrics charted before the player picks others.
pub const DEFAULT_TRUTH: [MetricId; 9] = [
    MetricId::HumanDailyActives,
    MetricId::UserShare,
    MetricId::ToxicShare,
    MetricId::OutrageShare,
    MetricId::MisinformationShare,
    MetricId::HarassmentPer1000,
    MetricId::WellbeingEverUsers,
    MetricId::PolarisationSociety,
    MetricId::InformednessNonUsers,
];

/// The metrics the compare charts overlay before the player picks others.
pub const DEFAULT_COMPARE: [MetricId; 6] = [
    MetricId::EngagementIndex,
    MetricId::HarmIndex,
    MetricId::BenefitIndex,
    MetricId::ToxicShare,
    MetricId::WellbeingEverUsers,
    MetricId::HumanDailyActives,
];

/// The platform's prediction of engagement for a feed item: the weighted
/// like, reply, share and dwell terms of its score.
pub fn predicted_engagement(item: &ExplainedItem) -> f32 {
    let c = &item.components;
    c.like + c.reply + c.share + c.dwell
}

/// The score at or above which an item counts as scored high: the top
/// quarter of the feed's scores (at least the top item).
pub fn high_score_cut(feed: &FeedExplanation) -> f32 {
    let mut scores: Vec<f32> = feed.items.iter().map(|i| i.score).collect();
    if scores.is_empty() {
        return f32::INFINITY;
    }
    scores.sort_by(|a, b| b.total_cmp(a));
    let top = scores.len().div_ceil(4).max(1);
    scores[top - 1]
}

/// The Goodhart gap in one item: the platform scored it high, yet the viewer
/// would not endorse having seen it.
pub fn is_goodhart_gap(item: &ExplainedItem, cut: f32) -> bool {
    item.score >= cut && item.truth.reflective < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use incentive_core::{Config, Simulation};

    fn history(days: u32) -> Vec<DailyMetrics> {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.run.days = days;
        let mut sim = Simulation::new(config).expect("valid");
        sim.run_to_end();
        sim.history().to_vec()
    }

    #[test]
    fn points_skip_missing_days() {
        let h = history(14);
        assert_eq!(metric_points(&h, MetricId::EngagementIndex).len(), 14);
        // The survey runs on days 6 and 13 only.
        let survey = metric_points(&h, MetricId::SurveySatisfaction);
        assert_eq!(survey.iter().map(|p| p[0]).collect::<Vec<_>>(), [6.0, 13.0]);
        assert_eq!(impression_points(&h, 3).len(), 14);
        assert!(wellbeing_points(&h, 0).len() <= 14);
        assert!(latest(&h, MetricId::SurveySatisfaction).is_finite());
        assert!(tail_mean(&h, MetricId::Retention30, 30).is_nan());
        let mean = tail_mean(&h, MetricId::EngagementIndex, 2);
        let last2 =
            (h[12].get(MetricId::EngagementIndex) + h[13].get(MetricId::EngagementIndex)) / 2.0;
        assert!((mean - last2).abs() < 1e-6);
    }

    #[test]
    fn smoothing_takes_the_trailing_mean_and_keeps_sparse_points() {
        let daily: Vec<[f64; 2]> = (0..10).map(|d| [f64::from(d), f64::from(d)]).collect();
        let s = smoothed(&daily, 7);
        assert_eq!(s.len(), 10);
        assert_eq!(s[0], [0.0, 0.0]);
        assert_eq!(s[2], [2.0, 1.0]);
        // Day 9 averages days 3 to 9.
        assert!((s[9][1] - 6.0).abs() < 1e-12);
        let weekly = [[6.0, 0.5], [13.0, 0.7], [20.0, 0.9]];
        assert_eq!(smoothed(&weekly, 7), weekly.to_vec());
        assert!(smoothed(&[], 7).is_empty());
    }

    #[test]
    fn axis_ticks_are_nice_and_readable() {
        assert_eq!(nice_step(0.03), 0.05);
        assert_eq!(nice_step(0.1), 0.1);
        assert_eq!(nice_step(140.0), 200.0);
        assert_eq!(nice_step(7.0), 10.0);
        assert_eq!(nice_step(0.0), 1.0);
        assert_eq!(axis_label(Format::Share, 0.15, 0.05), "15%");
        assert_eq!(axis_label(Format::Share, 0.005, 0.005), "0.5%");
        assert_eq!(axis_label(Format::Count, 1500.0, 500.0), "1,500");
        assert_eq!(axis_label(Format::Score, 0.25, 0.05), "0.25");
        assert_eq!(axis_label(Format::Decimal, 20.0, 5.0), "20");
        assert_eq!(axis_label(Format::Score, -1e-12, 0.1), "0.0");
    }

    #[test]
    fn values_format_by_kind() {
        assert_eq!(fmt_metric(MetricId::ToxicShare, 0.162), "16.2%");
        assert_eq!(fmt_metric(MetricId::Impressions, 15_750.4), "15,750");
        assert_eq!(fmt_metric(MetricId::HarmIndex, 0.4992), "0.499");
        assert_eq!(fmt_metric(MetricId::HarmIndex, f32::NAN), "-");
        assert_eq!(fmt_diff(MetricId::ToxicShare, -0.1613), "-16.1 pt");
        assert_eq!(fmt_diff(MetricId::HarmIndex, 0.05), "+0.050");
        assert_eq!(thousands(-1_234_567), "-1,234,567");
        assert!(is_unit(MetricId::ToxicShare) && !is_unit(MetricId::Impressions));
    }

    #[test]
    fn the_goodhart_gap_needs_a_high_score_and_negative_value() {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.run.days = 6;
        let mut sim = Simulation::new(config).expect("valid");
        sim.run_to_end();
        let feed = sim
            .tracked()
            .into_iter()
            .find_map(|id| sim.served_feed(id).cloned())
            .expect("a served feed");
        let cut = high_score_cut(&feed);
        let high = feed.items.iter().filter(|i| i.score >= cut).count();
        assert!(high >= 1 && high <= feed.items.len().div_ceil(4) + 1);
        for item in &feed.items {
            let gap = is_goodhart_gap(item, cut);
            assert_eq!(gap, item.score >= cut && item.truth.reflective < 0.0);
            let p = predicted_engagement(item);
            assert!(p.is_finite());
        }
    }
}
