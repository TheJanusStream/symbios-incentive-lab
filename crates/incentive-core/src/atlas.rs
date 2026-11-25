//! The exploration atlas: what `incentive explore` writes and the app's Atlas
//! tab reads. Plain serde types only; the searches themselves live in the
//! command-line runner, so neither this crate nor the browser bundle carries
//! any evolutionary code.
//!
//! Three kinds of file share [`ExploreSettings`] and [`AtlasDesign`]:
//!
//! - [`MapAtlas`]: a MAP-Elites map over the engagement index (across) and
//!   the harm index (up); each filled cell holds the design with the highest
//!   benefit index found there.
//! - [`ParetoAtlas`]: the non-dominated front of an NSGA-II search that
//!   maximises engagement, minimises harm and maximises benefit.
//! - [`GoodhartAtlas`]: a search on one observable proxy alone, with the
//!   best design of every generation and its true outcomes.

use serde::{Deserialize, Deserializer, Serialize};

use crate::config::{Config, DesignLevers, ModerationAction};
use crate::metrics::{MetricId, Summary};

/// How a search was run: everything needed to reproduce it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExploreSettings {
    /// `map`, `pareto` or `goodhart`.
    pub mode: String,
    /// Seed of the search's own random choices.
    pub seed: u64,
    /// Simulation seeds every candidate design ran on; metrics are the mean.
    pub seeds: Vec<u64>,
    /// Individuals per simulated run.
    pub population: u32,
    /// Days per simulated run.
    pub days: u32,
    /// Generations (batches after the initial designs).
    pub generations: u32,
    /// Candidates per generation (the population size of NSGA-II and of the
    /// Goodhart search).
    pub batch: u32,
    /// Designs evaluated before the first generation.
    pub initial: u32,
    /// Chance that each gene mutates in a child.
    pub mutation_rate: f32,
    /// Distinct designs simulated (repeats are looked up, not rerun).
    pub evaluations: u32,
    /// Simulation runs: `evaluations` times the number of seeds.
    pub runs: u32,
    /// Where the world assumptions came from: `defaults` or a file name.
    pub world: String,
    /// The command that reproduces this file (without `--out`).
    pub command: String,
    /// The configuration every candidate started from: run settings,
    /// population and world assumptions. Its design is not searched from;
    /// every design lever is a gene.
    pub base: Config,
}

/// The key metrics of an evaluated design: means over its seeds, each over
/// the summary window. A metric without a value is NaN (written as null).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasMetrics {
    /// Engagement index (observable).
    #[serde(deserialize_with = "nan_from_null")]
    pub engagement_index: f32,
    /// Harm index (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub harm_index: f32,
    /// Benefit index (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub benefit_index: f32,
    /// Share of impressions that are toxic (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub toxic_share: f32,
    /// Mean wellbeing of everyone who ever used the platform (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub wellbeing_ever_users: f32,
    /// Share of Creators earning a living (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub creators_living: f32,
    /// Society-wide affective polarisation (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub polarisation_society: f32,
    /// Share of individuals who are users (truth).
    #[serde(deserialize_with = "nan_from_null")]
    pub user_share: f32,
    /// Weekly survey satisfaction (observable).
    #[serde(deserialize_with = "nan_from_null")]
    pub survey_satisfaction: f32,
}

fn nan_from_null<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(Option::<f32>::deserialize(deserializer)?.unwrap_or(f32::NAN))
}

/// Rounds to four decimals, which keeps atlas files small; the metrics'
/// spread across seeds is far larger than the rounding.
pub fn round4(x: f32) -> f32 {
    if x.is_finite() {
        ((f64::from(x) * 1e4).round() / 1e4) as f32
    } else {
        x
    }
}

impl AtlasMetrics {
    /// The metrics in display order.
    pub const IDS: [MetricId; 9] = [
        MetricId::EngagementIndex,
        MetricId::HarmIndex,
        MetricId::BenefitIndex,
        MetricId::ToxicShare,
        MetricId::WellbeingEverUsers,
        MetricId::CreatorsLiving,
        MetricId::PolarisationSociety,
        MetricId::UserShare,
        MetricId::SurveySatisfaction,
    ];

    /// The key metrics of a (mean) summary, rounded to four decimals.
    pub fn from_summary(summary: &Summary) -> Self {
        let g = |id| round4(summary.get(id));
        Self {
            engagement_index: g(MetricId::EngagementIndex),
            harm_index: g(MetricId::HarmIndex),
            benefit_index: g(MetricId::BenefitIndex),
            toxic_share: g(MetricId::ToxicShare),
            wellbeing_ever_users: g(MetricId::WellbeingEverUsers),
            creators_living: g(MetricId::CreatorsLiving),
            polarisation_society: g(MetricId::PolarisationSociety),
            user_share: g(MetricId::UserShare),
            survey_satisfaction: g(MetricId::SurveySatisfaction),
        }
    }

    /// One of the key metrics; `None` for any other metric.
    pub fn get(&self, id: MetricId) -> Option<f32> {
        Some(match id {
            MetricId::EngagementIndex => self.engagement_index,
            MetricId::HarmIndex => self.harm_index,
            MetricId::BenefitIndex => self.benefit_index,
            MetricId::ToxicShare => self.toxic_share,
            MetricId::WellbeingEverUsers => self.wellbeing_ever_users,
            MetricId::CreatorsLiving => self.creators_living,
            MetricId::PolarisationSociety => self.polarisation_society,
            MetricId::UserShare => self.user_share,
            MetricId::SurveySatisfaction => self.survey_satisfaction,
            _ => return None,
        })
    }
}

/// The design levers that have no effect in `design`, by dotted path: a
/// search leaves them wherever mutation put them, so a list of how a found
/// design differs from another should skip them.
///
/// - The alternative feed, when nobody uses it (`alt_feed_share` 0).
/// - The classifier's threshold and action, when the classifier is off.
/// - The demotion penalty, unless the classifier demotes.
/// - The noted penalty, without community notes.
/// - The payout scheme and follower threshold, when nothing is paid out.
pub fn inert_levers(design: &DesignLevers) -> Vec<&'static str> {
    let mut inert = Vec::new();
    if design.alt_feed_share <= 0.0 {
        inert.push("design.alt_feed");
    }
    if !design.classifier_enabled {
        inert.extend(["design.classifier_threshold", "design.moderation_action"]);
    }
    if !(design.classifier_enabled && design.moderation_action == ModerationAction::Demote) {
        inert.push("design.demote_penalty");
    }
    if !design.community_notes {
        inert.push("design.noted_penalty");
    }
    if design.payout_share <= 0.0 {
        inert.extend(["design.payout_scheme", "design.payout_threshold_followers"]);
    }
    inert
}

/// A design found by a search, with its key metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasDesign {
    /// Every design lever.
    pub design: DesignLevers,
    /// Its key metrics at the search's size.
    pub metrics: AtlasMetrics,
}

/// The fixed grid of a map: `resolution` cells across the engagement range
/// and as many up the harm range. Values outside a range fall into the edge
/// cells.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapGrid {
    /// Cells per axis.
    pub resolution: u32,
    /// Engagement index at the left edge and the right edge.
    pub engagement: [f32; 2],
    /// Harm index at the bottom edge and the top edge.
    pub harm: [f32; 2],
}

impl MapGrid {
    /// A value's position in `range`, from 0 to 1 (not clamped).
    pub fn normalise(value: f32, range: [f32; 2]) -> f32 {
        (value - range[0]) / (range[1] - range[0])
    }

    /// The cell index of a normalised value, edge cells taking everything
    /// beyond them.
    pub fn index(&self, normalised: f32) -> u32 {
        let n = self.resolution.max(1);
        let scaled = normalised.clamp(0.0, 1.0) * n as f32;
        (scaled.floor() as u32).min(n - 1)
    }

    /// The cell `(x, y)` of an engagement and a harm index.
    pub fn cell_of(&self, engagement: f32, harm: f32) -> (u32, u32) {
        (
            self.index(Self::normalise(engagement, self.engagement)),
            self.index(Self::normalise(harm, self.harm)),
        )
    }

    /// The engagement range `[low, high)` of column `x`.
    pub fn column(&self, x: u32) -> [f32; 2] {
        Self::slice(self.engagement, x, self.resolution)
    }

    /// The harm range `[low, high)` of row `y`.
    pub fn row(&self, y: u32) -> [f32; 2] {
        Self::slice(self.harm, y, self.resolution)
    }

    fn slice(range: [f32; 2], i: u32, n: u32) -> [f32; 2] {
        let w = (range[1] - range[0]) / n.max(1) as f32;
        [range[0] + w * i as f32, range[0] + w * (i + 1) as f32]
    }
}

/// One filled cell of a map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapCell {
    /// Column (engagement), from 0 at the left.
    pub x: u32,
    /// Row (harm), from 0 at the bottom.
    pub y: u32,
    /// The cell's elite: the highest benefit index found in it.
    pub elite: AtlasDesign,
}

/// A MAP-Elites map of the design space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapAtlas {
    /// How the search ran.
    pub settings: ExploreSettings,
    /// The grid.
    pub grid: MapGrid,
    /// Every filled cell, by column then row.
    pub cells: Vec<MapCell>,
}

impl MapAtlas {
    /// The share of cells that are filled.
    pub fn coverage(&self) -> f32 {
        let n = self.grid.resolution.max(1) as f32;
        self.cells.len() as f32 / (n * n)
    }

    /// The cell at `(x, y)`, if filled.
    pub fn cell(&self, x: u32, y: u32) -> Option<&MapCell> {
        self.cells.iter().find(|c| c.x == x && c.y == y)
    }
}

/// The Pareto front of a three-objective search.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParetoAtlas {
    /// How the search ran.
    pub settings: ExploreSettings,
    /// The non-dominated designs, by engagement index.
    pub front: Vec<AtlasDesign>,
}

/// The best design of one generation of a Goodhart search.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoodhartGeneration {
    /// Generation, 0 for the initial random designs.
    pub generation: u32,
    /// The proxy value of the generation's best design.
    #[serde(deserialize_with = "nan_from_null")]
    pub proxy: f32,
    /// That design and its true outcomes.
    pub best: AtlasDesign,
}

/// A search on one observable proxy, with the truth recorded alongside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoodhartAtlas {
    /// How the search ran.
    pub settings: ExploreSettings,
    /// The key of the observable metric that was maximised.
    pub proxy: String,
    /// Every generation's best design.
    pub generations: Vec<GoodhartGeneration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inert_levers_follow_the_switches_they_depend_on() {
        let default = inert_levers(&DesignLevers::default());
        assert_eq!(
            default,
            [
                "design.alt_feed",
                "design.demote_penalty",
                "design.noted_penalty",
                "design.payout_scheme",
                "design.payout_threshold_followers"
            ]
        );
        let busy = DesignLevers {
            alt_feed_share: 0.2,
            moderation_action: ModerationAction::Demote,
            community_notes: true,
            payout_share: 0.3,
            ..DesignLevers::default()
        };
        assert!(inert_levers(&busy).is_empty());
        let off = DesignLevers {
            classifier_enabled: false,
            ..busy
        };
        assert_eq!(
            inert_levers(&off),
            [
                "design.classifier_threshold",
                "design.moderation_action",
                "design.demote_penalty"
            ]
        );
    }

    #[test]
    fn grid_cells_clamp_into_the_edges() {
        let grid = MapGrid {
            resolution: 16,
            engagement: [0.0, 0.32],
            harm: [0.0, 0.64],
        };
        assert_eq!(grid.cell_of(0.0, 0.0), (0, 0));
        assert_eq!(grid.cell_of(-1.0, 9.0), (0, 15));
        assert_eq!(grid.cell_of(0.32, 0.64), (15, 15));
        assert_eq!(grid.cell_of(0.03, 0.1), (1, 2));
        let col = grid.column(1);
        assert!((col[0] - 0.02).abs() < 1e-6 && (col[1] - 0.04).abs() < 1e-6);
        let row = grid.row(15);
        assert!((row[1] - 0.64).abs() < 1e-6);
    }

    #[test]
    fn metrics_round_and_survive_json_with_missing_values() {
        let mut m = AtlasMetrics {
            engagement_index: round4(0.262_54),
            harm_index: 0.5,
            benefit_index: 0.4,
            toxic_share: 0.1,
            wellbeing_ever_users: 0.6,
            creators_living: 0.0,
            polarisation_society: 0.45,
            user_share: 0.66,
            survey_satisfaction: f32::NAN,
        };
        assert_eq!(m.engagement_index, 0.2625);
        let text = serde_json::to_string(&m).unwrap_or_default();
        assert!(text.contains("\"survey_satisfaction\":null"));
        let back: AtlasMetrics = serde_json::from_str(&text).unwrap_or(m);
        assert!(back.survey_satisfaction.is_nan());
        assert_eq!(back.get(MetricId::HarmIndex), Some(0.5));
        assert_eq!(back.get(MetricId::Impressions), None);
        m.survey_satisfaction = 0.7;
        for id in AtlasMetrics::IDS {
            assert!(m.get(id).is_some(), "{id:?}");
        }
    }
}
