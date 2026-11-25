//! Scenarios: a starting configuration, a duration and goals that compare the
//! player's run with a baseline run on the same seeds (docs/model.md, 14.2).

use serde::Serialize;

use crate::config::{Config, DesignLevers};
use crate::config_path::FieldValue;
use crate::error::ConfigError;
use crate::metrics::{MetricId, Summary};

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

    /// Judges the player's summary against the baseline's, goal by goal.
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
            and halve the share of toxic posts people see, over a full year.",
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
        ],
        setup: |c| c.run.days = 365,
        known_solution: &[("design.w_reply", "0"), ("design.w_see_less", "5")],
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
                description: "Harassment per 1,000 users below half the baseline's",
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
            engagement-paid fund does, while keeping outrage, clickbait and attacks below a fifth of all payouts.",
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
            polarised than it would be under the engagement maximiser, while keeping at least 80% of its \
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
}
