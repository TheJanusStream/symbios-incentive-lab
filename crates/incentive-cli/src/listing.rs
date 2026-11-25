//! The listing commands: presets, scenarios, levers and a preset's JSON.

use incentive_core::{ParamKind, ParamSide, params, preset, presets, scenarios};

use crate::error::CliError;

/// Prints the presets with their descriptions.
pub fn list_presets() -> Result<(), CliError> {
    for p in presets() {
        outln!("{} ({})", p.name, p.id);
        outln!("  {}\n", p.description);
    }
    Ok(())
}

/// Prints the scenarios with their briefings and goals.
pub fn list_scenarios() -> Result<(), CliError> {
    for s in scenarios() {
        let days = s.config().run.days;
        outln!("{} ({}, {days} days)", s.title, s.id);
        outln!("  {}", s.briefing);
        for g in s.goals {
            outln!("  - {}", g.description);
        }
        outln!("");
    }
    Ok(())
}

/// Prints every lever and assumption with its current default and range.
pub fn list_levers() -> Result<(), CliError> {
    let defaults = incentive_core::Config::default();
    let mut side = None;
    let mut group = None;
    for p in params() {
        if side != Some(p.side) {
            side = Some(p.side);
            group = None;
            let title = match p.side {
                ParamSide::Design => "DESIGN LEVERS (what the platform chooses)",
                ParamSide::World => "WORLD ASSUMPTIONS (how people are assumed to behave)",
            };
            outln!("{title}\n");
        }
        if group != Some(p.group) {
            group = Some(p.group);
            outln!("[{}]", p.group.name());
        }
        let current = defaults
            .get_path(p.path)
            .map(|v| v.to_string())
            .unwrap_or_default();
        outln!("  {} = {current}  ({})", p.path, kind_text(&p.kind));
        outln!("    {}: {}", p.name, p.description);
    }
    Ok(())
}

fn kind_text(kind: &ParamKind) -> String {
    match kind {
        ParamKind::Range { min, max, step } => format!("{min} to {max}, step {step}"),
        ParamKind::Integer { min, max } => format!("whole number {min} to {max}"),
        ParamKind::OptionalInteger { min, max } => format!("whole number {min} to {max}, or none"),
        ParamKind::Toggle => "true or false".into(),
        ParamKind::Choice(choices) => choices.iter().map(|c| c.id).collect::<Vec<_>>().join(" | "),
    }
}

/// Prints a preset's configuration as JSON.
pub fn print_config(id: &str) -> Result<(), CliError> {
    outln!("{}", preset(id)?.config().to_json_pretty());
    Ok(())
}
