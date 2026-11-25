//! From parameter metadata to controls: which widget each design lever or
//! world assumption gets, how its value is read from a configuration and
//! written back, and how a design change is described in words.

use incentive_core::{
    ChoiceInfo, Config, ConfigError, DesignLevers, FieldValue, ParamGroup, ParamInfo, ParamKind,
    ParamSide, params,
};
use serde_json::Value;

/// The widget a parameter is edited with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    /// A slider over `[min, max]` in steps of `step`.
    Slider {
        /// Smallest value.
        min: f32,
        /// Largest value.
        max: f32,
        /// Slider step.
        step: f32,
    },
    /// A drag value for a whole number.
    Drag {
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// A checkbox that switches a limit on, plus a drag value for it.
    OptionalDrag {
        /// Smallest value.
        min: u32,
        /// Largest value.
        max: u32,
    },
    /// A checkbox.
    Checkbox,
    /// A combo box.
    Combo(&'static [ChoiceInfo]),
}

/// The control of a parameter kind. Every kind has one, so every parameter
/// of [`params`] gets a widget.
pub fn control_for(kind: ParamKind) -> Control {
    match kind {
        ParamKind::Range { min, max, step } => Control::Slider { min, max, step },
        ParamKind::Integer { min, max } => Control::Drag { min, max },
        ParamKind::OptionalInteger { min, max } => Control::OptionalDrag { min, max },
        ParamKind::Toggle => Control::Checkbox,
        ParamKind::Choice(choices) => Control::Combo(choices),
    }
}

/// The current value of a parameter, typed for its control.
#[derive(Clone, Debug, PartialEq)]
pub enum LeverValue {
    /// A slider value.
    Number(f32),
    /// A whole number.
    Integer(u32),
    /// A whole number or no limit.
    Optional(Option<u32>),
    /// A switch.
    Toggle(bool),
    /// A choice by its id.
    Choice(String),
}

impl LeverValue {
    /// The value as a configuration field value.
    pub fn to_field(&self) -> FieldValue {
        match self {
            Self::Number(x) => FieldValue::from(*x),
            Self::Integer(n) => FieldValue::Number(f64::from(*n)),
            Self::Optional(Some(n)) => FieldValue::Number(f64::from(*n)),
            Self::Optional(None) => FieldValue::None,
            Self::Toggle(b) => FieldValue::Bool(*b),
            // Text, not `FieldValue::parse`: the sanctions choice "none" is a
            // choice id, not an empty optional.
            Self::Choice(id) => FieldValue::Text(id.clone()),
        }
    }

    /// The value for display: numbers with the precision of their step.
    pub fn display(&self, info: &ParamInfo) -> String {
        match self {
            Self::Number(x) => {
                let step = match info.kind {
                    ParamKind::Range { step, .. } => step,
                    _ => 0.01,
                };
                format!("{x:.*}", decimals_for(step))
            }
            Self::Integer(n) => n.to_string(),
            Self::Optional(Some(n)) => n.to_string(),
            Self::Optional(None) => "no limit".into(),
            Self::Toggle(b) => if *b { "on" } else { "off" }.into(),
            Self::Choice(id) => match info.kind {
                ParamKind::Choice(choices) => choices
                    .iter()
                    .find(|c| c.id == id)
                    .map_or_else(|| id.clone(), |c| c.name.to_string()),
                _ => id.clone(),
            },
        }
    }
}

/// Decimals that show a value moved in steps of `step` without noise.
pub fn decimals_for(step: f32) -> usize {
    let step = f64::from(step.abs());
    (0..6)
        .find(|&d| {
            let scaled = step * 10f64.powi(d as i32);
            scaled.round() >= 1.0 && (scaled - scaled.round()).abs() <= 1e-3 * scaled
        })
        .unwrap_or(6)
}

/// A configuration as a JSON tree, built once per frame so every control
/// reads from it instead of serialising the configuration per parameter.
pub fn tree(config: &Config) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

/// Reads a parameter's value from a configuration tree; `None` when the path
/// or the type does not match the parameter's kind.
pub fn read(tree: &Value, info: &ParamInfo) -> Option<LeverValue> {
    let node = info
        .path
        .split('.')
        .try_fold(tree, |node, key| node.get(key))?;
    match info.kind {
        ParamKind::Range { .. } => node.as_f64().map(|x| LeverValue::Number(x as f32)),
        ParamKind::Integer { .. } => node
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(LeverValue::Integer),
        ParamKind::OptionalInteger { .. } => {
            if node.is_null() {
                Some(LeverValue::Optional(None))
            } else {
                node.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .map(|n| LeverValue::Optional(Some(n)))
            }
        }
        ParamKind::Toggle => node.as_bool().map(LeverValue::Toggle),
        ParamKind::Choice(_) => node.as_str().map(|s| LeverValue::Choice(s.to_string())),
    }
}

/// Writes a parameter's value into a configuration, validated; on error the
/// configuration is unchanged.
pub fn write(config: &mut Config, info: &ParamInfo, value: &LeverValue) -> Result<(), ConfigError> {
    config.set_path(info.path, value.to_field())
}

/// The parameters of one side, grouped by control group in display order.
pub fn grouped(side: ParamSide) -> Vec<(ParamGroup, Vec<&'static ParamInfo>)> {
    let mut groups: Vec<(ParamGroup, Vec<&'static ParamInfo>)> = Vec::new();
    for p in params().iter().filter(|p| p.side == side) {
        match groups.iter_mut().find(|(g, _)| *g == p.group) {
            Some((_, list)) => list.push(p),
            None => groups.push((p.group, vec![p])),
        }
    }
    groups
}

/// One line per design lever that differs between `before` and `after`, such
/// as "Reply weight 1.5 -> 0.0".
pub fn describe_design_change(before: &DesignLevers, after: &DesignLevers) -> Vec<String> {
    if before == after {
        return Vec::new();
    }
    let wrap = |design: &DesignLevers| {
        tree(&Config {
            design: design.clone(),
            ..Config::default()
        })
    };
    let (a, b) = (wrap(before), wrap(after));
    params()
        .iter()
        .filter(|p| p.side == ParamSide::Design)
        .filter_map(|p| {
            let (x, y) = (read(&a, p)?, read(&b, p)?);
            (x != y).then(|| format!("{} {} -> {}", p.name, x.display(p), y.display(p)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use incentive_core::preset;

    #[test]
    fn every_parameter_maps_to_a_control_and_round_trips() {
        let mut config = Config::default();
        let t = tree(&config);
        for p in params() {
            let control = control_for(p.kind);
            let value = read(&t, p).unwrap_or_else(|| panic!("{} unreadable", p.path));
            let matches = matches!(
                (control, &value),
                (Control::Slider { .. }, LeverValue::Number(_))
                    | (Control::Drag { .. }, LeverValue::Integer(_))
                    | (Control::OptionalDrag { .. }, LeverValue::Optional(_))
                    | (Control::Checkbox, LeverValue::Toggle(_))
                    | (Control::Combo(_), LeverValue::Choice(_))
            );
            assert!(matches, "{}: {control:?} cannot hold {value:?}", p.path);
            assert!(write(&mut config, p, &value).is_ok(), "{}", p.path);
            assert!(!value.display(p).is_empty());
        }
        assert_eq!(config, Config::default());
    }

    #[test]
    fn every_choice_and_the_extremes_can_be_written() {
        for p in params() {
            let mut config = Config::default();
            match control_for(p.kind) {
                Control::Slider { min, max, .. } => {
                    assert!(write(&mut config, p, &LeverValue::Number(min)).is_ok());
                    assert!(write(&mut config, p, &LeverValue::Number(max)).is_ok());
                }
                Control::Drag { min, max } => {
                    assert!(write(&mut config, p, &LeverValue::Integer(min)).is_ok());
                    assert!(write(&mut config, p, &LeverValue::Integer(max)).is_ok());
                }
                Control::OptionalDrag { max, .. } => {
                    assert!(write(&mut config, p, &LeverValue::Optional(Some(max))).is_ok());
                    assert!(write(&mut config, p, &LeverValue::Optional(None)).is_ok());
                    assert_eq!(read(&tree(&config), p), Some(LeverValue::Optional(None)));
                }
                Control::Checkbox => {
                    assert!(write(&mut config, p, &LeverValue::Toggle(true)).is_ok());
                }
                Control::Combo(choices) => {
                    for c in choices {
                        let value = LeverValue::Choice(c.id.to_string());
                        assert!(write(&mut config, p, &value).is_ok(), "{} {}", p.path, c.id);
                        assert_eq!(read(&tree(&config), p), Some(value));
                    }
                }
            }
        }
    }

    #[test]
    fn groups_cover_each_side_once() {
        let design: usize = grouped(ParamSide::Design)
            .iter()
            .map(|(_, l)| l.len())
            .sum();
        let world: usize = grouped(ParamSide::World).iter().map(|(_, l)| l.len()).sum();
        assert_eq!(design + world, params().len());
        assert_eq!(grouped(ParamSide::Design)[0].0, ParamGroup::Ranking);
    }

    #[test]
    fn design_changes_are_described() {
        let before = DesignLevers::default();
        let mut after = before.clone();
        after.w_reply = 0.0;
        after.reshare_depth_cap = Some(2);
        let lines = describe_design_change(&before, &after);
        assert_eq!(
            lines,
            ["Reply weight 1.5 -> 0.0", "Reshare depth cap no limit -> 2"]
        );
        assert!(describe_design_change(&before, &before).is_empty());
        let qb = preset("quality-bridging")
            .map(|p| p.design())
            .unwrap_or_default();
        assert!(describe_design_change(&before, &qb).len() >= 7);
    }

    #[test]
    fn decimals_follow_the_step() {
        assert_eq!(decimals_for(1.0), 0);
        assert_eq!(decimals_for(0.1), 1);
        assert_eq!(decimals_for(0.05), 2);
        assert_eq!(decimals_for(0.00005), 5);
    }
}
