//! The stylized facts the model is checked against (docs/model.md, section
//! 15), as data: each fact names its variants (paired runs on the same seeds)
//! and its checks. Callers run the simulations however they like (the tests
//! sequentially, the command-line runner in parallel) and hand the histories
//! to [`Fact::evaluate`].
//!
//! Each check has two readings: `passed` judges the effect on the seed means
//! alone, as the regression tests do; `verdict` judges the 95% t-interval of
//! the per-seed effect against the same bounds, as scenario verdicts judge
//! their margins: pass when the interval lies inside, fail when it lies
//! wholly outside, undecided otherwise.

use serde::Serialize;

use crate::config::{Config, Deactivation};
use crate::error::ConfigError;
use crate::metrics::{DailyMetrics, MetricId, nan_mean};
use crate::scenarios::Verdict;
use crate::simulation::Simulation;
use crate::stats::MeanInterval;

/// One arm of a fact's paired comparison.
#[derive(Clone, Copy, Debug)]
pub struct Variant {
    /// Short label.
    pub label: &'static str,
    apply: fn(&mut Config),
}

/// What a check measures in one run.
#[derive(Clone, Copy, Debug)]
enum Measure {
    /// Mean of a metric over a share of the run, `[from, to)` as fractions.
    Window(MetricId, f32, f32),
    /// Mean over the last fifth of the run minus mean over the first fifth.
    Rise(MetricId),
    /// Mean over days `[from, to)`, clipped to the run.
    Days(MetricId, u32, u32),
    /// The value on the run's last day.
    Last(MetricId),
}

impl Measure {
    fn of(self, history: &[DailyMetrics]) -> f32 {
        let n = history.len();
        let mean = |id: MetricId, from: usize, to: usize| {
            let to = to.clamp(1, n.max(1));
            let from = from.min(to - 1);
            nan_mean(history[from..to].iter().map(|d| d.get(id)))
        };
        match self {
            Self::Window(id, from, to) => mean(
                id,
                (from * n as f32) as usize,
                (to * n as f32).ceil() as usize,
            ),
            Self::Rise(id) => {
                let fifth = (n / 5).max(1);
                mean(id, n - fifth, n) - mean(id, 0, fifth)
            }
            Self::Days(id, from, to) => mean(id, from as usize, to as usize),
            Self::Last(id) => history.last().map_or(f32::NAN, |d| d.get(id)),
        }
    }
}

/// The effect a band check measures from the per-seed measurements.
#[derive(Clone, Copy, Debug)]
enum Effect {
    /// Paired mean of `a - b`.
    Difference { a: usize, b: usize },
    /// Mean of variant `a`.
    Mean { a: usize },
    /// Mean of `a` divided by mean of `b`.
    Ratio { a: usize, b: usize },
    /// Relative difference of the means, `(mean a - mean b) / mean b`.
    Relative { a: usize, b: usize },
}

/// How a check judges the per-seed measurements of two variants.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Not every kind of judge is in use at every calibration.
enum Judge {
    /// Paired mean of `a - b` above the margin.
    Exceeds { a: usize, b: usize, margin: f32 },
    /// Mean of variant `a` above the margin.
    Above { a: usize, margin: f32 },
    /// Mean of `a` divided by mean of `b` below the ratio.
    RatioBelow { a: usize, b: usize, ratio: f32 },
    /// Absolute relative difference of the means of `a` and `b` below the tolerance.
    Close { a: usize, b: usize, tolerance: f32 },
    /// The effect strictly between a lower and an upper bound.
    Band { effect: Effect, low: f32, high: f32 },
}

/// One check of a fact.
#[derive(Clone, Copy, Debug)]
pub struct Check {
    /// What the check claims.
    pub label: &'static str,
    measure: Measure,
    judge: Judge,
    /// Whether the model is known to fail this check (docs/model.md, section
    /// 16): it is judged and reported like any other, but the tests do not
    /// require it to hold.
    pub known_limit: bool,
}

/// The verdict on one check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckResult {
    /// What the check claims.
    pub label: &'static str,
    /// The effect: a paired mean difference, a mean, a ratio or a relative
    /// difference, depending on the check.
    pub effect: f32,
    /// Standard deviation of the per-seed effect: of the paired differences,
    /// of the values, of the per-seed ratios `a / b` for a ratio, or of the
    /// per-seed relative differences `(a - b) / b` for a relative difference.
    pub spread: f32,
    /// The value the effect had to beat or stay under; for a band, its
    /// lower bound.
    pub threshold: f32,
    /// Where the effect had to lie.
    pub bound: Bound,
    /// The mean of the per-seed effect with its 95% t-interval: of the
    /// paired differences, of the values, of the per-seed ratios `a / b`, or
    /// of the per-seed relative differences `(a - b) / b` (signed, also for
    /// a check that bounds their absolute value). For a ratio or a relative
    /// difference its mean is the mean of the per-seed values, which need not
    /// equal `effect`, the ratio of the means.
    pub interval: MeanInterval,
    /// Whether the effect on the seed means lies within the bound.
    pub passed: bool,
    /// The verdict on [`CheckResult::interval`]: pass when the interval lies
    /// within the bound, fail when it lies wholly outside it, undecided when
    /// it straddles a bound (or with fewer than two seeds). A check that
    /// bounds an absolute relative difference below a tolerance judges the
    /// signed interval against minus to plus the tolerance.
    pub verdict: Verdict,
    /// Whether the model is known to fail this check ([`Check::known_limit`]).
    pub known_limit: bool,
}

/// Where a check's effect has to lie.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Bound {
    /// Above this value.
    Above(f32),
    /// Below this value.
    Below(f32),
    /// Strictly between a lower and an upper bound.
    Between(f32, f32),
}

impl Bound {
    /// The bound as short text: `> 0.0200`, `< 0.9000` or `0.5000 to 0.8000`.
    pub fn text(self) -> String {
        match self {
            Self::Above(v) => format!("> {v:.4}"),
            Self::Below(v) => format!("< {v:.4}"),
            Self::Between(low, high) => format!("{low:.4} to {high:.4}"),
        }
    }

    /// The verdict on a 95% interval `[low, high]` of the effect: pass when
    /// it lies within the bound, fail when it lies wholly outside, undecided
    /// otherwise and without an interval. The bounds are strict, so an
    /// interval that ends exactly on one passes only on the outer side.
    pub fn verdict(self, low: f32, high: f32) -> Verdict {
        if low.is_nan() || high.is_nan() {
            return Verdict::Undecided;
        }
        let (pass, fail) = match self {
            Self::Above(t) => (low > t, high <= t),
            Self::Below(t) => (high < t, low >= t),
            Self::Between(a, b) => (low > a && high < b, high <= a || low >= b),
        };
        if pass {
            Verdict::Pass
        } else if fail {
            Verdict::Fail
        } else {
            Verdict::Undecided
        }
    }
}

/// A stylized fact.
#[derive(Clone, Copy, Debug)]
pub struct Fact {
    /// Stable id.
    pub id: &'static str,
    /// Title.
    pub title: &'static str,
    /// The empirical finding it reproduces.
    pub reference: &'static str,
    /// The paired arms.
    pub variants: &'static [Variant],
    /// The checks.
    pub checks: &'static [Check],
}

/// Run size of a fact check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactRun {
    /// Seeds, paired across variants.
    pub seeds: Vec<u64>,
    /// Individuals per run.
    pub population: u32,
    /// Days per run.
    pub days: u32,
}

impl Fact {
    /// The configuration of each variant for one seed.
    pub fn configs(&self, run: &FactRun, seed: u64) -> Vec<Config> {
        self.configs_from(&Config::default(), run, seed)
    }

    /// The configuration of each variant for one seed, starting from `base`
    /// (a world in which to check the fact) instead of the defaults; each
    /// variant's change applies on top.
    pub fn configs_from(&self, base: &Config, run: &FactRun, seed: u64) -> Vec<Config> {
        self.variants
            .iter()
            .map(|v| {
                let mut config = base.clone();
                config.run.seed = seed;
                config.run.population_size = run.population;
                config.run.days = run.days;
                config.run.tracked_agents = 0;
                (v.apply)(&mut config);
                config
            })
            .collect()
    }

    /// Judges the histories, indexed `[variant][seed]`.
    pub fn evaluate(&self, histories: &[Vec<Vec<DailyMetrics>>]) -> Vec<CheckResult> {
        self.checks.iter().map(|c| judge(c, histories)).collect()
    }

    /// Runs every variant on every seed, one after the other, and judges them.
    pub fn run_sequential(&self, run: &FactRun) -> Result<Vec<CheckResult>, ConfigError> {
        let mut histories = vec![Vec::new(); self.variants.len()];
        for &seed in &run.seeds {
            for (v, config) in self.configs(run, seed).into_iter().enumerate() {
                let mut sim = Simulation::new(config)?;
                sim.run_to_end();
                histories[v].push(sim.history().to_vec());
            }
        }
        Ok(self.evaluate(&histories))
    }
}

fn mean_sd(values: &[f32]) -> (f32, f32) {
    let n = values.len() as f32;
    if values.is_empty() {
        return (f32::NAN, 0.0);
    }
    let mean = values.iter().sum::<f32>() / n;
    let var = if values.len() > 1 {
        values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (n - 1.0)
    } else {
        0.0
    };
    (mean, var.sqrt())
}

impl Effect {
    /// The effect, its per-seed spread and the per-seed effects from the
    /// measurements, indexed `[variant][seed]`.
    fn of(self, values: &dyn Fn(usize) -> Vec<f32>) -> (f32, f32, Vec<f32>) {
        let paired = |a: usize, b: usize, f: &dyn Fn(f32, f32) -> f32| -> Vec<f32> {
            values(a)
                .iter()
                .zip(values(b))
                .map(|(x, y)| f(*x, y))
                .collect()
        };
        let with_spread = |effect: f32, per_seed: Vec<f32>| {
            let sd = mean_sd(&per_seed).1;
            (effect, sd, per_seed)
        };
        match self {
            Self::Difference { a, b } => {
                let d = paired(a, b, &|x, y| x - y);
                with_spread(mean_sd(&d).0, d)
            }
            Self::Mean { a } => {
                let v = values(a);
                with_spread(mean_sd(&v).0, v)
            }
            Self::Ratio { a, b } => {
                let r = mean_sd(&values(a)).0 / mean_sd(&values(b)).0;
                with_spread(r, paired(a, b, &|x, y| x / y))
            }
            Self::Relative { a, b } => {
                let (ma, mb) = (mean_sd(&values(a)).0, mean_sd(&values(b)).0);
                with_spread((ma - mb) / mb, paired(a, b, &|x, y| (x - y) / y))
            }
        }
    }
}

fn judge(check: &Check, histories: &[Vec<Vec<DailyMetrics>>]) -> CheckResult {
    let values = |v: usize| -> Vec<f32> {
        histories
            .get(v)
            .map(|runs| runs.iter().map(|h| check.measure.of(h)).collect())
            .unwrap_or_default()
    };
    // The bound the interval is judged against differs from the reported
    // bound only for `Close`, whose interval is of the signed difference.
    let (effect, spread, per_seed, bound, interval_bound) = match check.judge {
        Judge::Exceeds { a, b, margin } => {
            let (m, sd, v) = Effect::Difference { a, b }.of(&values);
            (m, sd, v, Bound::Above(margin), Bound::Above(margin))
        }
        Judge::Above { a, margin } => {
            let (m, sd, v) = Effect::Mean { a }.of(&values);
            (m, sd, v, Bound::Above(margin), Bound::Above(margin))
        }
        Judge::RatioBelow { a, b, ratio } => {
            let (r, sd, v) = Effect::Ratio { a, b }.of(&values);
            (r, sd, v, Bound::Below(ratio), Bound::Below(ratio))
        }
        Judge::Close { a, b, tolerance } => {
            let (rel, sd, v) = Effect::Relative { a, b }.of(&values);
            let band = Bound::Between(-tolerance, tolerance);
            (rel.abs(), sd, v, Bound::Below(tolerance), band)
        }
        Judge::Band { effect, low, high } => {
            let (m, sd, v) = effect.of(&values);
            let band = Bound::Between(low, high);
            (m, sd, v, band, band)
        }
    };
    let interval = MeanInterval::of(per_seed.into_iter());
    let verdict = interval_bound.verdict(interval.low, interval.high);
    let (threshold, passed) = match bound {
        Bound::Above(t) => (t, effect > t),
        Bound::Below(t) => (t, effect < t),
        Bound::Between(low, high) => (low, effect > low && effect < high),
    };
    CheckResult {
        label: check.label,
        effect,
        spread,
        threshold,
        bound,
        interval,
        passed,
        verdict,
        known_limit: check.known_limit,
    }
}

/// Every stylized fact, in the order of docs/model.md, section 15.
pub fn facts() -> &'static [Fact] {
    &FACTS
}

const ENGAGEMENT: Variant = Variant {
    label: "engagement ranking",
    apply: |_| {},
};
const CHRONOLOGICAL: Variant = Variant {
    label: "chronological",
    apply: |c| c.design.chronological = true,
};
const INTEGRITY: Variant = Variant {
    label: "engagement ranking with integrity",
    apply: |c| crate::presets::integrity_ranking(&mut c.design),
};
/// The integrity preset with a deactivation experiment that sends every
/// regular user on the break, or none of them in its placebo.
fn integrity_deactivation(c: &mut Config, placebo: bool) {
    crate::presets::integrity_ranking(&mut c.design);
    c.run.deactivation = Some(Deactivation {
        share: 1.0,
        placebo,
        ..Deactivation::default()
    });
}
const INTEGRITY_DEACTIVATION: Variant = Variant {
    label: "engagement ranking with integrity, every regular user on a break",
    apply: |c| integrity_deactivation(c, false),
};
const INTEGRITY_PLACEBO: Variant = Variant {
    label: "engagement ranking with integrity, the same break as a placebo",
    apply: |c| integrity_deactivation(c, true),
};

/// A check that has to hold.
const fn check(label: &'static str, measure: Measure, judge: Judge) -> Check {
    Check {
        label,
        measure,
        judge,
        known_limit: false,
    }
}

const FACTS: [Fact; 6] = [
    Fact {
        id: "amplification",
        title: "Engagement ranking amplifies divisive content that users do not value",
        reference: "Milli et al. (2025)",
        variants: &[ENGAGEMENT, CHRONOLOGICAL],
        checks: &[
            check(
                "more outrage impressions than chronological",
                Measure::Window(MetricId::OutrageShare, 0.5, 1.0),
                Judge::Band {
                    effect: Effect::Difference { a: 0, b: 1 },
                    low: 0.15,
                    high: 0.6,
                },
            ),
            check(
                "lower reflective value per impression than chronological",
                Measure::Window(MetricId::ReflectivePerImpression, 0.5, 1.0),
                Judge::Band {
                    effect: Effect::Difference { a: 1, b: 0 },
                    low: 0.05,
                    high: 0.4,
                },
            ),
        ],
    },
    Fact {
        id: "outrage-learning",
        title: "Social reward teaches outrage",
        reference: "Brady et al. (2021)",
        variants: &[
            ENGAGEMENT,
            Variant {
                label: "no learning",
                apply: |c| c.world.learning_rate = 0.0,
            },
        ],
        checks: &[
            check(
                "Outrage share of posts rises under the engagement ranking",
                Measure::Rise(MetricId::PostShareOutrage),
                Judge::Band {
                    effect: Effect::Mean { a: 0 },
                    low: 0.01,
                    high: 0.15,
                },
            ),
            check(
                "it rises more than without learning",
                Measure::Rise(MetricId::PostShareOutrage),
                Judge::Band {
                    effect: Effect::Difference { a: 0, b: 1 },
                    low: 0.01,
                    high: 0.15,
                },
            ),
        ],
    },
    Fact {
        id: "chronological-feed",
        title: "A chronological feed cuts time spent, barely moves polarisation in three months, and shifts \
            exposure from incivility toward untrustworthy content",
        reference: "Guess et al. (2023)",
        variants: &[ENGAGEMENT, CHRONOLOGICAL, INTEGRITY],
        checks: &[
            check(
                "time spent under chronological / engagement ranking",
                Measure::Window(MetricId::TimeSpent, 0.25, 1.0),
                Judge::Band {
                    effect: Effect::Ratio { a: 1, b: 0 },
                    low: 0.4,
                    high: 0.9,
                },
            ),
            check(
                "relative difference in polarisation around day 90",
                Measure::Days(MetricId::PolarisationSociety, 80, 90),
                Judge::Close {
                    a: 1,
                    b: 0,
                    tolerance: 0.05,
                },
            ),
            // The field lowered incivility by tens of percent, not by a
            // factor (docs/model.md, section 15): the model's cut is deeper,
            // a known limit (section 16).
            Check {
                label: "toxic impressions, days 30 to 90, chronological / with integrity",
                measure: Measure::Days(MetricId::ToxicShare, 30, 90),
                judge: Judge::Band {
                    effect: Effect::Ratio { a: 1, b: 2 },
                    low: 0.5,
                    high: 0.9,
                },
                known_limit: true,
            },
            Check {
                label: "misinformation impressions, days 30 to 90, chronological / with integrity",
                measure: Measure::Days(MetricId::MisinformationShare, 30, 90),
                judge: Judge::Band {
                    effect: Effect::Ratio { a: 1, b: 2 },
                    low: 1.05,
                    high: 3.0,
                },
                known_limit: true,
            },
        ],
    },
    Fact {
        id: "forwarding-limits",
        title: "Forwarding limits slow viral spread",
        reference: "de Freitas Melo et al. (2019)",
        variants: &[
            ENGAGEMENT,
            Variant {
                label: "reshare depth cap 1",
                apply: |c| c.design.reshare_depth_cap = Some(1),
            },
        ],
        checks: &[check(
            "largest cascades with cap 1 / without cap",
            Measure::Window(MetricId::LargestCascade, 0.25, 1.0),
            Judge::Band {
                effect: Effect::Ratio { a: 1, b: 0 },
                low: 0.3,
                high: 0.85,
            },
        )],
    },
    Fact {
        id: "harassment-exit",
        title: "Harassment drives its targets away",
        reference: "Pew Research Center (2017)",
        variants: &[
            ENGAGEMENT,
            Variant {
                label: "four times the trolls",
                apply: |c| c.population.archetype_shares.troll = 0.12,
            },
        ],
        checks: &[check(
            "more Vulnerable users leave with more trolls",
            Measure::Window(MetricId::VulnerableLeftShare, 0.9, 1.0),
            Judge::Band {
                effect: Effect::Difference { a: 1, b: 0 },
                low: 0.03,
                high: 0.3,
            },
        )],
    },
    // A paired design only a simulation can run: the same people with and
    // without the break, on the same seed, so which people happen to be in
    // the break group no longer adds noise (docs/model.md, 13.2). The
    // integrity preset is the model's realistic ranked feed, as in fact 3
    // (docs/model.md, 14.1).
    Fact {
        id: "deactivation",
        title: "Leaving the platform raises wellbeing",
        reference: "Allcott, Gentzkow, Wittenbrink et al. (2025); Allcott et al. (2020)",
        variants: &[INTEGRITY_DEACTIVATION, INTEGRITY_PLACEBO],
        checks: &[check(
            "break group's wellbeing change, break minus placebo, in SDs, under integrity",
            Measure::Last(MetricId::BreakChange),
            Judge::Band {
                effect: Effect::Difference { a: 0, b: 1 },
                low: 0.02,
                high: 0.25,
            },
        )],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Histories of one day each, `[variant][seed]`, with the outrage share
    /// set to the given values.
    fn histories(values: &[&[f32]]) -> Vec<Vec<Vec<DailyMetrics>>> {
        values
            .iter()
            .map(|seeds| {
                seeds
                    .iter()
                    .map(|&v| {
                        let mut day = DailyMetrics::empty(0);
                        day.set(MetricId::OutrageShare, v);
                        vec![day]
                    })
                    .collect()
            })
            .collect()
    }

    fn check(judge: Judge) -> Check {
        super::check(
            "test",
            Measure::Window(MetricId::OutrageShare, 0.0, 1.0),
            judge,
        )
    }

    #[test]
    fn band_checks_have_both_bounds() {
        let h = histories(&[&[0.5, 0.6, 0.7], &[0.2, 0.2, 0.2]]);
        let band = |low, high| {
            check(Judge::Band {
                effect: Effect::Difference { a: 0, b: 1 },
                low,
                high,
            })
        };
        let inside = judge(&band(0.2, 0.6), &h);
        assert!((inside.effect - 0.4).abs() < 1e-6);
        assert!((inside.spread - 0.1).abs() < 1e-6);
        assert_eq!(inside.threshold, 0.2);
        assert_eq!(inside.bound, Bound::Between(0.2, 0.6));
        assert_eq!(inside.bound.text(), "0.2000 to 0.6000");
        assert!(inside.passed);
        assert!(!judge(&band(0.2, 0.35), &h).passed, "above the band");
        assert!(!judge(&band(0.45, 0.9), &h).passed, "below the band");
        let ratio = judge(
            &check(Judge::Band {
                effect: Effect::Ratio { a: 0, b: 1 },
                low: 2.5,
                high: 3.5,
            }),
            &h,
        );
        assert!((ratio.effect - 3.0).abs() < 1e-5 && ratio.passed);
    }

    #[test]
    fn verdicts_judge_the_interval_of_the_per_seed_effect() {
        let band = |low, high| {
            check(Judge::Band {
                effect: Effect::Difference { a: 0, b: 1 },
                low,
                high,
            })
        };
        // Differences 0.3, 0.4, 0.5: mean 0.4, sd 0.1, interval 0.4 +- 4.303
        // * 0.1 / sqrt(3), about 0.152 to 0.648.
        let h = histories(&[&[0.5, 0.6, 0.7], &[0.2, 0.2, 0.2]]);
        let r = judge(&band(0.1, 0.7), &h);
        assert_eq!(r.interval.n, 3);
        assert!((r.interval.mean - 0.4).abs() < 1e-6);
        assert!((r.interval.half_width() - 4.303 * 0.1 / 3f32.sqrt()).abs() < 1e-4);
        assert_eq!(r.verdict, Verdict::Pass);
        // The seed mean lies inside, but the interval straddles a bound.
        let r = judge(&band(0.2, 0.6), &h);
        assert!(r.passed);
        assert_eq!(r.verdict, Verdict::Undecided);
        // Wholly above the band, wholly below it.
        assert_eq!(judge(&band(0.0, 0.1), &h).verdict, Verdict::Fail);
        assert_eq!(judge(&band(0.7, 0.9), &h).verdict, Verdict::Fail);
        // One seed has no interval.
        let one = histories(&[&[0.5], &[0.2]]);
        assert_eq!(judge(&band(0.1, 0.7), &one).verdict, Verdict::Undecided);
        // A closeness check judges the signed relative differences, here
        // -0.02, 0, 0.02, against minus to plus the tolerance.
        let h = histories(&[&[0.49, 0.5, 0.51], &[0.5, 0.5, 0.5]]);
        let close = |tolerance| {
            check(Judge::Close {
                a: 0,
                b: 1,
                tolerance,
            })
        };
        let r = judge(&close(0.1), &h);
        assert!(r.interval.low < 0.0 && r.interval.high > 0.0);
        assert_eq!(r.verdict, Verdict::Pass);
        let h = histories(&[&[0.6, 0.62, 0.64], &[0.5, 0.5, 0.5]]);
        assert_eq!(judge(&close(0.1), &h).verdict, Verdict::Fail);
    }

    #[test]
    fn one_sided_bounds_have_verdicts() {
        use Verdict::{Fail, Pass, Undecided};
        assert_eq!(Bound::Above(0.1).verdict(0.2, 0.3), Pass);
        assert_eq!(Bound::Above(0.1).verdict(0.0, 0.1), Fail);
        assert_eq!(Bound::Above(0.1).verdict(0.05, 0.3), Undecided);
        assert_eq!(Bound::Below(0.1).verdict(0.0, 0.05), Pass);
        assert_eq!(Bound::Below(0.1).verdict(0.1, 0.2), Fail);
        assert_eq!(Bound::Between(0.0, 1.0).verdict(f32::NAN, 0.5), Undecided);
    }

    #[test]
    fn every_kind_of_check_reports_a_spread() {
        let h = histories(&[&[0.5, 0.6, 0.7], &[0.25, 0.3, 0.35]]);
        // Per-seed ratios are all 2: no spread, ratio of means 2.
        let ratio = judge(
            &check(Judge::RatioBelow {
                a: 0,
                b: 1,
                ratio: 3.0,
            }),
            &h,
        );
        assert!((ratio.effect - 2.0).abs() < 1e-6 && ratio.spread.abs() < 1e-6);
        assert!(ratio.passed && ratio.bound == Bound::Below(3.0));
        let h = histories(&[&[0.5, 0.6, 0.7], &[0.5, 0.5, 0.5]]);
        // Relative differences 0, 0.2, 0.4: spread 0.2, effect 0.2.
        let close = judge(
            &check(Judge::Close {
                a: 0,
                b: 1,
                tolerance: 0.1,
            }),
            &h,
        );
        assert!((close.effect - 0.2).abs() < 1e-6, "{close:?}");
        assert!((close.spread - 0.2).abs() < 1e-6, "{close:?}");
        assert!(!close.passed);
    }
}
