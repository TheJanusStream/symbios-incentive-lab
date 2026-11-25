//! The stylized facts the model is checked against (docs/model.md, section
//! 15), as data: each fact names its variants (paired runs on the same seeds)
//! and its checks. Callers run the simulations however they like (the tests
//! sequentially, the command-line runner in parallel) and hand the histories
//! to [`Fact::evaluate`].

use serde::Serialize;

use crate::config::Config;
use crate::error::ConfigError;
use crate::metrics::{DailyMetrics, MetricId, nan_mean};
use crate::simulation::Simulation;

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
        }
    }
}

/// How a check judges the per-seed measurements of two variants.
#[derive(Clone, Copy, Debug)]
enum Judge {
    /// Paired mean of `a - b` above the margin.
    Exceeds { a: usize, b: usize, margin: f32 },
    /// Mean of variant `a` above the margin.
    Above { a: usize, margin: f32 },
    /// Mean of `a` divided by mean of `b` below the ratio.
    RatioBelow { a: usize, b: usize, ratio: f32 },
    /// Absolute relative difference of the means of `a` and `b` below the tolerance.
    Close { a: usize, b: usize, tolerance: f32 },
}

/// One check of a fact.
#[derive(Clone, Copy, Debug)]
pub struct Check {
    /// What the check claims.
    pub label: &'static str,
    measure: Measure,
    judge: Judge,
}

/// The verdict on one check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckResult {
    /// What the check claims.
    pub label: &'static str,
    /// The effect: a paired mean difference, a mean, a ratio or a relative
    /// difference, depending on the check.
    pub effect: f32,
    /// Standard deviation of the per-seed effect (0 for ratios).
    pub spread: f32,
    /// The value the effect had to beat or stay under.
    pub threshold: f32,
    /// Whether the check holds.
    pub passed: bool,
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
        self.variants
            .iter()
            .map(|v| {
                let mut config = Config::default();
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

fn judge(check: &Check, histories: &[Vec<Vec<DailyMetrics>>]) -> CheckResult {
    let values = |v: usize| -> Vec<f32> {
        histories
            .get(v)
            .map(|runs| runs.iter().map(|h| check.measure.of(h)).collect())
            .unwrap_or_default()
    };
    let (effect, spread, threshold, passed) = match check.judge {
        Judge::Exceeds { a, b, margin } => {
            let diffs: Vec<f32> = values(a)
                .iter()
                .zip(values(b))
                .map(|(x, y)| x - y)
                .collect();
            let (m, sd) = mean_sd(&diffs);
            (m, sd, margin, m > margin)
        }
        Judge::Above { a, margin } => {
            let (m, sd) = mean_sd(&values(a));
            (m, sd, margin, m > margin)
        }
        Judge::RatioBelow { a, b, ratio } => {
            let r = mean_sd(&values(a)).0 / mean_sd(&values(b)).0;
            (r, 0.0, ratio, r < ratio)
        }
        Judge::Close { a, b, tolerance } => {
            let (ma, mb) = (mean_sd(&values(a)).0, mean_sd(&values(b)).0);
            let rel = ((ma - mb) / mb).abs();
            (rel, 0.0, tolerance, rel < tolerance)
        }
    };
    CheckResult {
        label: check.label,
        effect,
        spread,
        threshold,
        passed,
    }
}

/// Every stylized fact, in the order of docs/model.md, section 15.
pub fn facts() -> &'static [Fact] {
    &FACTS
}

const ENGAGEMENT: Variant = Variant {
    label: "engagement maximiser",
    apply: |_| {},
};
const CHRONOLOGICAL: Variant = Variant {
    label: "chronological",
    apply: |c| c.design.chronological = true,
};

const FACTS: [Fact; 5] = [
    Fact {
        id: "amplification",
        title: "Engagement ranking amplifies divisive content that users do not value",
        reference: "Milli et al. (2025)",
        variants: &[ENGAGEMENT, CHRONOLOGICAL],
        checks: &[
            Check {
                label: "more outrage impressions than chronological",
                measure: Measure::Window(MetricId::OutrageShare, 0.5, 1.0),
                judge: Judge::Exceeds {
                    a: 0,
                    b: 1,
                    margin: 0.02,
                },
            },
            Check {
                label: "lower reflective value per impression than chronological",
                measure: Measure::Window(MetricId::ReflectivePerImpression, 0.5, 1.0),
                judge: Judge::Exceeds {
                    a: 1,
                    b: 0,
                    margin: 0.02,
                },
            },
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
            Check {
                label: "Outrage share of posts rises under the engagement maximiser",
                measure: Measure::Rise(MetricId::PostShareOutrage),
                judge: Judge::Above { a: 0, margin: 0.01 },
            },
            Check {
                label: "it rises more than without learning",
                measure: Measure::Rise(MetricId::PostShareOutrage),
                judge: Judge::Exceeds {
                    a: 0,
                    b: 1,
                    margin: 0.01,
                },
            },
        ],
    },
    Fact {
        id: "chronological-feed",
        title: "A chronological feed cuts time spent and barely moves polarisation in three months",
        reference: "Guess et al. (2023)",
        variants: &[ENGAGEMENT, CHRONOLOGICAL],
        checks: &[
            Check {
                label: "time spent under chronological / engagement maximiser",
                measure: Measure::Window(MetricId::TimeSpent, 0.25, 1.0),
                judge: Judge::RatioBelow {
                    a: 1,
                    b: 0,
                    ratio: 0.9,
                },
            },
            Check {
                label: "relative difference in polarisation around day 90",
                measure: Measure::Days(MetricId::PolarisationSociety, 80, 90),
                judge: Judge::Close {
                    a: 1,
                    b: 0,
                    tolerance: 0.05,
                },
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
        checks: &[Check {
            label: "largest cascades with cap 1 / without cap",
            measure: Measure::Window(MetricId::LargestCascade, 0.25, 1.0),
            judge: Judge::RatioBelow {
                a: 1,
                b: 0,
                ratio: 0.85,
            },
        }],
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
        checks: &[Check {
            label: "more Vulnerable users leave with more trolls",
            measure: Measure::Window(MetricId::VulnerableLeftShare, 0.9, 1.0),
            judge: Judge::Exceeds {
                a: 1,
                b: 0,
                margin: 0.02,
            },
        }],
    },
];
