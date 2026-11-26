//! The incentive lab's simulation core: a deterministic, engine-free toy model
//! of incentive systems on one social platform, specified in `docs/model.md`.
//!
//! A [`Config`] holds the run settings, the population, the platform's design
//! levers and the world assumptions. [`Simulation`] runs it one day at a time
//! and records [`DailyMetrics`], each metric labelled observable (what the
//! platform can measure) or truth (what only the simulation knows).
//!
//! ```
//! use incentive_core::{Config, MetricId, Simulation};
//!
//! let mut config = Config::default();
//! config.run.population_size = 200;
//! config.run.days = 10;
//! let mut sim = Simulation::new(config)?;
//! let summary = sim.run_to_end();
//! assert!(summary.get(MetricId::EngagementIndex) > 0.0);
//! # Ok::<(), incentive_core::ConfigError>(())
//! ```

pub mod atlas;
mod behaviour;
mod bots;
mod config;
mod config_path;
mod content;
mod economy;
mod error;
mod experiment;
mod facts;
mod init;
mod inspector;
mod learning;
mod lifecycle;
mod measure;
mod metrics;
mod moderation;
mod network;
mod params;
mod person;
mod population;
mod posting;
mod posts;
mod presets;
mod ranking;
mod rng;
mod scenarios;
mod session;
mod simulation;
mod society;
pub mod stats;
mod world;

pub use atlas::{
    AtlasDesign, AtlasMetrics, ExploreSettings, GoodhartAtlas, GoodhartGeneration, MapAtlas,
    MapCell, MapGrid, ParetoAtlas, inert_levers,
};
pub use config::{
    AltFeed, ArchetypeShares, BotStrategy, Config, DEACTIVATION_DAYS, DEACTIVATION_SHARE,
    Deactivation, DesignLevers, ModerationAction, PayoutScheme, PopulationSettings, RunSettings,
    Sanctions, WorldAssumptions,
};
pub use config_path::FieldValue;
pub use content::{Group, Post, STYLE_COUNT, Style, StyleProfile, TOPIC_COUNT, TOPICS};
pub use error::ConfigError;
pub use facts::{Bound, Check, CheckResult, Fact, FactRun, Variant, facts};
pub use inspector::{
    AGENT_HISTORY_DAYS, AgentDay, AgentView, ExplainedItem, FeedExplanation, ItemActions,
    TrueResponse,
};
pub use metrics::{
    DailyMetrics, Family, Format, METRIC_COUNT, MOVING_STANDARD_ERRORS, MOVING_THRESHOLD, MetricId,
    MetricInfo, Summary, Visibility, is_moving_over, is_moving_slope, metric_infos,
};
pub use params::{ChoiceInfo, ParamGroup, ParamInfo, ParamKind, ParamSide, param, params};
pub use person::{Person, PersonState};
pub use population::{ARCHETYPE_COUNT, Archetype, Traits};
pub use presets::{Preset, preset, presets};
pub use ranking::{FeedKind, ScoreComponents};
pub use scenarios::{
    Comparison, Goal, GoalResult, GoalVerdict, Scenario, ScenarioVerdict, Verdict, scenario,
    scenarios,
};
pub use simulation::Simulation;
pub use stats::{MeanInterval, PairedDifference, Stats};

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync_clone<T: Send + Sync + Clone>() {}

    #[test]
    fn simulation_is_send_sync_clone() {
        assert_send_sync_clone::<Simulation>();
    }
}
