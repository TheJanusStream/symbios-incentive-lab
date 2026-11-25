//! Typed errors of the public API.

use std::fmt;

/// Why a configuration, or a change to one, was rejected.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigError {
    /// A value lies outside its documented range.
    OutOfRange {
        /// Dotted path of the field.
        path: String,
        /// The rejected value.
        value: f64,
        /// Smallest allowed value.
        min: f64,
        /// Largest allowed value.
        max: f64,
    },
    /// No configuration field has this dotted path.
    UnknownPath(String),
    /// The value has the wrong type for the field, or is not one of its choices.
    InvalidValue {
        /// Dotted path of the field.
        path: String,
        /// What was wrong.
        message: String,
    },
    /// The JSON text could not be read as a configuration.
    Json(String),
    /// A constraint across several fields does not hold.
    Invalid(String),
    /// No preset has this id.
    UnknownPreset(String),
    /// No scenario has this id.
    UnknownScenario(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange {
                path,
                value,
                min,
                max,
            } => write!(f, "{path} = {value} is outside its range [{min}, {max}]"),
            Self::UnknownPath(path) => write!(f, "no configuration field is called {path}"),
            Self::InvalidValue { path, message } => {
                write!(f, "invalid value for {path}: {message}")
            }
            Self::Json(message) => write!(f, "invalid configuration JSON: {message}"),
            Self::Invalid(message) => write!(f, "invalid configuration: {message}"),
            Self::UnknownPreset(id) => write!(f, "no preset is called {id}"),
            Self::UnknownScenario(id) => write!(f, "no scenario is called {id}"),
        }
    }
}

impl std::error::Error for ConfigError {}
