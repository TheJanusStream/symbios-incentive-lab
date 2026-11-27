//! The run configuration: run settings, population, design levers and world
//! assumptions (docs/model.md, sections 10 and 11).

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::params;

mod assumptions;
mod design;

pub(crate) use assumptions::REFERENCE_SUBSCRIPTION_PRICE;
pub use assumptions::{BotStrategy, WorldAssumptions};
pub use design::{AltFeed, DesignLevers, ModerationAction, PayoutScheme, Sanctions};

/// A complete run configuration. The default is the Engagement ranking on
/// the default world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Seed, duration, population size and other settings of the run itself.
    pub run: RunSettings,
    /// Who lives in the simulated society.
    pub population: PopulationSettings,
    /// What the platform chooses: the player's levers.
    pub design: DesignLevers,
    /// How people are assumed to behave: not design choices.
    pub world: WorldAssumptions,
}

/// Settings of the run itself (docs/model.md, section 11, "Run settings").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunSettings {
    /// Seed of every random stream.
    pub seed: u64,
    /// Number of simulated days.
    pub days: u32,
    /// Number of individuals, users and non-users.
    pub population_size: u32,
    /// Share of individuals who are users on day 0.
    pub initial_user_share: f32,
    /// Days a post stays a feed candidate.
    pub post_lifetime_days: u32,
    /// Most candidates a feed considers.
    pub candidate_cap: u32,
    /// Offline ties per individual.
    pub offline_ties: u32,
    /// Accounts an initial user follows.
    pub initial_follows: u32,
    /// Share of the last days a run summary averages (at least 30 days).
    pub summary_window: f32,
    /// Individuals the inspector tracks from day 0, spread over the archetypes.
    pub tracked_agents: u32,
    /// A deactivation experiment at the end of the run, a researcher's
    /// intervention (docs/model.md, 13.2); `None`, the default, runs none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivation: Option<Deactivation>,
    /// A notifications-off experiment at the end of the run, a researcher's
    /// intervention (docs/model.md, 13.2); `None`, the default, runs none.
    /// At most one experiment runs at a time, so it cannot be set together
    /// with `deactivation`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications_off: Option<NotificationsOff>,
    /// A per-person feed experiment at the end of the run, a researcher's
    /// intervention (docs/model.md, 13.2); `None`, the default, runs none.
    /// At most one experiment runs at a time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_experiment: Option<FeedExperiment>,
}

/// Days of a deactivation experiment's break by default.
pub const DEACTIVATION_DAYS: u32 = 35;
/// Share of the eligible users a deactivation experiment sends on the break
/// by default.
pub const DEACTIVATION_SHARE: f32 = 0.1;

/// A deactivation experiment (docs/model.md, 13.2, and fact 6 of section
/// 15): over `days` days that end `ends_before_end` days before the end of
/// the run (by default with it), a random `share` of the regular users stays
/// off the platform, as the paid participants of a field experiment do. Its effect within one run is the truth metric
/// `deactivation_effect`; the truth metric `break_change` is the break
/// group's own change, compared across a real run and its placebo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Deactivation {
    /// Days the break lasts. At least 1, and the break with the days after
    /// it must leave at least one day of the run before it.
    pub days: u32,
    /// Chance that each eligible user joins the break group, in `(0, 1]`.
    pub share: f32,
    /// A placebo run: the participants are found and the break group drawn
    /// exactly as in a real run, but nobody goes on the break. A real run
    /// and its placebo on the same seed are identical up to the first day
    /// and assign the same people, so the paired difference of their
    /// `break_change` is the break's effect on those people (docs/model.md,
    /// 13.2). Left out of JSON when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub placebo: bool,
    /// Days between the break's end and the run's end, 0 by default: the
    /// break covers the days `[end - ends_before_end - days, end -
    /// ends_before_end)`. After it the break group uses the platform again
    /// under the usual rules, and the truth metric `post_break_dwell` reads
    /// their time on it. Left out of JSON when 0.
    #[serde(skip_serializing_if = "is_zero")]
    pub ends_before_end: u32,
}

/// Whether a count is 0, so that serde leaves it out.
fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl Default for Deactivation {
    fn default() -> Self {
        Self {
            days: DEACTIVATION_DAYS,
            share: DEACTIVATION_SHARE,
            placebo: false,
            ends_before_end: 0,
        }
    }
}

impl Deactivation {
    /// The first day of the break in a run of `run_days` days.
    pub fn first_day(&self, run_days: u32) -> u32 {
        run_days.saturating_sub(self.days.saturating_add(self.ends_before_end))
    }

    /// The first day after the break in a run of `run_days` days: the run's
    /// length when the break ends with the run.
    pub fn end_day(&self, run_days: u32) -> u32 {
        run_days.saturating_sub(self.ends_before_end)
    }
}

/// Days of a notifications-off experiment by default, a week as in the
/// field (docs/model.md, 13.2).
pub const NOTIFICATIONS_OFF_DAYS: u32 = 7;
/// Share of the eligible users a notifications-off experiment switches
/// notifications off for by default: all of them, the paired design of fact
/// 7, which compares a run with its placebo rather than two groups.
pub const NOTIFICATIONS_OFF_SHARE: f32 = 1.0;
/// Days before a notifications-off experiment over which each participant's
/// baseline time and sessions are averaged.
pub const NOTIFICATIONS_OFF_BASELINE_DAYS: u32 = 7;

/// A notifications-off experiment (docs/model.md, 13.2, and fact 7 of
/// section 15): over the last `days` days of the run, a random `share` of
/// the regular users has the platform's notifications switched off, as the
/// participants of a field experiment switch them off in their phone's
/// settings. For them notification intensity is 0: no notification-driven
/// sessions, and none of its other terms. Its measures are the truth metrics
/// `notifications_off_time`, `notifications_off_sessions` and
/// `notifications_off_wellbeing`, compared across a real run and its
/// placebo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotificationsOff {
    /// Days the notifications stay off; the experiment ends with the run,
    /// so it starts this many days before the end. At least 1, and the run
    /// must have the 7 baseline days before it.
    pub days: u32,
    /// Chance that each eligible user has notifications switched off, in
    /// `(0, 1]`.
    pub share: f32,
    /// A placebo run: the participants are found and the treated group
    /// drawn exactly as in a real run, but nobody's notifications change.
    /// Left out of JSON when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub placebo: bool,
}

impl Default for NotificationsOff {
    fn default() -> Self {
        Self {
            days: NOTIFICATIONS_OFF_DAYS,
            share: NOTIFICATIONS_OFF_SHARE,
            placebo: false,
        }
    }
}

impl NotificationsOff {
    /// The first day with notifications off in a run of `run_days` days.
    pub fn first_day(&self, run_days: u32) -> u32 {
        run_days.saturating_sub(self.days)
    }
}

/// Days of a feed experiment by default, the 90 days of the field studies
/// (docs/model.md, 13.2).
pub const FEED_EXPERIMENT_DAYS: u32 = 90;
/// Share of the eligible users a feed experiment treats by default.
pub const FEED_EXPERIMENT_SHARE: f32 = 0.2;

/// How a feed experiment changes the treated viewers' feeds (docs/model.md,
/// 13.2). Every kind acts on the candidates or the ranking of the treated
/// only; a placebo run changes nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum FeedExperimentKind {
    /// Drops candidates whose toxicity classifier score (docs/model.md, 6.7),
    /// an observable score, exceeds `threshold`, as the field study hid posts
    /// above a classifier's score.
    HideFlagged {
        /// Classifier score above which a post is hidden, in `[0, 1]`.
        threshold: f32,
    },
    /// Drops reshares from the candidates, keeping the original posts, in
    /// network and recommended.
    #[default]
    NoReshares,
    /// Subtracts `penalty` from the ranking score of posts by the viewer's
    /// own group, so fewer of them reach the top of the feed. A
    /// chronological feed has no score, so it is unchanged.
    FewerSameGroup {
        /// The score penalty, in `[0, 10]`.
        penalty: f32,
    },
    /// Gives the treated viewers the chronological feed of their in-network
    /// posts (docs/model.md, 6.2) instead of the feed the design gives them:
    /// a holdback from ranking on the follow graph the ranked feed built.
    Chronological,
}

/// A per-person feed experiment (docs/model.md, 13.2): over the last `days`
/// days of the run, a random `share` of the regular users gets a changed
/// feed, as the field studies' extensions and holdbacks gave their
/// participants. Its measures are the truth metrics `feed_experiment_*`,
/// compared across a real run and its placebo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedExperiment {
    /// What changes in the treated viewers' feeds.
    pub kind: FeedExperimentKind,
    /// Days the experiment lasts; it ends with the run. At least 1 and less
    /// than the run's days.
    pub days: u32,
    /// Chance that each eligible user is treated, in `(0, 1]`.
    pub share: f32,
    /// A placebo run: the participants are found and the treated group
    /// drawn exactly as in a real run, but nobody's feed changes. Left out
    /// of JSON when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub placebo: bool,
}

impl Default for FeedExperiment {
    fn default() -> Self {
        Self {
            kind: FeedExperimentKind::default(),
            days: FEED_EXPERIMENT_DAYS,
            share: FEED_EXPERIMENT_SHARE,
            placebo: false,
        }
    }
}

impl FeedExperiment {
    /// The experiment's first day in a run of `run_days` days.
    pub fn first_day(&self, run_days: u32) -> u32 {
        run_days.saturating_sub(self.days)
    }
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            seed: 1,
            days: 365,
            population_size: 2_000,
            initial_user_share: 0.6,
            post_lifetime_days: 3,
            candidate_cap: 80,
            offline_ties: 8,
            initial_follows: 30,
            summary_window: 0.3,
            tracked_agents: 10,
            deactivation: None,
            notifications_off: None,
            feed_experiment: None,
        }
    }
}

/// Shares of the five archetypes; they are normalised, so they need not sum to 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArchetypeShares {
    /// Casual users.
    pub casual: f32,
    /// Creators.
    pub creator: f32,
    /// Partisans.
    pub partisan: f32,
    /// Trolls.
    pub troll: f32,
    /// Vulnerable people.
    pub vulnerable: f32,
}

impl Default for ArchetypeShares {
    fn default() -> Self {
        Self {
            casual: 0.50,
            creator: 0.10,
            partisan: 0.20,
            troll: 0.03,
            vulnerable: 0.17,
        }
    }
}

impl ArchetypeShares {
    /// The shares in archetype order (Casual, Creator, Partisan, Troll, Vulnerable).
    pub fn as_array(&self) -> [f32; 5] {
        [
            self.casual,
            self.creator,
            self.partisan,
            self.troll,
            self.vulnerable,
        ]
    }
}

/// The population: archetype mix and bot accounts (docs/model.md, 3.1 and 3.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PopulationSettings {
    /// Shares of the archetypes.
    pub archetype_shares: ArchetypeShares,
    /// Bot accounts per individual before the account cost.
    pub bot_share: f32,
    /// Posts each bot likes and shares per day.
    pub bot_boosts: u32,
    /// Share of bot posts that are Clickbait; the rest are Outrage.
    pub bot_clickbait_share: f32,
    /// Added to the truth of every bot post, after its draws, and clamped to
    /// [0, 1]: below 0 the farms post more falsehoods than their styles'
    /// profiles give (docs/model.md, 3.5). The default of 0 changes nothing.
    pub bot_truth_shift: f32,
    /// Posts per bot per day.
    pub bot_posts_per_day: u32,
}

impl Default for PopulationSettings {
    fn default() -> Self {
        Self {
            archetype_shares: ArchetypeShares::default(),
            bot_share: 0.05,
            bot_boosts: 8,
            bot_clickbait_share: 0.5,
            bot_truth_shift: 0.0,
            bot_posts_per_day: 1,
        }
    }
}

impl Config {
    /// Checks every value against its documented range.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.validate_run()?;
        let shares = self.population.archetype_shares.as_array();
        if shares.iter().sum::<f32>() <= 0.0 {
            return Err(ConfigError::Invalid(
                "population.archetype_shares must not all be zero".into(),
            ));
        }
        params::validate_ranges(self)
    }

    fn validate_run(&self) -> Result<(), ConfigError> {
        let r = &self.run;
        check_int("run.days", r.days, 1, 100_000)?;
        check_int("run.population_size", r.population_size, 50, 100_000)?;
        check_float("run.initial_user_share", r.initial_user_share, 0.01, 1.0)?;
        check_int("run.post_lifetime_days", r.post_lifetime_days, 1, 14)?;
        check_int("run.candidate_cap", r.candidate_cap, 1, 1_000)?;
        check_int("run.offline_ties", r.offline_ties, 0, 50)?;
        check_int("run.initial_follows", r.initial_follows, 0, 1_000)?;
        check_float("run.summary_window", r.summary_window, 0.01, 1.0)?;
        check_int("run.tracked_agents", r.tracked_agents, 0, 1_000)?;
        let experiments = [
            r.deactivation.is_some(),
            r.notifications_off.is_some(),
            r.feed_experiment.is_some(),
        ];
        if experiments.iter().filter(|&&on| on).count() > 1 {
            return Err(ConfigError::Invalid(
                "at most one of run.deactivation, run.notifications_off and run.feed_experiment \
                 may be set: at most one experiment runs at a time"
                    .into(),
            ));
        }
        if let Some(d) = &r.deactivation {
            let room = r.days.saturating_sub(1);
            check_int(
                "run.deactivation.ends_before_end",
                d.ends_before_end,
                0,
                room.saturating_sub(1),
            )?;
            check_int("run.deactivation.days", d.days, 1, room - d.ends_before_end)?;
            if !(d.share > 0.0 && d.share <= 1.0) {
                return Err(ConfigError::Invalid(format!(
                    "run.deactivation.share must lie above 0 and at most 1, not {}",
                    d.share
                )));
            }
        }
        if let Some(n) = &r.notifications_off {
            let most = r.days.saturating_sub(NOTIFICATIONS_OFF_BASELINE_DAYS);
            check_int("run.notifications_off.days", n.days, 1, most)?;
            if !(n.share > 0.0 && n.share <= 1.0) {
                return Err(ConfigError::Invalid(format!(
                    "run.notifications_off.share must lie above 0 and at most 1, not {}",
                    n.share
                )));
            }
        }
        if let Some(f) = &r.feed_experiment {
            check_int(
                "run.feed_experiment.days",
                f.days,
                1,
                r.days.saturating_sub(1),
            )?;
            if !(f.share > 0.0 && f.share <= 1.0) {
                return Err(ConfigError::Invalid(format!(
                    "run.feed_experiment.share must lie above 0 and at most 1, not {}",
                    f.share
                )));
            }
            match f.kind {
                FeedExperimentKind::HideFlagged { threshold } => {
                    check_float("run.feed_experiment.kind.threshold", threshold, 0.0, 1.0)?;
                }
                FeedExperimentKind::FewerSameGroup { penalty } => {
                    check_float("run.feed_experiment.kind.penalty", penalty, 0.0, 10.0)?;
                }
                FeedExperimentKind::NoReshares | FeedExperimentKind::Chronological => {}
            }
        }
        let p = &self.population;
        check_int("population.bot_boosts", p.bot_boosts, 0, 200)?;
        check_int("population.bot_posts_per_day", p.bot_posts_per_day, 0, 20)?;
        check_float(
            "population.bot_clickbait_share",
            p.bot_clickbait_share,
            0.0,
            1.0,
        )?;
        check_float("population.bot_truth_shift", p.bot_truth_shift, -1.0, 1.0)
    }

    /// Parses a configuration from JSON; missing fields take their defaults.
    pub fn from_json(text: &str) -> Result<Self, ConfigError> {
        let config: Self =
            serde_json::from_str(text).map_err(|e| ConfigError::Json(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// The configuration as pretty-printed JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

fn check_int(path: &str, value: u32, min: u32, max: u32) -> Result<(), ConfigError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ConfigError::OutOfRange {
            path: path.into(),
            value: f64::from(value),
            min: f64::from(min),
            max: f64::from(max),
        })
    }
}

fn check_float(path: &str, value: f32, min: f32, max: f32) -> Result<(), ConfigError> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ConfigError::OutOfRange {
            path: path.into(),
            value: f64::from(value),
            min: f64::from(min),
            max: f64::from(max),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_validates_and_round_trips() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        let parsed = Config::from_json(&config.to_json_pretty());
        assert_eq!(parsed.ok(), Some(config));
    }

    #[test]
    fn partial_json_takes_defaults() {
        let parsed = Config::from_json(r#"{"design": {"chronological": true}}"#);
        assert!(parsed.as_ref().is_ok_and(|c| c.design.chronological));
        assert!(parsed.is_ok_and(|c| c.run.days == 365));
    }

    #[test]
    fn unknown_fields_and_bad_ranges_are_rejected() {
        assert!(Config::from_json(r#"{"design": {"w_likes": 1}}"#).is_err());
        let mut config = Config::default();
        config.design.share_friction = 2.0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::OutOfRange { .. })
        ));
    }

    #[test]
    fn the_deactivation_experiment_is_optional_and_validated() {
        // Absent by default, and absent from the JSON, so existing files and
        // their round trips are unchanged.
        let config = Config::default();
        assert!(config.run.deactivation.is_none());
        assert!(!config.to_json_pretty().contains("deactivation"));
        let parsed = Config::from_json(r#"{"run": {"days": 100, "deactivation": {}}}"#);
        let d = parsed.ok().and_then(|c| c.run.deactivation);
        assert_eq!(d, Some(Deactivation::default()));
        assert_eq!(Deactivation::default().first_day(100), 65);
        let with = |days: u32, share: f32| {
            let mut c = Config::default();
            c.run.days = 100;
            c.run.deactivation = Some(Deactivation {
                days,
                share,
                ..Deactivation::default()
            });
            c.validate().is_ok()
        };
        assert!(with(1, 0.1) && with(99, 1.0));
        assert!(!with(0, 0.1), "no break");
        assert!(!with(100, 0.1), "a break as long as the run");
        assert!(!with(35, 0.0) && !with(35, 1.5) && !with(35, f32::NAN));
        let mut c = Config::default();
        c.run.days = 100;
        c.run.deactivation = Some(Deactivation::default());
        assert!(!c.to_json_pretty().contains("placebo"), "false is left out");
        assert!(
            !c.to_json_pretty().contains("ends_before_end"),
            "0 is left out"
        );
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c.clone()));
        // A placebo run is written out and read back.
        c.run.deactivation = Some(Deactivation {
            placebo: true,
            ..Deactivation::default()
        });
        assert!(c.to_json_pretty().contains("\"placebo\": true"));
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c));
    }

    #[test]
    fn the_break_may_end_before_the_run() {
        let d = Deactivation {
            days: 35,
            ends_before_end: 28,
            ..Deactivation::default()
        };
        assert_eq!((d.first_day(100), d.end_day(100)), (37, 72));
        assert_eq!(Deactivation::default().end_day(100), 100);
        let with = |days: u32, offset: u32| {
            let mut c = Config::default();
            c.run.days = 100;
            c.run.deactivation = Some(Deactivation {
                days,
                ends_before_end: offset,
                ..Deactivation::default()
            });
            c.validate().is_ok()
        };
        assert!(with(35, 28) && with(1, 98) && with(71, 28));
        assert!(!with(72, 28), "no day before the break");
        assert!(!with(1, 99), "no room for a break");
        let mut c = Config::default();
        c.run.deactivation = Some(d);
        assert!(c.to_json_pretty().contains("\"ends_before_end\": 28"));
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c));
    }

    #[test]
    fn the_feed_experiment_is_optional_validated_and_alone() {
        let config = Config::default();
        assert!(config.run.feed_experiment.is_none());
        assert!(!config.to_json_pretty().contains("feed_experiment"));
        let parsed = Config::from_json(r#"{"run": {"days": 100, "feed_experiment": {}}}"#);
        let f = parsed.ok().and_then(|c| c.run.feed_experiment);
        assert_eq!(f, Some(FeedExperiment::default()));
        assert_eq!(FeedExperiment::default().first_day(100), 10);
        let text = r#"{"run": {"feed_experiment": {"kind": {"hide_flagged": {"threshold": 0.4}},
            "days": 42, "share": 0.2}}}"#;
        let f = Config::from_json(text)
            .ok()
            .and_then(|c| c.run.feed_experiment);
        assert_eq!(
            f.map(|f| (f.kind, f.days)),
            Some((FeedExperimentKind::HideFlagged { threshold: 0.4 }, 42))
        );
        let text = r#"{"run": {"feed_experiment": {"kind": "no_reshares"}}}"#;
        assert!(Config::from_json(text).is_ok());
        let text = r#"{"run": {"feed_experiment": {"kind": "chronological"}}}"#;
        let f = Config::from_json(text)
            .ok()
            .and_then(|c| c.run.feed_experiment);
        assert_eq!(f.map(|f| f.kind), Some(FeedExperimentKind::Chronological));
        let text =
            r#"{"run": {"feed_experiment": {"kind": {"fewer_same_group": {"penalty": 0.3}}}}}"#;
        assert!(Config::from_json(text).is_ok());
        let with = |kind: FeedExperimentKind, days: u32, share: f32| {
            let mut c = Config::default();
            c.run.days = 100;
            c.run.feed_experiment = Some(FeedExperiment {
                kind,
                days,
                share,
                placebo: false,
            });
            c.validate().is_ok()
        };
        let none = FeedExperimentKind::NoReshares;
        assert!(with(none, 1, 0.2) && with(none, 99, 1.0));
        assert!(!with(none, 0, 0.2) && !with(none, 100, 0.2));
        assert!(!with(none, 10, 0.0) && !with(none, 10, 1.5) && !with(none, 10, f32::NAN));
        let hide = |threshold| FeedExperimentKind::HideFlagged { threshold };
        assert!(with(hide(0.0), 10, 0.2) && with(hide(1.0), 10, 0.2));
        assert!(!with(hide(-0.1), 10, 0.2) && !with(hide(f32::NAN), 10, 0.2));
        let penalty = |penalty| FeedExperimentKind::FewerSameGroup { penalty };
        assert!(with(penalty(0.5), 10, 0.2) && !with(penalty(-0.5), 10, 0.2));
        // A placebo is written out and read back; false is left out.
        let mut c = Config::default();
        c.run.feed_experiment = Some(FeedExperiment {
            kind: hide(0.45),
            ..FeedExperiment::default()
        });
        assert!(!c.to_json_pretty().contains("placebo"));
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c.clone()));
        if let Some(f) = c.run.feed_experiment.as_mut() {
            f.placebo = true;
        }
        assert!(c.to_json_pretty().contains("\"placebo\": true"));
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c.clone()));
        // At most one experiment at a time.
        let mut both = c.clone();
        both.run.deactivation = Some(Deactivation::default());
        assert!(matches!(both.validate(), Err(ConfigError::Invalid(_))));
        let mut both = c;
        both.run.notifications_off = Some(NotificationsOff::default());
        assert!(matches!(both.validate(), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn the_notifications_off_experiment_is_optional_validated_and_alone() {
        let config = Config::default();
        assert!(config.run.notifications_off.is_none());
        assert!(!config.to_json_pretty().contains("notifications_off"));
        let parsed = Config::from_json(r#"{"run": {"days": 100, "notifications_off": {}}}"#);
        let n = parsed.ok().and_then(|c| c.run.notifications_off);
        assert_eq!(n, Some(NotificationsOff::default()));
        assert_eq!(NotificationsOff::default().first_day(100), 93);
        assert_eq!(NotificationsOff::default().share, 1.0);
        let with = |days: u32, share: f32| {
            let mut c = Config::default();
            c.run.days = 100;
            c.run.notifications_off = Some(NotificationsOff {
                days,
                share,
                placebo: false,
            });
            c.validate().is_ok()
        };
        assert!(with(1, 0.1) && with(93, 1.0));
        assert!(!with(0, 1.0), "no experiment");
        assert!(!with(94, 1.0), "no room for the baseline week");
        assert!(!with(7, 0.0) && !with(7, 1.5) && !with(7, f32::NAN));
        // A placebo run is written out and read back.
        let mut c = Config::default();
        c.run.notifications_off = Some(NotificationsOff {
            placebo: true,
            ..NotificationsOff::default()
        });
        assert!(c.to_json_pretty().contains("\"placebo\": true"));
        assert_eq!(Config::from_json(&c.to_json_pretty()).ok(), Some(c.clone()));
        // At most one experiment at a time; the deactivation JSON still parses.
        c.run.deactivation = Some(Deactivation::default());
        assert!(matches!(c.validate(), Err(ConfigError::Invalid(_))));
        let both = r#"{"run": {"deactivation": {}, "notifications_off": {}}}"#;
        assert!(Config::from_json(both).is_err());
        let old = r#"{"run": {"deactivation": {"days": 35, "share": 1.0, "placebo": true}}}"#;
        let c = Config::from_json(old).expect("the deactivation JSON parses");
        assert!(c.run.notifications_off.is_none());
        assert!(
            c.run
                .deactivation
                .is_some_and(|d| d.placebo && d.share == 1.0)
        );
    }

    #[test]
    fn no_predictor_reads_minds() {
        let mut config = Config::default();
        assert!(config.set_path("design.predictor_quality", 0.95).is_ok());
        assert!(matches!(
            config.set_path("design.predictor_quality", 1.0),
            Err(ConfigError::OutOfRange { .. })
        ));
        assert!((config.design.predictor_quality - 0.95).abs() < 1e-6);
    }

    #[test]
    fn the_bot_strategy_is_a_choice() {
        let mut config = Config::default();
        assert_eq!(config.world.bot_strategy, BotStrategy::Adaptive);
        assert!(config.set_assignment("world.bot_strategy=own-side").is_ok());
        assert_eq!(config.world.bot_strategy, BotStrategy::OwnSide);
        assert!(
            config
                .set_assignment("world.bot_strategy=shared-pool")
                .is_ok()
        );
        assert_eq!(config.world.bot_strategy, BotStrategy::SharedPool);
        assert!(config.set_assignment("world.bot_strategy=sneaky").is_err());
        assert!(config.set_assignment("world.bot_brigading=6").is_err());
        assert!(
            config
                .set_assignment("world.polarisation_backfire=0.5")
                .is_ok()
        );
    }
}
