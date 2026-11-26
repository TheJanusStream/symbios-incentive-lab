//! The run configuration: run settings, population, design levers and world
//! assumptions (docs/model.md, sections 10 and 11).

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::params;

mod assumptions;
mod design;

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
}

/// Days of a deactivation experiment's break by default.
pub const DEACTIVATION_DAYS: u32 = 35;
/// Share of the eligible users a deactivation experiment sends on the break
/// by default.
pub const DEACTIVATION_SHARE: f32 = 0.1;

/// A deactivation experiment (docs/model.md, 13.2, and fact 6 of section
/// 15): over the last `days` days of the run, a random `share` of the regular
/// users stays off the platform, as the paid participants of a field
/// experiment do. Its effect within one run is the truth metric
/// `deactivation_effect`; the truth metric `break_change` is the break
/// group's own change, compared across a real run and its placebo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Deactivation {
    /// Days the break lasts; it ends with the run, so it starts this many
    /// days before the end. At least 1 and less than the run's days.
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
}

impl Default for Deactivation {
    fn default() -> Self {
        Self {
            days: DEACTIVATION_DAYS,
            share: DEACTIVATION_SHARE,
            placebo: false,
        }
    }
}

impl Deactivation {
    /// The first day of the break in a run of `run_days` days.
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
        if let Some(d) = &r.deactivation {
            check_int("run.deactivation.days", d.days, 1, r.days.saturating_sub(1))?;
            if !(d.share > 0.0 && d.share <= 1.0) {
                return Err(ConfigError::Invalid(format!(
                    "run.deactivation.share must lie above 0 and at most 1, not {}",
                    d.share
                )));
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
        )
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
                placebo: false,
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
