//! Scenarios: a starting configuration, a duration and goals that compare the
//! player's run with a baseline run on the same seeds (docs/model.md, 14.2).
//!
//! [`Scenario::judge`] gives the verdict: each goal is judged on its per-seed
//! margins, the player's run against the baseline's on the same seed, with a
//! 95% confidence interval; [`Scenario::evaluate`] compares seed means only.

use serde::Serialize;

use crate::config::{Config, DesignLevers};
use crate::config_path::FieldValue;
use crate::error::ConfigError;
use crate::metrics::{MetricId, Summary};
use crate::stats::MeanInterval;

/// How a goal compares the player's value with the baseline's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Comparison {
    /// At least this multiple of the baseline.
    AtLeastTimesBaseline(f32),
    /// At most this multiple of the baseline.
    AtMostTimesBaseline(f32),
    /// Strictly more than the baseline.
    MoreThanBaseline,
    /// Strictly less than the baseline.
    LessThanBaseline,
    /// At least the baseline plus this amount.
    AtLeastBaselinePlus(f32),
    /// Strictly below this absolute value.
    Below(f32),
}

impl Comparison {
    /// The threshold the player's value is checked against.
    pub fn threshold(self, baseline: f32) -> f32 {
        match self {
            Self::AtLeastTimesBaseline(f) | Self::AtMostTimesBaseline(f) => f * baseline,
            Self::MoreThanBaseline | Self::LessThanBaseline => baseline,
            Self::AtLeastBaselinePlus(d) => baseline + d,
            Self::Below(v) => v,
        }
    }

    /// Whether `player` meets the goal against `baseline`.
    pub fn passes(self, player: f32, baseline: f32) -> bool {
        let t = self.threshold(baseline);
        match self {
            Self::AtLeastTimesBaseline(_) | Self::AtLeastBaselinePlus(_) => player >= t,
            Self::AtMostTimesBaseline(_) => player <= t,
            Self::MoreThanBaseline => player > t,
            Self::LessThanBaseline | Self::Below(_) => player < t,
        }
    }

    /// How far `player` is on the passing side of the threshold: positive
    /// when it meets the goal by that much, negative when it misses.
    pub fn margin(self, player: f32, baseline: f32) -> f32 {
        let t = self.threshold(baseline);
        match self {
            Self::AtLeastTimesBaseline(_)
            | Self::AtLeastBaselinePlus(_)
            | Self::MoreThanBaseline => player - t,
            Self::AtMostTimesBaseline(_) | Self::LessThanBaseline | Self::Below(_) => t - player,
        }
    }

    /// Whether the goal is strict (more than, less than, below), so that a
    /// margin of exactly zero misses it.
    pub fn is_strict(self) -> bool {
        matches!(
            self,
            Self::MoreThanBaseline | Self::LessThanBaseline | Self::Below(_)
        )
    }

    /// The verdict on a 95% interval `[low, high]` of the margin: pass when
    /// the interval lies wholly on the passing side, fail when it lies wholly
    /// on the failing side, undecided otherwise (and without an interval).
    /// Zero belongs to the failing side of a strict goal and to the passing
    /// side of the others.
    pub fn verdict(self, low: f32, high: f32) -> Verdict {
        if low.is_nan() || high.is_nan() {
            return Verdict::Undecided;
        }
        let (pass, fail) = if self.is_strict() {
            (low > 0.0, high <= 0.0)
        } else {
            (low >= 0.0, high < 0.0)
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

/// The verdict on a goal or a scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// Met, beyond the seeds' noise.
    Pass,
    /// Missed, beyond the seeds' noise.
    Fail,
    /// The seeds cannot tell.
    Undecided,
}

impl Verdict {
    /// `PASS`, `FAIL` or `UNDECIDED`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Undecided => "UNDECIDED",
        }
    }

    /// A scenario's verdict from its goals': pass when every goal passes,
    /// fail when any goal fails, undecided otherwise.
    pub fn all(verdicts: impl Iterator<Item = Verdict>) -> Verdict {
        let mut result = Verdict::Pass;
        for v in verdicts {
            match v {
                Verdict::Fail => return Verdict::Fail,
                Verdict::Undecided => result = Verdict::Undecided,
                Verdict::Pass => {}
            }
        }
        result
    }
}

/// The verdict on one goal over paired seeds.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GoalVerdict {
    /// The goal in plain language.
    pub description: &'static str,
    /// The metric.
    pub metric: MetricId,
    /// How it is compared.
    pub comparison: Comparison,
    /// The player's mean over the seeds.
    pub player: f32,
    /// The baseline's mean over the seeds.
    pub baseline: f32,
    /// The threshold at the baseline's mean.
    pub threshold: f32,
    /// The mean per-seed margin ([`Comparison::margin`]) with its 95%
    /// interval.
    pub margin: MeanInterval,
    /// Pass, fail or undecided.
    pub verdict: Verdict,
}

/// The verdict on a scenario over paired seeds.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScenarioVerdict {
    /// Every goal's verdict.
    pub goals: Vec<GoalVerdict>,
    /// Pass only when every goal passes; fail when any goal fails.
    pub verdict: Verdict,
}

/// One goal of a scenario.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Goal {
    /// The metric, averaged over the summary window.
    pub metric: MetricId,
    /// How it is compared.
    pub comparison: Comparison,
    /// The goal in plain language.
    pub description: &'static str,
}

/// The verdict on one goal.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GoalResult {
    /// The goal in plain language.
    pub description: &'static str,
    /// The metric.
    pub metric: MetricId,
    /// The player's value.
    pub player: f32,
    /// The baseline's value.
    pub baseline: f32,
    /// The value the player had to reach (or stay under).
    pub threshold: f32,
    /// Whether the goal is met.
    pub passed: bool,
}

/// A scenario.
#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    /// Stable kebab-case id.
    pub id: &'static str,
    /// Title.
    pub title: &'static str,
    /// Plain-language briefing for the player.
    pub briefing: &'static str,
    /// The goals.
    pub goals: &'static [Goal],
    setup: fn(&mut Config),
    known_solution: &'static [(&'static str, &'static str)],
}

impl Scenario {
    /// The scenario's starting configuration, which is also the baseline.
    pub fn config(&self) -> Config {
        let mut config = Config::default();
        (self.setup)(&mut config);
        config
    }

    /// The scenario with the player's design: run, population and world stay
    /// the scenario's.
    pub fn player_config(&self, design: &DesignLevers) -> Config {
        Config {
            design: design.clone(),
            ..self.config()
        }
    }

    /// Changes to the starting design that are known to meet every goal.
    pub fn known_solution_changes(&self) -> &'static [(&'static str, &'static str)] {
        self.known_solution
    }

    /// The design known to meet every goal.
    pub fn known_solution(&self) -> Result<DesignLevers, ConfigError> {
        let mut config = self.config();
        for (path, value) in self.known_solution {
            config.set_path(path, FieldValue::parse(value))?;
        }
        Ok(config.design)
    }

    /// Judges the player's runs against the baseline's, paired by seed:
    /// `player[i]` and `baseline[i]` ran on the same seed. Each goal's
    /// per-seed margins get a 95% t-interval and a [`Verdict`]
    /// ([`Comparison::verdict`]); seeds where either side lacks the metric
    /// are skipped.
    pub fn judge(&self, player: &[Summary], baseline: &[Summary]) -> ScenarioVerdict {
        let goals: Vec<GoalVerdict> = self
            .goals
            .iter()
            .map(|g| {
                let values = |runs: &[Summary]| -> Vec<f32> {
                    runs.iter().map(|s| s.get(g.metric)).collect()
                };
                let (p, b) = (values(player), values(baseline));
                let margin =
                    MeanInterval::of(p.iter().zip(&b).map(|(&p, &b)| g.comparison.margin(p, b)));
                let mean = |v: &[f32]| crate::stats::Stats::of(v.iter().copied()).mean;
                let (pm, bm) = (mean(&p), mean(&b));
                GoalVerdict {
                    description: g.description,
                    metric: g.metric,
                    comparison: g.comparison,
                    player: pm,
                    baseline: bm,
                    threshold: g.comparison.threshold(bm),
                    margin,
                    verdict: g.comparison.verdict(margin.low, margin.high),
                }
            })
            .collect();
        let verdict = Verdict::all(goals.iter().map(|g| g.verdict));
        ScenarioVerdict { goals, verdict }
    }

    /// Judges the player's summary against the baseline's, goal by goal, on
    /// seed means alone; [`Scenario::judge`] gives the verdict with intervals.
    pub fn evaluate(&self, player: &Summary, baseline: &Summary) -> Vec<GoalResult> {
        self.goals
            .iter()
            .map(|g| {
                let (p, b) = (player.get(g.metric), baseline.get(g.metric));
                GoalResult {
                    description: g.description,
                    metric: g.metric,
                    player: p,
                    baseline: b,
                    threshold: g.comparison.threshold(b),
                    passed: g.comparison.passes(p, b),
                }
            })
            .collect()
    }
}

/// Every scenario, in display order.
pub fn scenarios() -> &'static [Scenario] {
    &SCENARIOS
}

/// A scenario by id.
pub fn scenario(id: &str) -> Result<&'static Scenario, ConfigError> {
    SCENARIOS
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ConfigError::UnknownScenario(id.to_string()))
}

const SCENARIOS: [Scenario; 5] = [
    Scenario {
        id: "engagement-trap",
        title: "The engagement trap",
        briefing: "Your platform is growing fast, and so is the bile. Keep at least 85% of today's engagement \
            and halve the share of toxic posts people see, over a full year, without more than doubling the \
            harmless posts your filters take down or bury.",
        goals: &[
            Goal {
                metric: MetricId::EngagementIndex,
                comparison: Comparison::AtLeastTimesBaseline(0.85),
                description: "Keep at least 85% of the baseline's engagement index",
            },
            Goal {
                metric: MetricId::ToxicShare,
                comparison: Comparison::AtMostTimesBaseline(0.5),
                description: "At most half the baseline's toxic impression share",
            },
            Goal {
                metric: MetricId::FalsePositives,
                comparison: Comparison::AtMostTimesBaseline(2.0),
                description: "At most twice the baseline's wrongful removals and demotions",
            },
        ],
        setup: |c| c.run.days = 365,
        known_solution: &[
            ("design.w_reply", "0"),
            ("design.review_capacity", "10"),
            ("design.report_trust", "1"),
        ],
    },
    Scenario {
        id: "toxicity-spiral",
        title: "Toxicity spiral",
        briefing: "Trolls have found your platform and your moderation team is a skeleton crew. Cut harassment \
            to less than half of what it would be, without losing more than a fifth of your daily users, \
            within six months.",
        goals: &[
            Goal {
                metric: MetricId::HarassmentPer1000,
                comparison: Comparison::AtMostTimesBaseline(0.5),
                description: "Harassment per 1,000 users at most half the baseline's",
            },
            Goal {
                metric: MetricId::HumanDailyActives,
                comparison: Comparison::AtLeastTimesBaseline(0.8),
                description: "Keep at least 80% of the baseline's human daily actives",
            },
        ],
        setup: |c| {
            c.run.days = 180;
            c.population.archetype_shares.troll = 0.08;
            c.design.review_capacity = 0.3;
        },
        known_solution: &[
            ("design.review_capacity", "10"),
            ("design.account_cost", "1"),
            ("design.classifier_threshold", "0.3"),
        ],
    },
    Scenario {
        id: "creator-living",
        title: "A living for creators",
        briefing: "You pay 30% of revenue to the people who post. Make more creators earn a living than the \
            engagement-paid fund does, while keeping outrage, clickbait and attacks below a fifth of all payouts. \
            Note that fans here tip by what posts are truly worth to them, more cleanly than any real tip jar.",
        goals: &[
            Goal {
                metric: MetricId::CreatorsLiving,
                comparison: Comparison::MoreThanBaseline,
                description: "More Creators earning a living than the baseline",
            },
            Goal {
                metric: MetricId::HarmfulPayoutShare,
                comparison: Comparison::Below(0.2),
                description: "Less than 20% of payouts to Outrage, Clickbait and Attack posts",
            },
        ],
        setup: |c| {
            c.run.days = 365;
            c.design.payout_share = 0.3;
        },
        known_solution: &[("design.payout_scheme", "fan-funded")],
    },
    Scenario {
        id: "misinformation-surge",
        title: "Misinformation surge",
        briefing: "Bot farms flood the platform with clickbait, and people are more credulous than usual. \
            Protect the people who are not even on your platform: raise non-users' informedness by at least \
            0.03 above where it would be, within six months.",
        goals: &[Goal {
            metric: MetricId::InformednessNonUsers,
            comparison: Comparison::AtLeastBaselinePlus(0.03),
            description: "Non-users' informedness at least the baseline's plus 0.03",
        }],
        setup: |c| {
            c.run.days = 180;
            c.world.credulity_scale = 1.6;
            c.population.bot_clickbait_share = 0.9;
        },
        known_solution: &[
            ("design.w_like", "0.5"),
            ("design.w_reply", "0.3"),
            ("design.w_share", "0.5"),
            ("design.w_quality", "1"),
            ("design.w_bridging", "1"),
            ("design.w_see_less", "1"),
            ("design.community_notes", "true"),
            ("design.account_cost", "1"),
        ],
    },
    Scenario {
        id: "public-square",
        title: "Public square",
        briefing: "Your platform has become where the country argues. Make society at least a tenth less \
            polarised than it would be under the engagement ranking, while keeping at least 80% of its \
            engagement, over a full year.",
        goals: &[
            Goal {
                metric: MetricId::PolarisationSociety,
                comparison: Comparison::AtMostTimesBaseline(0.9),
                description: "Society-wide affective polarisation at most 90% of the baseline's",
            },
            Goal {
                metric: MetricId::EngagementIndex,
                comparison: Comparison::AtLeastTimesBaseline(0.8),
                description: "Keep at least 80% of the baseline's engagement index",
            },
        ],
        setup: |c| c.run.days = 365,
        known_solution: &[("design.w_reply", "0"), ("design.w_bridging", "0.6")],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenarios_and_solutions_validate() {
        for s in scenarios() {
            assert!(s.config().validate().is_ok(), "{}", s.id);
            assert!(s.known_solution().is_ok(), "{}", s.id);
        }
        assert!(scenario("nope").is_err());
    }

    #[test]
    fn comparisons_judge_correctly() {
        assert!(Comparison::AtLeastTimesBaseline(0.85).passes(0.9, 1.0));
        assert!(!Comparison::AtMostTimesBaseline(0.5).passes(0.6, 1.0));
        assert!(Comparison::AtLeastBaselinePlus(0.05).passes(0.66, 0.6));
        assert!(!Comparison::LessThanBaseline.passes(1.0, 1.0));
    }

    #[test]
    fn margins_are_positive_on_the_passing_side() {
        assert!((Comparison::AtLeastTimesBaseline(0.85).margin(0.9, 1.0) - 0.05).abs() < 1e-6);
        assert!((Comparison::AtMostTimesBaseline(0.5).margin(0.6, 1.0) + 0.1).abs() < 1e-6);
        assert!((Comparison::AtLeastBaselinePlus(0.03).margin(0.64, 0.6) - 0.01).abs() < 1e-6);
        assert!((Comparison::Below(0.2).margin(0.05, 9.0) - 0.15).abs() < 1e-6);
        assert_eq!(Comparison::MoreThanBaseline.margin(2.0, 1.5), 0.5);
    }

    #[test]
    fn verdicts_follow_the_interval() {
        let at_least = Comparison::AtLeastTimesBaseline(0.85);
        assert_eq!(at_least.verdict(0.01, 0.05), Verdict::Pass);
        assert_eq!(at_least.verdict(-0.05, -0.01), Verdict::Fail);
        assert_eq!(at_least.verdict(-0.01, 0.05), Verdict::Undecided);
        assert_eq!(at_least.verdict(f32::NAN, f32::NAN), Verdict::Undecided);
        // Zero passes a goal that allows equality and fails a strict one.
        assert_eq!(at_least.verdict(0.0, 0.0), Verdict::Pass);
        assert_eq!(
            Comparison::MoreThanBaseline.verdict(0.0, 0.0),
            Verdict::Fail
        );
        assert_eq!(
            Verdict::all([Verdict::Pass, Verdict::Undecided].into_iter()),
            Verdict::Undecided
        );
        assert_eq!(
            Verdict::all([Verdict::Undecided, Verdict::Fail].into_iter()),
            Verdict::Fail
        );
        assert_eq!(Verdict::all([Verdict::Pass].into_iter()), Verdict::Pass);
    }

    #[test]
    fn judging_pairs_the_seeds() {
        use crate::metrics::DailyMetrics;
        let summary = |engagement: f32, toxic: f32| {
            let mut day = DailyMetrics::empty(0);
            day.set(MetricId::EngagementIndex, engagement);
            day.set(MetricId::ToxicShare, toxic);
            // Wrongful sanctions as many as the baseline's: within the
            // trap's third goal.
            day.set(MetricId::FalsePositives, 3.0);
            Summary::from_history(&[day], 30)
        };
        let sc = scenario("engagement-trap").expect("known scenario");
        // Engagement varies a lot between seeds but the player keeps 95% of
        // each seed's: paired, the margin is clearly positive.
        let baseline = [summary(0.2, 0.16), summary(0.3, 0.14), summary(0.25, 0.15)];
        let player = [
            summary(0.19, 0.06),
            summary(0.285, 0.05),
            summary(0.2375, 0.055),
        ];
        let v = sc.judge(&player, &baseline);
        assert_eq!(v.goals[0].verdict, Verdict::Pass, "{:?}", v.goals[0]);
        assert_eq!(v.goals[0].margin.n, 3);
        assert_eq!(v.goals[1].verdict, Verdict::Pass, "{:?}", v.goals[1]);
        assert_eq!(v.verdict, Verdict::Pass);
        // Against itself the baseline fails the toxicity goal.
        let own = sc.judge(&baseline, &baseline);
        assert_eq!(own.goals[1].verdict, Verdict::Fail);
        assert_eq!(own.verdict, Verdict::Fail);
        // One toxic seed on the wrong side leaves the toxicity goal undecided.
        let mixed = [
            summary(0.19, 0.06),
            summary(0.285, 0.09),
            summary(0.2375, 0.07),
        ];
        assert_eq!(
            sc.judge(&mixed, &baseline).goals[1].verdict,
            Verdict::Undecided
        );
    }
}
