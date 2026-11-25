//! Metadata of every design lever and world assumption, for user interfaces
//! and for range validation. The app builds its controls from [`params`].

use serde_json::Value;

use crate::config::Config;
use crate::error::ConfigError;

mod catalogue;

use catalogue::PARAMS;

/// Whether a parameter is the platform's choice or an assumption about people.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamSide {
    /// A design lever: what the platform chooses.
    Design,
    /// A world assumption: how people are assumed to behave, or who they are.
    World,
}

/// The control group a parameter belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamGroup {
    /// Feed ranking.
    Ranking,
    /// What users see of others' numbers.
    Visibility,
    /// How far posts spread.
    Virality,
    /// Classifiers, reviewers and sanctions.
    Moderation,
    /// Context added to posts.
    Context,
    /// Paying authors.
    CreatorMoney,
    /// Controls in users' hands.
    UserControls,
    /// Cost of accounts.
    Identity,
    /// How the platform is funded.
    Business,
    /// What draws attention.
    Appeal,
    /// What hurts people.
    Harm,
    /// How authors adapt.
    Learning,
    /// Habits and sessions.
    Habit,
    /// Leaving and joining.
    Membership,
    /// Attitudes and beliefs across society.
    Society,
    /// Money in the world.
    Economy,
    /// Who lives in the society.
    Population,
}

impl ParamGroup {
    /// Display name of the group.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ranking => "Ranking",
            Self::Visibility => "Visibility",
            Self::Virality => "Virality",
            Self::Moderation => "Moderation",
            Self::Context => "Context",
            Self::CreatorMoney => "Creator money",
            Self::UserControls => "User controls",
            Self::Identity => "Identity",
            Self::Business => "Business model",
            Self::Appeal => "Attention",
            Self::Harm => "Harm",
            Self::Learning => "Learning",
            Self::Habit => "Habit and sessions",
            Self::Membership => "Joining and leaving",
            Self::Society => "Society",
            Self::Economy => "Economy",
            Self::Population => "Population",
        }
    }
}

/// One choice of an enum parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoiceInfo {
    /// The value as written in a configuration.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
}

/// The kind of control a parameter needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    /// A number in `[min, max]`, moved in steps of `step`.
    Range {
        /// Smallest value.
        min: f32,
        /// Largest value.
        max: f32,
        /// Suggested step of a slider.
        step: f32,
    },
    /// A whole number in `[min, max]`.
    Integer {
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// A whole number in `[min, max]`, or no value (no limit).
    OptionalInteger {
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// On or off.
    Toggle,
    /// One of a fixed set of choices.
    Choice(&'static [ChoiceInfo]),
}

/// Metadata of one design lever or world assumption.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamInfo {
    /// Dotted configuration path, for [`Config::set_path`].
    pub path: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One plain-language sentence: what it is and its real-world counterpart.
    pub description: &'static str,
    /// Design lever or world assumption.
    pub side: ParamSide,
    /// Control group.
    pub group: ParamGroup,
    /// Control kind and range.
    pub kind: ParamKind,
}

/// Every design lever and world assumption, in display order.
pub fn params() -> &'static [ParamInfo] {
    &PARAMS
}

/// The metadata of one parameter by path.
pub fn param(path: &str) -> Option<&'static ParamInfo> {
    PARAMS.iter().find(|p| p.path == path)
}

/// Checks every numeric parameter of `config` against its range.
pub(crate) fn validate_ranges(config: &Config) -> Result<(), ConfigError> {
    let tree = serde_json::to_value(config).map_err(|e| ConfigError::Json(e.to_string()))?;
    PARAMS.iter().try_for_each(|p| check_param(p, &tree))
}

fn check_param(info: &ParamInfo, tree: &Value) -> Result<(), ConfigError> {
    let value = info
        .path
        .split('.')
        .try_fold(tree, |node, key| node.get(key))
        .ok_or_else(|| ConfigError::UnknownPath(info.path.to_string()))?;
    let (min, max) = match info.kind {
        ParamKind::Range { min, max, .. } => (f64::from(min), f64::from(max)),
        ParamKind::Integer { min, max } | ParamKind::OptionalInteger { min, max } => {
            (f64::from(min), f64::from(max))
        }
        ParamKind::Toggle | ParamKind::Choice(_) => return Ok(()),
    };
    let Some(x) = value.as_f64() else {
        return if value.is_null() && matches!(info.kind, ParamKind::OptionalInteger { .. }) {
            Ok(())
        } else {
            Err(ConfigError::InvalidValue {
                path: info.path.to_string(),
                message: "expected a number".into(),
            })
        };
    };
    let tolerance = 1e-6 * (max - min).abs().max(1.0);
    if x.is_finite() && x >= min - tolerance && x <= max + tolerance {
        Ok(())
    } else {
        Err(ConfigError::OutOfRange {
            path: info.path.to_string(),
            value: x,
            min,
            max,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_resolves_and_defaults_validate() {
        let config = Config::default();
        for p in params() {
            assert!(config.get_path(p.path).is_ok(), "{}", p.path);
            assert!(!p.description.is_empty());
        }
        assert!(validate_ranges(&config).is_ok());
    }

    #[test]
    fn every_design_lever_is_listed() {
        let tree = serde_json::to_value(Config::default()).unwrap_or_default();
        let design = tree
            .get("design")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let world = tree
            .get("world")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for key in design.keys() {
            assert!(param(&format!("design.{key}")).is_some(), "design.{key}");
        }
        for key in world.keys() {
            assert!(param(&format!("world.{key}")).is_some(), "world.{key}");
        }
    }
}
