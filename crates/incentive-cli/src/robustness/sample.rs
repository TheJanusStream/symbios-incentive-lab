//! The worlds of `incentive robustness`: which world assumptions vary, over
//! which range, and how a world is drawn from the sample seed.
//!
//! - Varied: every world assumption of the parameter catalogue (`world.*`)
//!   except [`EXCLUDED`], and the two population settings of [`POPULATION`],
//!   the troll share and the bot share. Run and design settings and the rest
//!   of the population never vary.
//! - Local: each varied continuous assumption with a nonzero default is
//!   multiplied by U(1 - jitter, 1 + jitter) and clamped to its catalogue
//!   range; those whose default is 0 (mechanisms off) and the categorical
//!   ones keep their defaults.
//! - Global: a Latin hypercube over each varied continuous assumption's
//!   range; categorical assumptions are drawn uniformly, except under the
//!   plausible ranges, which hold them at their defaults. The ranges are
//!   chosen by [`Ranges`]: as documented, revised by the rule of lab #31, or
//!   plausible by the rule of lab #35 ([`range_for`], [`PLAUSIBLE_RULES`]).

use clap::ValueEnum;
use incentive_core::{Config, ConfigError, FieldValue, ParamInfo, ParamKind, params};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use serde::Serialize;

/// World assumptions that keep their defaults in every mode: the
/// checking-habit trio and the interruption cost, off by the owner's decision
/// because they change the model's time scale (docs/model.md, 6.1 and 13.2),
/// and the session length `base_session_items`, held for the same reason by
/// the decision of lab #35: it sets how many items a visit holds, the time
/// scale every per-impression rate was calibrated at.
pub const EXCLUDED: [&str; 5] = [
    "world.recheck_rate",
    "world.notification_pull",
    "world.extra_session_share",
    "world.interruption_cost",
    "world.base_session_items",
];

/// The population settings that vary beside the world assumptions (lab #31):
/// the troll share, a relative share that the population normalises with the
/// other archetypes' when it draws them (docs/model.md, 3.1), and the bot
/// accounts per individual.
pub const POPULATION: [&str; 2] = ["population.archetype_shares.troll", "population.bot_share"];

/// The ranges to explore as docs/model.md, section 11, documented them for
/// the specification's text defaults, before lab #31 revised them, by path.
/// Assumptions it listed with "-" (no range) are absent.
pub const DOCUMENTED_RANGES: [(&str, f64, f64); 50] = [
    ("world.outrage_appeal", 0.0, 3.0),
    ("world.arousal_appeal", 0.0, 2.0),
    ("world.social_proof", 0.0, 1.0),
    ("world.harassment_harm", 0.0, 6.0),
    ("world.comparison_cost", 0.0, 0.5),
    ("world.regret_cost", 0.0, 0.2),
    ("world.time_cost", 0.0, 0.4),
    ("world.interruption_cost", 0.0, 2.0),
    ("world.wrongful_sanction_cost", 0.0, 2.0),
    ("world.learning_rate", 0.0, 0.5),
    ("world.imitation_rate", 0.0, 0.1),
    ("world.choice_temperature", 0.05, 1.0),
    ("world.evasion_rate", 0.0, 0.5),
    ("world.evasion_decay", 0.0, 0.1),
    ("world.evasion_cost", 0.0, 1.0),
    ("world.classifier_retraining", 0.0, 1.0),
    ("world.intensity_rate", 0.0, 0.5),
    ("world.deterrence", 0.0, 2.0),
    ("world.habit_gain", 0.0, 0.3),
    ("world.stop_bias", -5.0, 0.0),
    ("world.churn_base", 0.0, 0.1),
    ("world.leave_threshold", -0.5, 0.5),
    ("world.harassment_exit", 0.0, 0.2),
    ("world.join_base", 0.0, 0.02),
    ("world.reputation_sensitivity", 0.0, 5.0),
    ("world.homophily", 0.5, 1.0),
    ("world.account_cost_deterrence", 0.0, 1.0),
    ("world.bot_farm_floor", 0.0, 1.0),
    ("world.bot_brigading", 0.0, 5.0),
    ("world.polarisation_rate", 0.0, 0.02),
    ("world.contact_rate", 0.0, 0.05),
    ("world.spillover_rate", 0.0, 0.05),
    ("world.polarisation_backfire", 0.0, 1.0),
    ("world.credulity_scale", 0.0, 2.0),
    ("world.subscription_price", 1.0, 4.0),
    ("world.subscription_uptake", 0.01, 0.12),
    ("world.subscription_elasticity", 0.5, 2.0),
    ("world.base_session_items", 5.0, 60.0),
    ("world.recheck_rate", 0.0, 40.0),
    ("world.notification_pull", 0.0, 20.0),
    ("world.extra_session_share", 0.0, 1.0),
    ("world.quality_signal_noise", 0.0, 0.5),
    ("world.polish_gain", 0.0, 1.0),
    ("world.polish_cost", 0.0, 1.0),
    ("world.reflective_quality_weight", 0.0, 2.0),
    ("world.in_group_outrage_cost", 0.0, 1.0),
    ("world.clickbait_penalty", 0.0, 1.0),
    ("world.style_quality_gap", 0.0, 1.5),
    ("world.survey_impulsive_weight", 0.0, 0.5),
    ("world.survey_habit_weight", 0.0, 0.5),
];

/// The defaults in the specification's text (docs/model.md, section 11's
/// Default column) that calibration changed, as 13.1 records, by path: the
/// documented ranges were written for these values. Every other section 11
/// default is the code's.
pub const TEXT_DEFAULTS: [(&str, f64); 7] = [
    ("world.classifier_retraining", 0.2),
    ("world.intensity_rate", 0.05),
    ("world.habit_gain", 0.08),
    ("world.leave_threshold", 0.15),
    ("world.polarisation_rate", 0.004),
    ("world.contact_rate", 0.01),
    ("world.living_income", 1.0),
];

/// Judgement ranges that replace the revised rule's range (lab #35):
/// `leave_threshold`, whose default calibration set to 0, gets -0.1 to 0.1,
/// two thirds each way of the text default's 0.15 (0.15 lost two thirds of
/// the users a year on the rescaled stay utility, docs/model.md 13.1);
/// `outgroup_like_cut`, a world assumption since lab #36, explores its whole
/// catalogue range, 0 to 1; `exposure_contagion` (lab #38) explores from off
/// to twice its calibrated default of 0.28, 0 to 0.56.
pub const JUDGEMENT_RANGES: [(&str, f64, f64); 3] = [
    ("world.leave_threshold", -0.1, 0.1),
    ("world.outgroup_like_cut", 0.0, 1.0),
    ("world.exposure_contagion", 0.0, 0.56),
];

/// Which global ranges to use: [`DOCUMENTED_RANGES`], the revised ranges of
/// [`range_for`], or the plausible ranges of [`PlausibleRule`] and
/// [`PLAUSIBLE_RULES`]. The variants' own docs are the command's help, so
/// they stay plain text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ranges {
    /// Section 11's ranges as documented before lab #31, clipped to the
    /// catalogue; the catalogue range where it gave none.
    Documented,
    /// The ranges to explore: lab #31's rule with the judgement ranges of
    /// lab #35, as section 11 now lists them.
    Revised,
    /// Plausible ranges around the defaults, by lab #35's rule; categorical
    /// assumptions keep their defaults.
    Plausible,
}

impl Ranges {
    /// The choice's name, as on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Documented => "documented",
            Self::Revised => "revised",
            Self::Plausible => "plausible",
        }
    }
}

/// The rule that gives a varied number its plausible range (lab #35).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlausibleRule {
    /// A positive scale (a strength, cost, rate, per-day or per-event chance,
    /// price or noise; `stop_bias`, a negative scale, by its magnitude):
    /// default/1.5 to 1.5 x default.
    Scale,
    /// A share or weight on a scale of 0 to 1 (a fraction of something, or a
    /// mixing weight): default - 0.15 to default + 0.15, clamped.
    Share,
    /// `leave_threshold`: -0.05 to 0.05.
    LeaveThreshold,
    /// A population setting ([`POPULATION`]): default/1.5 to 1.5 x default.
    Population,
}

impl PlausibleRule {
    /// Short text for tables.
    pub fn label(self) -> &'static str {
        match self {
            Self::Scale => "positive scale: default/1.5 to 1.5 x default",
            Self::Share => "share or weight in [0, 1]: default +- 0.15",
            Self::LeaveThreshold => "leave threshold: -0.05 to 0.05",
            Self::Population => "population setting: default/1.5 to 1.5 x default",
        }
    }
}

/// Every varied number's plausible rule, in catalogue order. A rate or a
/// chance per day or per event is a scale, even when 1 bounds it: its size
/// comes from calibration in units of time or events, and a shift of 0.15
/// would multiply a small one many times. A share or weight is a fraction
/// of something (a cost, a skill, a count, a survey answer) or a mixing
/// weight, whose meaning is on the scale of 0 to 1 itself.
pub const PLAUSIBLE_RULES: [(&str, PlausibleRule); 53] = {
    use PlausibleRule::{LeaveThreshold as L, Population as P, Scale as S, Share as W};
    [
        ("world.outrage_appeal", S),
        ("world.arousal_appeal", S),
        ("world.social_proof", W),
        ("world.harassment_harm", S),
        ("world.comparison_cost", S),
        ("world.regret_cost", S),
        ("world.time_cost", S),
        ("world.wrongful_sanction_cost", S),
        ("world.learning_rate", S),
        ("world.imitation_rate", S),
        ("world.choice_temperature", S),
        ("world.evasion_rate", S),
        ("world.evasion_decay", S),
        ("world.evasion_cost", W),
        ("world.classifier_retraining", S),
        ("world.intensity_rate", S),
        ("world.deterrence", S),
        ("world.exposure_contagion", S),
        ("world.habit_gain", S),
        ("world.stop_bias", S),
        ("world.churn_base", S),
        ("world.leave_threshold", L),
        ("world.harassment_exit", S),
        ("world.join_base", S),
        ("world.reputation_sensitivity", S),
        ("world.account_cost_deterrence", W),
        ("world.homophily", W),
        ("world.polarisation_rate", S),
        ("world.contact_rate", S),
        ("world.spillover_rate", S),
        ("world.credulity_scale", S),
        ("world.polarisation_backfire", W),
        ("world.ad_rate", S),
        ("world.subscription_price", S),
        ("world.subscription_hurdle", S),
        ("world.subscription_uptake", S),
        ("world.subscription_elasticity", S),
        ("world.ad_annoyance", S),
        ("world.living_income", S),
        ("population.archetype_shares.troll", P),
        ("population.bot_share", P),
        ("world.bot_farm_floor", W),
        ("world.bot_brigading", S),
        ("world.polish_gain", S),
        ("world.polish_cost", S),
        ("world.reflective_quality_weight", S),
        ("world.in_group_outrage_cost", W),
        ("world.outgroup_like_cut", W),
        ("world.clickbait_penalty", S),
        ("world.style_quality_gap", S),
        ("world.quality_signal_noise", S),
        ("world.survey_impulsive_weight", W),
        ("world.survey_habit_weight", W),
    ]
};

/// The plausible rule of a path, if [`PLAUSIBLE_RULES`] lists it.
pub fn plausible_rule(path: &str) -> Option<PlausibleRule> {
    PLAUSIBLE_RULES
        .iter()
        .find(|(p, _)| *p == path)
        .map(|&(_, r)| r)
}

/// Where a global range comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RangeSource {
    /// Section 11's documented range.
    Docs,
    /// Section 11's documented range, clipped to the catalogue range it
    /// exceeds.
    DocsClipped,
    /// The catalogue range: section 11 documented none (documented ranges
    /// only).
    Catalogue,
    /// Section 11's documented range scaled by the factor by which
    /// calibration changed the default (13.1), clipped to the catalogue.
    Scaled,
    /// Default/3 to 3 x default, clipped to the catalogue: section 11
    /// documented none.
    DefaultThirds,
    /// Default/2 to 2 x default, clipped to the catalogue: a population
    /// setting.
    Population,
    /// A judgement range of lab #35 or #38 ([`JUDGEMENT_RANGES`]).
    Judgement,
    /// A plausible range by its rule, clipped to the catalogue.
    Plausible(PlausibleRule),
    /// No plausible rule: the default alone (no varied number is left
    /// unclassified; a test checks it).
    Unclassified,
}

impl RangeSource {
    /// Short text for tables.
    pub fn label(self) -> &'static str {
        match self {
            Self::Docs => "docs 11",
            Self::DocsClipped => "docs 11, clipped to catalogue",
            Self::Catalogue => "catalogue",
            Self::Scaled => "docs 11 scaled by 13.1's factor",
            Self::DefaultThirds => "default/3 to 3 x default",
            Self::Population => "default/2 to 2 x default",
            Self::Judgement => "judgement (labs #35 and #38)",
            Self::Plausible(rule) => rule.label(),
            Self::Unclassified => "unclassified: the default alone",
        }
    }
}

/// The global range of a continuous assumption with this default and
/// catalogue range, and where it comes from. Documented: section 11's
/// documented range clipped to the catalogue, or the catalogue range where
/// it gave none. Revised, the rule fixed before the robustness runs
/// (lab #31), with the judgement ranges of lab #35:
///
/// - an assumption of [`JUDGEMENT_RANGES`] gets its judgement range;
/// - a documented range written for a text default that 13.1 records as
///   changed by calibration ([`TEXT_DEFAULTS`]) is scaled by the same factor
///   as the default, code default over text default;
/// - any other documented range is kept;
/// - an assumption without a documented range gets default/3 to 3 x
///   default.
///
/// Both clip to the catalogue range, and the population settings
/// ([`POPULATION`]) get default/2 to 2 x default in both. Plausible: the
/// range of the assumption's [`PlausibleRule`], clipped to the catalogue.
pub fn range_for(
    path: &str,
    ranges: Ranges,
    default: f64,
    catalogue: (f64, f64),
) -> ((f64, f64), RangeSource) {
    let clip = |lo: f64, hi: f64| (lo.max(catalogue.0), hi.min(catalogue.1));
    let ordered = |a: f64, b: f64| (a.min(b), a.max(b));
    if ranges == Ranges::Plausible {
        let Some(rule) = plausible_rule(path) else {
            return ((default, default), RangeSource::Unclassified);
        };
        let (lo, hi) = match rule {
            PlausibleRule::Scale | PlausibleRule::Population => {
                ordered(shortest(default / 1.5), shortest(default * 1.5))
            }
            PlausibleRule::Share => (
                shortest((default - 0.15).max(0.0)),
                shortest((default + 0.15).min(1.0)),
            ),
            PlausibleRule::LeaveThreshold => (-0.05, 0.05),
        };
        return (clip(lo, hi), RangeSource::Plausible(rule));
    }
    if POPULATION.contains(&path) {
        let (lo, hi) = ordered(default / 2.0, default * 2.0);
        return (clip(lo, hi), RangeSource::Population);
    }
    if ranges == Ranges::Revised
        && let Some(&(_, lo, hi)) = JUDGEMENT_RANGES.iter().find(|(p, ..)| *p == path)
    {
        return (clip(lo, hi), RangeSource::Judgement);
    }
    let documented = DOCUMENTED_RANGES
        .iter()
        .find(|(p, ..)| *p == path)
        .map(|&(_, lo, hi)| (lo, hi));
    let text_default = TEXT_DEFAULTS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|&(_, d)| d);
    match (ranges, documented, text_default) {
        (Ranges::Revised, Some((lo, hi)), Some(text)) => {
            let factor = default / text;
            let (lo, hi) = ordered(shortest(lo * factor), shortest(hi * factor));
            (clip(lo, hi), RangeSource::Scaled)
        }
        (_, Some((lo, hi)), _) => {
            let clipped = clip(lo, hi);
            let source = if clipped == (lo, hi) {
                RangeSource::Docs
            } else {
                RangeSource::DocsClipped
            };
            (clipped, source)
        }
        (Ranges::Revised, None, _) if default != 0.0 => {
            let (lo, hi) = ordered(shortest(default / 3.0), shortest(default * 3.0));
            (clip(lo, hi), RangeSource::DefaultThirds)
        }
        _ => (catalogue, RangeSource::Catalogue),
    }
}

/// How a varied assumption varies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A number: its default, catalogue range and global range.
    Continuous {
        /// The default value.
        default: f64,
        /// The catalogue range, `[min, max]`.
        catalogue: (f64, f64),
        /// The global range to explore.
        global: (f64, f64),
        /// Where the global range comes from.
        source: RangeSource,
    },
    /// One of a set of choices.
    Categorical {
        /// The choices' ids, in catalogue order.
        choices: &'static [incentive_core::ChoiceInfo],
        /// The default choice's index.
        default: usize,
        /// Whether global mode draws it uniformly; the plausible ranges hold
        /// it at its default.
        drawn: bool,
    },
}

/// One varied world assumption.
#[derive(Clone, Copy, Debug)]
pub struct Varied {
    /// Its catalogue entry.
    pub info: &'static ParamInfo,
    /// How it varies.
    pub kind: Kind,
}

impl Varied {
    /// Whether local mode jitters it: continuous with a nonzero default.
    pub fn jittered(&self) -> bool {
        matches!(self.kind, Kind::Continuous { default, .. } if default != 0.0)
    }

    /// Whether global mode varies it: categorical and drawn, or continuous
    /// with a range wider than one value.
    pub fn spans(&self) -> bool {
        match self.kind {
            Kind::Continuous { global, .. } => global.1 > global.0,
            Kind::Categorical { drawn, .. } => drawn,
        }
    }
}

/// Every varied assumption, in catalogue order, with its global range from
/// `ranges`.
pub fn varied(ranges: Ranges) -> Result<Vec<Varied>, ConfigError> {
    let defaults = Config::default();
    params()
        .iter()
        .filter(|p| {
            (p.path.starts_with("world.") && !EXCLUDED.contains(&p.path))
                || POPULATION.contains(&p.path)
        })
        .filter_map(|info| match info.kind {
            ParamKind::Range { min, max, .. } => Some((info, Some((min, max)))),
            ParamKind::Choice(_) => Some((info, None)),
            _ => None,
        })
        .map(|(info, range)| {
            let kind = match (range, info.kind) {
                (Some((min, max)), _) => {
                    let default = shortest(defaults.get_number(info.path)?.unwrap_or(0.0));
                    let catalogue = (shortest(f64::from(min)), shortest(f64::from(max)));
                    let (global, source) = range_for(info.path, ranges, default, catalogue);
                    Kind::Continuous {
                        default,
                        catalogue,
                        global,
                        source,
                    }
                }
                (None, ParamKind::Choice(choices)) => {
                    let current = defaults.get_path(info.path)?;
                    let default = choices
                        .iter()
                        .position(|c| current.as_str() == Some(c.id))
                        .unwrap_or(0);
                    Kind::Categorical {
                        choices,
                        default,
                        drawn: ranges != Ranges::Plausible,
                    }
                }
                _ => unreachable!("filtered to ranges and choices"),
            };
            Ok(Varied { info, kind })
        })
        .collect()
}

/// The f64 written like the f32 nearest to `x` in its shortest form, so an
/// f32 setting such as 0.05 reads 0.05, not 0.05000000074505806.
pub fn shortest(x: f64) -> f64 {
    (x as f32).to_string().parse().unwrap_or(x)
}

/// One sampled value: a number, or a choice's index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A continuous assumption's value.
    Number(f64),
    /// A categorical assumption's choice, by index.
    Choice(usize),
}

/// The sampling plan of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// Multiplicative jitter around the defaults.
    Local {
        /// Half-width of the factor's uniform range.
        jitter: f64,
    },
    /// A Latin hypercube over the ranges to explore.
    Global,
}

/// `worlds` worlds from `sample_seed`, each a value per varied assumption in
/// the order of `varied`.
pub fn sample(varied: &[Varied], plan: Plan, worlds: usize, sample_seed: u64) -> Vec<Vec<Value>> {
    let mut rng = Pcg64::seed_from_u64(sample_seed);
    match plan {
        Plan::Local { jitter } => (0..worlds)
            .map(|_| {
                varied
                    .iter()
                    .map(|v| {
                        // One draw per assumption, used or not, so the stream
                        // does not depend on which defaults are zero.
                        let u: f64 = rng.random();
                        match v.kind {
                            Kind::Continuous {
                                default, catalogue, ..
                            } => {
                                if default == 0.0 {
                                    Value::Number(default)
                                } else {
                                    let factor = 1.0 - jitter + 2.0 * jitter * u;
                                    Value::Number(
                                        (default * factor).clamp(catalogue.0, catalogue.1),
                                    )
                                }
                            }
                            Kind::Categorical { default, .. } => Value::Choice(default),
                        }
                    })
                    .collect()
            })
            .collect(),
        Plan::Global => {
            let columns: Vec<Vec<Value>> = varied
                .iter()
                .map(|v| match v.kind {
                    Kind::Continuous { global, .. } => {
                        let mut strata: Vec<usize> = (0..worlds).collect();
                        for i in (1..worlds).rev() {
                            strata.swap(i, rng.random_range(0..=i));
                        }
                        strata
                            .into_iter()
                            .map(|s| {
                                let u: f64 = rng.random();
                                let t = (s as f64 + u) / worlds as f64;
                                Value::Number(global.0 + t * (global.1 - global.0))
                            })
                            .collect()
                    }
                    Kind::Categorical {
                        choices,
                        default,
                        drawn,
                    } => (0..worlds)
                        .map(|_| {
                            // One draw per world, used or not, as for a
                            // continuous range of one value.
                            let c = rng.random_range(0..choices.len());
                            Value::Choice(if drawn { c } else { default })
                        })
                        .collect(),
                })
                .collect();
            (0..worlds)
                .map(|w| columns.iter().map(|c| c[w]).collect())
                .collect()
        }
    }
}

/// The default configuration with a world's values set.
pub fn world_config(varied: &[Varied], values: &[Value]) -> Result<Config, ConfigError> {
    let mut config = Config::default();
    for (v, value) in varied.iter().zip(values) {
        match (*value, v.kind) {
            (Value::Number(x), _) => config.set_path(v.info.path, x)?,
            (Value::Choice(i), Kind::Categorical { choices, .. }) => {
                config.set_path(v.info.path, FieldValue::Text(choices[i].id.to_string()))?;
            }
            (Value::Choice(_), Kind::Continuous { .. }) => {
                return Err(ConfigError::InvalidValue {
                    path: v.info.path.to_string(),
                    message: "a choice for a number".into(),
                });
            }
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(v: Value) -> f64 {
        match v {
            Value::Number(x) => x,
            Value::Choice(i) => i as f64,
        }
    }

    fn all(ranges: Ranges) -> Vec<Varied> {
        varied(ranges).expect("defaults resolve")
    }

    fn global_of(v: &[Varied], path: &str) -> Option<((f64, f64), RangeSource)> {
        v.iter()
            .find(|x| x.info.path == path)
            .and_then(|x| match x.kind {
                Kind::Continuous { global, source, .. } => Some((global, source)),
                Kind::Categorical { .. } => None,
            })
    }

    #[test]
    fn the_varied_set_excludes_the_checking_habits_and_adds_two_population_settings() {
        for ranges in [Ranges::Documented, Ranges::Revised, Ranges::Plausible] {
            let v = all(ranges);
            assert_eq!(v.len(), 54);
            for path in EXCLUDED {
                assert!(v.iter().all(|x| x.info.path != path), "{path}");
            }
            let population: Vec<&str> = v
                .iter()
                .filter(|x| !x.info.path.starts_with("world."))
                .map(|x| x.info.path)
                .collect();
            assert_eq!(population, POPULATION);
            let categorical: Vec<&str> = v
                .iter()
                .filter(|x| matches!(x.kind, Kind::Categorical { .. }))
                .map(|x| x.info.path)
                .collect();
            assert_eq!(categorical, ["world.bot_strategy"]);
            let not_jittered: Vec<&str> = v
                .iter()
                .filter(|x| matches!(x.kind, Kind::Continuous { .. }) && !x.jittered())
                .map(|x| x.info.path)
                .collect();
            assert_eq!(
                not_jittered,
                ["world.leave_threshold", "world.polarisation_backfire"]
            );
        }
    }

    #[test]
    fn local_values_lie_within_the_jitter_and_the_ranges() {
        let v = all(Ranges::Revised);
        let worlds = sample(&v, Plan::Local { jitter: 0.25 }, 200, 5);
        for world in &worlds {
            for (x, value) in v.iter().zip(world) {
                match x.kind {
                    Kind::Continuous {
                        default, catalogue, ..
                    } => {
                        let value = number(*value);
                        assert!(value >= catalogue.0 && value <= catalogue.1);
                        if default == 0.0 {
                            assert_eq!(value, 0.0, "{}", x.info.path);
                        } else {
                            let (a, b) = (default * 0.75, default * 1.25);
                            let (lo, hi) = (a.min(b).max(catalogue.0), a.max(b).min(catalogue.1));
                            assert!(value >= lo - 1e-12 && value <= hi + 1e-12);
                        }
                    }
                    Kind::Categorical { default, .. } => {
                        assert_eq!(*value, Value::Choice(default));
                    }
                }
            }
        }
        // The draws use the whole jitter: some factor below 0.8, some above 1.2.
        for (path, default) in [
            ("world.outrage_appeal", 1.2),
            ("population.archetype_shares.troll", 0.03),
            ("population.bot_share", 0.05),
        ] {
            let i = v.iter().position(|x| x.info.path == path).expect("listed");
            let factors: Vec<f64> = worlds.iter().map(|w| number(w[i]) / default).collect();
            assert!(
                factors.iter().any(|f| *f < 0.8) && factors.iter().any(|f| *f > 1.2),
                "{path}"
            );
        }
        // Zero jitter is the default world.
        let still = sample(&v, Plan::Local { jitter: 0.0 }, 2, 5);
        let config = world_config(&v, &still[0]).expect("valid");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn global_draws_fill_every_stratum_once() {
        for ranges in [Ranges::Documented, Ranges::Revised] {
            let v = all(ranges);
            let k = 37;
            let worlds = sample(&v, Plan::Global, k, 9);
            for (j, x) in v.iter().enumerate() {
                let Kind::Continuous { global, .. } = x.kind else {
                    continue;
                };
                if !x.spans() {
                    // A range of one value: every world holds it.
                    assert!(worlds.iter().all(|w| number(w[j]) == global.0));
                    continue;
                }
                let mut seen = vec![0; k];
                for w in &worlds {
                    let t = (number(w[j]) - global.0) / (global.1 - global.0);
                    assert!((0.0..=1.0).contains(&t), "{}", x.info.path);
                    seen[((t * k as f64) as usize).min(k - 1)] += 1;
                }
                assert!(seen.iter().all(|&n| n == 1), "{}: {seen:?}", x.info.path);
            }
            let i = v
                .iter()
                .position(|x| x.info.path == "world.bot_strategy")
                .expect("listed");
            let mut counts = [0; 3];
            for w in &sample(&v, Plan::Global, 300, 9) {
                if let Value::Choice(c) = w[i] {
                    counts[c] += 1;
                }
            }
            assert!(counts.iter().all(|&n| n > 70), "{counts:?}");
        }
    }

    #[test]
    fn global_ranges_name_their_source() {
        let documented = all(Ranges::Documented);
        assert_eq!(
            global_of(&documented, "world.outrage_appeal"),
            Some(((0.0, 3.0), RangeSource::Docs))
        );
        let (range, src) = global_of(&documented, "world.polarisation_rate").expect("continuous");
        assert_eq!(src, RangeSource::DocsClipped);
        assert!((range.1 - 0.002).abs() < 1e-9);
        assert_eq!(
            global_of(&documented, "world.ad_rate").map(|s| s.1),
            Some(RangeSource::Catalogue)
        );
        let revised = all(Ranges::Revised);
        assert_eq!(
            global_of(&revised, "world.outrage_appeal"),
            Some(((0.0, 3.0), RangeSource::Docs))
        );
        assert_eq!(
            global_of(&revised, "world.polarisation_rate"),
            Some(((0.0, 0.00075), RangeSource::Scaled))
        );
        let (range, src) = global_of(&revised, "world.ad_rate").expect("continuous");
        assert_eq!(src, RangeSource::DefaultThirds);
        assert!((range.0 - 0.001 / 3.0).abs() < 1e-9 && (range.1 - 0.003).abs() < 1e-9);
        // living_income: 0.025 / 3 to 3 x 0.025, above the catalogue's floor of 0.005.
        let (range, _) = global_of(&revised, "world.living_income").expect("continuous");
        assert!((range.0 - 0.025 / 3.0).abs() < 1e-6 && (range.1 - 0.075).abs() < 1e-6);
        // Judgement ranges replace the rule's (lab #35).
        assert_eq!(
            global_of(&revised, "world.leave_threshold"),
            Some(((-0.1, 0.1), RangeSource::Judgement))
        );
        assert_eq!(
            global_of(&revised, "world.outgroup_like_cut"),
            Some(((0.0, 1.0), RangeSource::Judgement))
        );
        // exposure_contagion: off to twice its default (lab #38).
        assert_eq!(
            global_of(&revised, "world.exposure_contagion"),
            Some(((0.0, 0.56), RangeSource::Judgement))
        );
    }

    /// `base_session_items` is held at its default in every mode and range
    /// set, as the checking habits are; `leave_threshold` varies in global
    /// mode only: -0.1 to 0.1 revised, -0.05 to 0.05 plausible, and stays at
    /// its default of 0 in local mode.
    #[test]
    fn session_length_is_held_and_the_leave_threshold_has_judgement_ranges() {
        assert!(EXCLUDED.contains(&"world.base_session_items"));
        assert!(!EXCLUDED.contains(&"world.outgroup_like_cut"));
        for ranges in [Ranges::Documented, Ranges::Revised, Ranges::Plausible] {
            let v = all(ranges);
            assert!(v.iter().all(|x| x.info.path != "world.base_session_items"));
            assert!(v.iter().any(|x| x.info.path == "world.outgroup_like_cut"));
            for plan in [Plan::Local { jitter: 0.25 }, Plan::Global] {
                for world in sample(&v, plan, 16, 7) {
                    let c = world_config(&v, &world).expect("valid");
                    assert_eq!(
                        c.world.base_session_items,
                        Config::default().world.base_session_items
                    );
                }
            }
        }
        assert_eq!(default_of("world.leave_threshold"), 0.0);
        let expected = [
            (Ranges::Documented, (-0.5, 0.5)),
            (Ranges::Revised, (-0.1, 0.1)),
            (Ranges::Plausible, (-0.05, 0.05)),
        ];
        for (ranges, range) in expected {
            let v = all(ranges);
            assert_eq!(
                global_of(&v, "world.leave_threshold").map(|r| r.0),
                Some(range)
            );
            let i = v
                .iter()
                .position(|x| x.info.path == "world.leave_threshold")
                .expect("listed");
            let local = sample(&v, Plan::Local { jitter: 0.25 }, 30, 2);
            assert!(local.iter().all(|w| w[i] == Value::Number(0.0)));
            let global: Vec<f64> = sample(&v, Plan::Global, 30, 2)
                .iter()
                .map(|w| number(w[i]))
                .collect();
            assert!(global.iter().all(|x| *x >= range.0 && *x <= range.1));
            assert!(global.iter().any(|x| *x < 0.0) && global.iter().any(|x| *x > 0.0));
        }
    }

    /// Every varied number has exactly one plausible rule, in catalogue
    /// order; shares lie on a scale of 0 to 1, the leave threshold and the
    /// population settings have their own rules, and every other number is a
    /// scale with a nonzero default.
    #[test]
    fn every_varied_number_has_a_plausible_rule() {
        let v = all(Ranges::Plausible);
        let continuous: Vec<&str> = v
            .iter()
            .filter(|x| matches!(x.kind, Kind::Continuous { .. }))
            .map(|x| x.info.path)
            .collect();
        let listed: Vec<&str> = PLAUSIBLE_RULES.iter().map(|(p, _)| *p).collect();
        assert_eq!(listed, continuous);
        for x in &v {
            match x.kind {
                Kind::Continuous {
                    default,
                    catalogue,
                    source,
                    ..
                } => {
                    let rule = plausible_rule(x.info.path).expect("classified");
                    assert_eq!(source, RangeSource::Plausible(rule), "{}", x.info.path);
                    match rule {
                        PlausibleRule::Share => assert!(
                            catalogue.0 >= 0.0
                                && catalogue.1 <= 1.0
                                && (0.0..=1.0).contains(&default),
                            "{}",
                            x.info.path
                        ),
                        PlausibleRule::Scale => assert!(default != 0.0, "{}", x.info.path),
                        PlausibleRule::LeaveThreshold => {
                            assert_eq!(x.info.path, "world.leave_threshold");
                        }
                        PlausibleRule::Population => {
                            assert!(POPULATION.contains(&x.info.path));
                        }
                    }
                }
                Kind::Categorical { drawn, .. } => {
                    assert!(!drawn, "the plausible ranges hold {}", x.info.path);
                    assert!(!x.spans());
                }
            }
        }
        let count = |r: PlausibleRule| PLAUSIBLE_RULES.iter().filter(|(_, x)| *x == r).count();
        assert_eq!(count(PlausibleRule::LeaveThreshold), 1);
        assert_eq!(count(PlausibleRule::Population), POPULATION.len());
        assert_eq!(count(PlausibleRule::Share), 10);
        assert_eq!(count(PlausibleRule::Scale), 40);
        // Unlisted paths fall back to the default alone.
        assert_eq!(
            range_for("world.nothing", Ranges::Plausible, 2.0, (0.0, 5.0)),
            ((2.0, 2.0), RangeSource::Unclassified)
        );
    }

    /// The plausible rule on examples of each class, with the clamps.
    #[test]
    fn plausible_ranges_follow_their_rule() {
        let v = all(Ranges::Plausible);
        let range = |path: &str| global_of(&v, path).expect("continuous").0;
        let close = |a: (f64, f64), b: (f64, f64)| {
            assert!(
                (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6,
                "{a:?} against {b:?}"
            );
        };
        // Scales: default/1.5 to 1.5 x default.
        close(range("world.outrage_appeal"), (0.8, 1.8));
        close(range("world.time_cost"), (0.035 / 1.5, 0.0525));
        close(range("world.exposure_contagion"), (0.28 / 1.5, 0.42));
        // A negative scale keeps its order.
        close(range("world.stop_bias"), (-3.75, -2.5 / 1.5));
        // Shares: default +- 0.15, clamped to [0, 1] and the catalogue.
        close(range("world.social_proof"), (0.15, 0.45));
        close(range("world.survey_habit_weight"), (0.0, 0.25));
        close(range("world.polarisation_backfire"), (0.0, 0.15));
        close(range("world.outgroup_like_cut"), (0.65, 0.95));
        close(range("world.homophily"), (0.55, 0.85));
        // Population settings: default/1.5 to 1.5 x default.
        close(range("population.archetype_shares.troll"), (0.02, 0.045));
        close(range("population.bot_share"), (0.05 / 1.5, 0.075));
        close(range("world.leave_threshold"), (-0.05, 0.05));
        // Catalogue clamps apply to every rule.
        assert_eq!(
            range_for("world.social_proof", Ranges::Plausible, 0.95, (0.0, 1.0)).0,
            (0.8, 1.0)
        );
        assert_eq!(
            range_for("world.learning_rate", Ranges::Plausible, 0.4, (0.0, 0.5)).0,
            (shortest(0.4 / 1.5), 0.5)
        );
        // Global draws stay inside, and bot_strategy keeps its default.
        let i = v
            .iter()
            .position(|x| x.info.path == "world.bot_strategy")
            .expect("listed");
        let Kind::Categorical { default, .. } = v[i].kind else {
            panic!("categorical");
        };
        for world in sample(&v, Plan::Global, 40, 3) {
            assert_eq!(world[i], Value::Choice(default));
            for (x, value) in v.iter().zip(&world) {
                if let Kind::Continuous { global, .. } = x.kind {
                    let value = number(*value);
                    assert!(value >= global.0 && value <= global.1, "{}", x.info.path);
                }
            }
            assert!(world_config(&v, &world).expect("valid").validate().is_ok());
        }
    }

    /// The population settings: default/2 to 2 x default in global mode,
    /// whatever the ranges; only the troll share among the archetypes'
    /// shares changes, which the population normalises when it draws.
    #[test]
    fn population_settings_vary_around_their_defaults() {
        for ranges in [Ranges::Documented, Ranges::Revised] {
            let v = all(ranges);
            assert_eq!(
                global_of(&v, "population.archetype_shares.troll"),
                Some(((0.015, 0.06), RangeSource::Population))
            );
            assert_eq!(
                global_of(&v, "population.bot_share"),
                Some(((0.025, 0.1), RangeSource::Population))
            );
            let defaults = Config::default();
            for world in sample(&v, Plan::Global, 20, 4) {
                let c = world_config(&v, &world).expect("valid");
                assert!(c.validate().is_ok());
                let (a, d) = (
                    &c.population.archetype_shares,
                    &defaults.population.archetype_shares,
                );
                assert!(a.troll >= 0.015 && a.troll <= 0.06);
                assert_eq!(
                    (a.casual, a.creator, a.partisan, a.vulnerable),
                    (d.casual, d.creator, d.partisan, d.vulnerable)
                );
                assert!(c.population.bot_share >= 0.025 && c.population.bot_share <= 0.1);
                assert_eq!(c.population.bot_boosts, defaults.population.bot_boosts);
            }
        }
    }

    #[test]
    fn worlds_are_reproducible_and_leave_the_rest_untouched() {
        let v = all(Ranges::Revised);
        for plan in [Plan::Local { jitter: 0.25 }, Plan::Global] {
            let a = sample(&v, plan, 12, 3);
            assert_eq!(a, sample(&v, plan, 12, 3));
            assert_ne!(a, sample(&v, plan, 12, 4));
            let defaults = Config::default();
            for world in &a {
                let config = world_config(&v, world).expect("every sampled world is valid");
                assert_eq!(config.run, defaults.run);
                assert_eq!(config.design, defaults.design);
                let mut population = config.population.clone();
                population.archetype_shares.troll = defaults.population.archetype_shares.troll;
                population.bot_share = defaults.population.bot_share;
                assert_eq!(population, defaults.population);
                for path in EXCLUDED {
                    assert_eq!(config.get_path(path).ok(), defaults.get_path(path).ok());
                }
            }
        }
    }

    /// Section 11's table: assumption names, and the Default, Range and
    /// Plausible cells, by row.
    #[allow(clippy::type_complexity)]
    fn section_11() -> Vec<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
        let text = include_str!("../../../../docs/model.md");
        let section = text
            .split("## 11. World assumptions")
            .nth(1)
            .and_then(|s| s.split("\n## ").next())
            .expect("section 11");
        section
            .lines()
            .filter(|l| l.starts_with("| `"))
            .map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                let names = cells[1]
                    .split(',')
                    .filter_map(|n| n.split('`').nth(1).map(|n| format!("world.{n}")))
                    .collect();
                let split = |c: &str| c.split(", ").map(str::to_string).collect();
                (names, split(cells[2]), split(cells[3]), split(cells[4]))
            })
            .collect()
    }

    /// A range cell `a-b`, where `a` may be negative.
    fn parse_range(r: &str) -> Option<(f64, f64)> {
        let cut = r.get(1..)?.find('-')? + 1;
        Some((r[..cut].parse().ok()?, r[cut + 1..].parse().ok()?))
    }

    fn catalogue_of(path: &str) -> (f64, f64) {
        params()
            .iter()
            .find(|p| p.path == path)
            .and_then(|p| match p.kind {
                ParamKind::Range { min, max, .. } => {
                    Some((shortest(f64::from(min)), shortest(f64::from(max))))
                }
                _ => None,
            })
            .expect("a range in the catalogue")
    }

    fn default_of(path: &str) -> f64 {
        shortest(
            Config::default()
                .get_number(path)
                .ok()
                .flatten()
                .expect("a number"),
        )
    }

    /// Section 11 lists the revised ranges: each one matches the rule, so
    /// the table's ranges are the ones the revised sampling uses.
    #[test]
    fn section_11_lists_the_revised_ranges() {
        let mut parsed = 0;
        for (names, _, ranges, _) in section_11() {
            if ranges.len() != names.len() {
                continue;
            }
            for (path, cell) in names.iter().zip(&ranges) {
                let Some((lo, hi)) = parse_range(cell) else {
                    continue;
                };
                let (expected, _) =
                    range_for(path, Ranges::Revised, default_of(path), catalogue_of(path));
                let close = |a: f64, b: f64| (a - b).abs() <= 1e-3 * b.abs().max(1e-6);
                assert!(
                    close(lo, expected.0) && close(hi, expected.1),
                    "{path}: table {lo}-{hi}, rule {expected:?}"
                );
                parsed += 1;
            }
        }
        // The 50 documented ranges, the four the rule adds, and the
        // judgement ranges of outgroup_like_cut and exposure_contagion.
        assert_eq!(parsed, DOCUMENTED_RANGES.len() + 6);
    }

    /// Section 11's Plausible column lists the plausible ranges: each one
    /// matches the rule; the held assumptions read "held" and `bot_strategy`
    /// its default, the only choice the plausible ranges sample.
    #[test]
    fn section_11_lists_the_plausible_ranges() {
        let v = all(Ranges::Plausible);
        let mut parsed = 0;
        let mut rows = 0;
        for (names, _, _, cells) in section_11() {
            rows += 1;
            assert_eq!(cells.len(), names.len(), "{names:?}: {cells:?}");
            for (path, cell) in names.iter().zip(&cells) {
                if EXCLUDED.contains(&path.as_str()) {
                    assert_eq!(cell, "held", "{path}");
                    continue;
                }
                let x = v
                    .iter()
                    .find(|x| x.info.path == path)
                    .unwrap_or_else(|| panic!("{path} varies"));
                match x.kind {
                    Kind::Categorical {
                        choices, default, ..
                    } => assert_eq!(cell, choices[default].id, "{path}"),
                    Kind::Continuous { global, .. } => {
                        let (lo, hi) = parse_range(cell)
                            .unwrap_or_else(|| panic!("{path}: a range, not {cell}"));
                        let close = |a: f64, b: f64| (a - b).abs() <= 1e-3 * b.abs().max(1e-6);
                        assert!(
                            close(lo, global.0) && close(hi, global.1),
                            "{path}: table {lo}-{hi}, rule {global:?}"
                        );
                        parsed += 1;
                    }
                }
            }
        }
        assert!(rows > 20);
        // Every varied number except the population settings, which
        // section 11 does not list.
        assert_eq!(parsed, PLAUSIBLE_RULES.len() - POPULATION.len());
    }

    /// Each scaled range against its 13.1 factor: the text defaults are
    /// section 11's Default column and 13.1's Text column, the code defaults
    /// 13.1's Code column, and the revised range is the documented one times
    /// code over text; every other numeric default in section 11 is the
    /// code's.
    #[test]
    fn scaled_ranges_follow_their_13_1_factor() {
        let text = include_str!("../../../../docs/model.md");
        let table_13_1 = text
            .split("### 13.1 Values that differ from the text")
            .nth(1)
            .and_then(|s| s.split("\n### ").next())
            .expect("13.1");
        let mut text_defaults = Vec::new();
        for (names, defaults, ..) in section_11() {
            for (path, cell) in names.iter().zip(&defaults) {
                let Ok(listed) = cell.parse::<f64>() else {
                    continue;
                };
                let code = default_of(path);
                if (listed - code).abs() > 1e-9 {
                    text_defaults.push((path.clone(), listed));
                }
            }
        }
        let mut expected: Vec<(String, f64)> = TEXT_DEFAULTS
            .iter()
            .map(|(p, d)| (p.to_string(), *d))
            .collect();
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        text_defaults.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(text_defaults, expected);
        for (path, text_default) in TEXT_DEFAULTS {
            let name = path.trim_start_matches("world.");
            let cells: Vec<&str> = table_13_1
                .lines()
                .filter(|l| l.starts_with("| "))
                .map(|l| l.split('|').map(str::trim).collect::<Vec<_>>())
                .find(|c| c.len() > 4 && c[2].starts_with(&format!("`{name}`")))
                .unwrap_or_else(|| panic!("13.1 records {name}"));
            assert_eq!(cells[3].parse::<f64>().ok(), Some(text_default), "{name}");
            let code = default_of(path);
            assert_eq!(cells[4].parse::<f64>().ok(), Some(code), "{name}");
            let catalogue = catalogue_of(path);
            let (revised, source) = range_for(path, Ranges::Revised, code, catalogue);
            if JUDGEMENT_RANGES.iter().any(|(p, ..)| *p == path) {
                assert_eq!(source, RangeSource::Judgement, "{name}");
                continue;
            }
            match DOCUMENTED_RANGES.iter().find(|(p, ..)| *p == path) {
                Some(&(_, lo, hi)) => {
                    let f = code / text_default;
                    assert_eq!(source, RangeSource::Scaled, "{name}");
                    assert!(
                        (revised.0 - (lo * f).max(catalogue.0)).abs() < 1e-9,
                        "{name}"
                    );
                    assert!(
                        (revised.1 - (hi * f).min(catalogue.1)).abs() < 1e-9,
                        "{name}"
                    );
                }
                None => assert_eq!(source, RangeSource::DefaultThirds, "{name}"),
            }
        }
    }

    /// The note under section 11's table names the rule and the old value of
    /// every range the revision changed.
    #[test]
    fn the_revision_note_names_the_old_ranges() {
        let text = include_str!("../../../../docs/model.md");
        let note: String = text
            .split("Ranges revised")
            .nth(1)
            .expect("the revision note")
            .split("\n\n")
            .next()
            .unwrap_or_default()
            .replace('\n', " ");
        assert!(note.contains("default/3 to 3 x default") && note.contains("13.1"));
        for (path, _) in TEXT_DEFAULTS {
            let name = path.trim_start_matches("world.");
            match DOCUMENTED_RANGES.iter().find(|(p, ..)| *p == path) {
                Some(&(_, lo, hi)) => {
                    let old = format!("`{name}` {lo}-{hi}");
                    assert!(note.contains(&old), "the note names {old}");
                }
                None => assert!(note.contains(&format!("`{name}`")), "{name}"),
            }
        }
    }
}
