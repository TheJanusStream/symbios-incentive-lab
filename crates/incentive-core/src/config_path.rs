//! Reading and writing configuration fields by dotted path, for sweeps and for
//! the app's generic controls. The configuration round-trips through a
//! `serde_json::Value`, so every serialised field is reachable.

use serde_json::Value;

use crate::config::Config;
use crate::error::ConfigError;

/// A value for a configuration field, as a sweep or a user interface gives it.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// A number; whole numbers also fit integer fields.
    Number(f64),
    /// A switch.
    Bool(bool),
    /// An enum choice by its kebab-case id.
    Text(String),
    /// No value: clears an optional field, or picks the choice "none" of an
    /// enum field.
    None,
}

impl FieldValue {
    /// Reads a command-line value: `true`/`false`, `none`, a number, or text.
    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        match trimmed {
            "true" => Self::Bool(true),
            "false" => Self::Bool(false),
            "none" | "null" => Self::None,
            _ => trimmed
                .parse::<f64>()
                .map_or_else(|_| Self::Text(trimmed.to_string()), Self::Number),
        }
    }
}

impl From<f64> for FieldValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<f32> for FieldValue {
    fn from(value: f32) -> Self {
        Self::Number(f64::from(value))
    }
}

impl From<bool> for FieldValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl Config {
    /// Sets one field by dotted path, for example `design.w_quality`, and
    /// validates the result. On error the configuration is unchanged.
    pub fn set_path(
        &mut self,
        path: &str,
        value: impl Into<FieldValue>,
    ) -> Result<(), ConfigError> {
        let mut tree =
            serde_json::to_value(&*self).map_err(|e| ConfigError::Json(e.to_string()))?;
        let slot = field_mut(&mut tree, path)?;
        *slot = convert(path, slot, value.into())?;
        let updated: Config =
            serde_json::from_value(tree).map_err(|e| ConfigError::InvalidValue {
                path: path.to_string(),
                message: e.to_string(),
            })?;
        updated.validate()?;
        *self = updated;
        Ok(())
    }

    /// Sets a field from command-line text such as `design.chronological=true`.
    pub fn set_assignment(&mut self, assignment: &str) -> Result<(), ConfigError> {
        let (path, value) =
            assignment
                .split_once('=')
                .ok_or_else(|| ConfigError::InvalidValue {
                    path: assignment.to_string(),
                    message: "expected path=value".into(),
                })?;
        self.set_path(path.trim(), FieldValue::parse(value))
    }

    /// The current value of a field by dotted path.
    pub fn get_path(&self, path: &str) -> Result<Value, ConfigError> {
        let tree = serde_json::to_value(self).map_err(|e| ConfigError::Json(e.to_string()))?;
        let mut node = &tree;
        for key in path.split('.') {
            node = node
                .get(key)
                .ok_or_else(|| ConfigError::UnknownPath(path.to_string()))?;
        }
        if node.is_object() {
            return Err(ConfigError::UnknownPath(path.to_string()));
        }
        Ok(node.clone())
    }

    /// The current value of a numeric or boolean field as a number (booleans
    /// give 0 or 1, an empty optional gives `None`).
    pub fn get_number(&self, path: &str) -> Result<Option<f64>, ConfigError> {
        let value = self.get_path(path)?;
        Ok(match value {
            Value::Number(n) => n.as_f64(),
            Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
            _ => None,
        })
    }
}

fn field_mut<'a>(tree: &'a mut Value, path: &str) -> Result<&'a mut Value, ConfigError> {
    let mut node = tree;
    for key in path.split('.') {
        node = node
            .get_mut(key)
            .ok_or_else(|| ConfigError::UnknownPath(path.to_string()))?;
    }
    if node.is_object() {
        return Err(ConfigError::UnknownPath(path.to_string()));
    }
    Ok(node)
}

fn convert(path: &str, current: &Value, value: FieldValue) -> Result<Value, ConfigError> {
    let mismatch = |message: &str| ConfigError::InvalidValue {
        path: path.to_string(),
        message: message.to_string(),
    };
    match value {
        // "none" is a choice of some enum levers (sanctions); only fields
        // that are not strings, such as `reshare_depth_cap`, are cleared.
        FieldValue::None if current.is_string() => Ok(Value::String("none".into())),
        FieldValue::None => Ok(Value::Null),
        FieldValue::Bool(b) => Ok(Value::Bool(b)),
        FieldValue::Text(text) => Ok(Value::String(text)),
        FieldValue::Number(x) => {
            if !x.is_finite() {
                return Err(mismatch("not a finite number"));
            }
            if current.is_boolean() {
                return Ok(Value::Bool(x != 0.0));
            }
            let integer_field = current.is_u64() || current.is_null();
            if integer_field && x.fract() == 0.0 && x >= 0.0 {
                return Ok(Value::from(x as u64));
            }
            serde_json::Number::from_f64(x)
                .map(Value::Number)
                .ok_or_else(|| mismatch("not a finite number"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_numbers_bools_enums_and_options() {
        let mut config = Config::default();
        assert!(config.set_path("design.w_quality", 1.0).is_ok());
        assert_eq!(config.design.w_quality, 1.0);
        assert!(config.set_assignment("design.chronological=true").is_ok());
        assert!(config.design.chronological);
        assert!(
            config
                .set_assignment("design.payout_scheme=fan-funded")
                .is_ok()
        );
        assert_eq!(
            config.design.payout_scheme,
            crate::config::PayoutScheme::FanFunded
        );
        assert!(config.set_assignment("design.reshare_depth_cap=2").is_ok());
        assert_eq!(config.design.reshare_depth_cap, Some(2));
        assert!(
            config
                .set_assignment("design.reshare_depth_cap=none")
                .is_ok()
        );
        assert_eq!(config.design.reshare_depth_cap, None);
        assert!(config.set_assignment("run.days=30").is_ok());
        assert_eq!(config.run.days, 30);
    }

    #[test]
    fn rejects_unknown_paths_and_bad_values_without_change() {
        let mut config = Config::default();
        let before = config.clone();
        assert!(matches!(
            config.set_path("design.nonsense", 1.0),
            Err(ConfigError::UnknownPath(_))
        ));
        assert!(config.set_path("design", 1.0).is_err());
        assert!(
            config
                .set_assignment("design.payout_scheme=bribes")
                .is_err()
        );
        assert!(config.set_path("design.share_friction", 3.0).is_err());
        assert!(config.set_path("run.days", 1.5).is_err());
        assert_eq!(config, before);
    }

    #[test]
    fn reads_fields_back() {
        let config = Config::default();
        assert_eq!(
            config.get_number("design.w_share").ok().flatten(),
            Some(2.0)
        );
        assert_eq!(
            config.get_number("design.chronological").ok().flatten(),
            Some(0.0)
        );
        assert!(config.get_path("world").is_err());
    }

    #[test]
    fn every_choice_of_every_enum_lever_can_be_set() {
        use crate::params::{ParamKind, params};
        let mut tested = 0;
        for info in params() {
            let ParamKind::Choice(choices) = info.kind else {
                continue;
            };
            for choice in choices {
                let mut config = Config::default();
                let assignment = format!("{}={}", info.path, choice.id);
                assert!(config.set_assignment(&assignment).is_ok(), "{assignment}");
                assert_eq!(
                    config.get_path(info.path).ok(),
                    Some(Value::String(choice.id.to_string())),
                    "{assignment}"
                );
                tested += 1;
            }
        }
        assert!(tested >= 12, "{tested}");
        let mut config = Config::default();
        assert!(config.set_assignment("design.sanctions=none").is_ok());
        assert_eq!(config.design.sanctions, crate::config::Sanctions::None);
        assert!(config.set_assignment("design.reshare_depth_cap=3").is_ok());
        assert!(
            config
                .set_assignment("design.reshare_depth_cap=none")
                .is_ok()
        );
        assert_eq!(config.design.reshare_depth_cap, None);
    }
}
