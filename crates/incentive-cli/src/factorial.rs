//! `factorial`: a full 2^k factorial between two designs. Every cell is
//! design B with a subset of the listed levers set to design A's values; all
//! cells run on the same seeds. From the cells come, for each lever, its main
//! effect, its two-way interactions and its Shapley share of the difference
//! between the cell with every listed lever at A's value and B itself, all in
//! the direction from A to B, with 95% intervals over the seeds.

use std::fmt::Write as _;
use std::path::PathBuf;

use incentive_core::{
    Config, FieldValue, MeanInterval, MetricId, ParamSide, Stats, Summary, param,
};
use serde_json::{Value, json};

use crate::cli::RunOptions;
use crate::error::CliError;
use crate::report::{self, MOVING_NOTE, fmt_value, moving_mark, table};
use crate::runs::{self, RunOutput, apply_options, is_moving, parse_seeds};

/// At most this many levers: 64 cells.
pub const MAX_LEVERS: usize = 6;

/// The metrics every cell shows.
const CELL_METRICS: [(&str, MetricId); 5] = [
    ("engagement", MetricId::EngagementIndex),
    ("harm", MetricId::HarmIndex),
    ("benefit", MetricId::BenefitIndex),
    ("toxic", MetricId::ToxicShare),
    ("wrongful", MetricId::FalsePositives),
];

/// The metrics whose effects are decomposed.
const EFFECT_METRICS: [MetricId; 3] = [
    MetricId::EngagementIndex,
    MetricId::HarmIndex,
    MetricId::BenefitIndex,
];

/// The arguments of `factorial`.
pub struct FactorialArgs {
    /// Design A (preset id or configuration file).
    pub a: String,
    /// Design B.
    pub b: String,
    /// The levers, as given.
    pub levers: Vec<String>,
    /// The seeds, as given.
    pub seeds: String,
    /// Days per run.
    pub days: Option<u32>,
    /// Individuals per run.
    pub population: Option<u32>,
    /// Output directory.
    pub out: Option<PathBuf>,
}

/// The effects of the levers on one metric, from a table `y` indexed by cell:
/// bit `j` of the cell index is set when lever `j` has A's value. Every
/// effect is in the direction from A's value to B's.
#[derive(Clone, Debug, PartialEq)]
pub struct Effects {
    /// Main effect of each lever: the mean over the cells of the change when
    /// that lever goes from A's value to B's, the others held.
    pub main: Vec<f64>,
    /// Two-way interaction of each pair `(j, l)`, `j < l`: the change with
    /// both at B's values minus the two changes alone,
    /// `y(both B) - y(j B) - y(l B) + y(neither B)`, averaged over the other
    /// levers (twice the classical +-1-coded interaction).
    pub interactions: Vec<((usize, usize), f64)>,
    /// Shapley value of each lever in the game "levers at B's values": the
    /// values add up to `y(B) - y(every lever at A's value)`.
    pub shapley: Vec<f64>,
}

/// The effects of `k` levers from a table of `2^k` cells.
pub fn effects(y: &[f64], k: usize) -> Effects {
    let full = (1usize << k) - 1;
    let others = |skip: usize| (0..=full).filter(move |m| m & skip == 0);
    let main = (0..k)
        .map(|j| {
            let bit = 1 << j;
            let sum: f64 = others(bit).map(|m| y[m] - y[m | bit]).sum();
            sum / (1usize << (k - 1)) as f64
        })
        .collect();
    let mut interactions = Vec::new();
    for j in 0..k {
        for l in j + 1..k {
            let (bj, bl) = (1 << j, 1 << l);
            let sum: f64 = others(bj | bl)
                .map(|m| y[m] - y[m | bj] - y[m | bl] + y[m | bj | bl])
                .sum();
            interactions.push(((j, l), sum / (1usize << (k - 2)) as f64));
        }
    }
    // The coalition T of levers at B's values is cell `full & !T`.
    let worth = |t: usize| y[full & !t];
    let factorial = |n: usize| (1..=n).map(|i| i as f64).product::<f64>();
    let shapley = (0..k)
        .map(|j| {
            let bit = 1 << j;
            others(bit)
                .map(|t| {
                    let size = t.count_ones() as usize;
                    let weight = factorial(size) * factorial(k - size - 1) / factorial(k);
                    weight * (worth(t | bit) - worth(t))
                })
                .sum()
        })
        .collect();
    Effects {
        main,
        interactions,
        shapley,
    }
}

/// A design lever by name: `w_reply` or `design.w_reply`.
fn lever_path(name: &str) -> Result<String, CliError> {
    let name = name.trim();
    let path = if name.contains('.') {
        name.to_string()
    } else {
        format!("design.{name}")
    };
    match param(&path) {
        Some(p) if p.side == ParamSide::Design => Ok(path),
        _ => Err(CliError::Explore(format!(
            "{name} is not a design lever; see `incentive levers`"
        ))),
    }
}

/// Sets `path` in `to` to its value in `from`.
fn copy_lever(from: &Config, to: &mut Config, path: &str) -> Result<(), CliError> {
    let value = match from.get_path(path)? {
        Value::Number(n) => FieldValue::Number(n.as_f64().unwrap_or_default()),
        Value::Bool(b) => FieldValue::Bool(b),
        Value::String(s) => FieldValue::Text(s),
        _ => FieldValue::None,
    };
    to.set_path(path, value)?;
    Ok(())
}

/// The short name of a lever.
fn short(path: &str) -> &str {
    path.strip_prefix("design.").unwrap_or(path)
}

/// `factorial`: runs the cells and prints their metrics and the effects.
pub fn factorial(args: &FactorialArgs) -> Result<(), CliError> {
    let (label_a, mut a) = runs::load_spec(&args.a)?;
    let (label_b, mut b) = runs::load_spec(&args.b)?;
    let size = RunOptions {
        days: args.days,
        population: args.population,
        seeds: args.seeds.clone(),
        set: Vec::new(),
        metrics: Vec::new(),
        out: None,
    };
    apply_options(&mut a, &size, &[])?;
    apply_options(&mut b, &size, &[])?;
    let (mut run_a, mut run_b) = (a.run.clone(), b.run.clone());
    run_a.seed = 0;
    run_b.seed = 0;
    if run_a != run_b || a.population != b.population || a.world != b.world {
        return Err(CliError::Explore(
            "A and B must share run settings, population and world: only design levers are crossed \
             (pass --days and --population to give both the same size)"
                .into(),
        ));
    }
    let mut levers: Vec<String> = Vec::new();
    for name in &args.levers {
        let path = lever_path(name)?;
        if !levers.contains(&path) {
            levers.push(path);
        }
    }
    if levers.is_empty() || levers.len() > MAX_LEVERS {
        return Err(CliError::Explore(format!(
            "--levers takes 1 to {MAX_LEVERS} design levers"
        )));
    }
    let k = levers.len();
    let seeds = parse_seeds(&args.seeds)?;
    let mut cells = Vec::with_capacity(1 << k);
    for mask in 0..1usize << k {
        let mut config = b.clone();
        for (j, path) in levers.iter().enumerate() {
            if mask & (1 << j) != 0 {
                copy_lever(&a, &mut config, path)?;
            }
        }
        cells.push(config);
    }
    let covers = cells.last().is_some_and(|c| c.design == a.design);
    let jobs: Vec<(Config, u64)> = cells
        .iter()
        .flat_map(|c| seeds.iter().map(move |&s| (c.clone(), s)))
        .collect();
    let outputs = runs::run_many(&jobs)?;
    let by_cell: Vec<&[RunOutput]> = outputs.chunks(seeds.len()).collect();
    outln!(
        "Factorial: B = {label_b} with every subset of {k} levers set to A = {label_a}'s values; {} \
         cells, {} paired seeds, N = {}, {} days\n",
        cells.len(),
        seeds.len(),
        b.run.population_size,
        b.run.days
    );
    for (j, path) in levers.iter().enumerate() {
        let shown = |c: &Config| match c.get_path(path) {
            Ok(Value::Number(n)) => n
                .as_f64()
                .map_or_else(|| n.to_string(), |v| format!("{}", (v * 1e4).round() / 1e4)),
            Ok(Value::Null) => "none".into(),
            Ok(v) => v.to_string().trim_matches('"').to_string(),
            Err(_) => String::new(),
        };
        outln!(
            "  lever {}: {} (A {}, B {})",
            j + 1,
            short(path),
            shown(&a),
            shown(&b)
        );
    }
    outln!("");
    let cell_label = |mask: usize| -> String {
        if mask == 0 {
            return "B".into();
        }
        let names: Vec<&str> = (0..k)
            .filter(|j| mask & (1 << j) != 0)
            .map(|j| short(&levers[j]))
            .collect();
        format!("B with A's {}", names.join(", "))
    };
    let mut rows = Vec::new();
    let mut any_moving = false;
    let mut csv = String::from("cell,levers_from_a");
    for (key, _) in CELL_METRICS {
        let _ = write!(csv, ",{key}_mean,{key}_sd");
    }
    csv.push('\n');
    for (mask, runs) in by_cell.iter().enumerate() {
        let mut row = vec![cell_label(mask)];
        let _ = write!(csv, "{mask},\"{}\"", cell_label(mask));
        for (_, id) in CELL_METRICS {
            let s = Stats::of(runs.iter().map(|r| r.summary.get(id)));
            let moving = is_moving(runs, id);
            any_moving |= moving;
            row.push(format!("{}{}", fmt_value(id, s.mean), moving_mark(moving)));
            let _ = write!(csv, ",{},{}", s.mean, s.sd);
        }
        csv.push('\n');
        rows.push(row);
    }
    let mut header = vec!["cell"];
    header.extend(CELL_METRICS.iter().map(|(k, _)| *k));
    out!("{}", table(&header, &rows));
    outln!("wrongful: removals and demotions of posts that were not toxic, per day");
    if any_moving {
        outln!("{MOVING_NOTE}");
    }
    let mut doc_effects = serde_json::Map::new();
    for id in EFFECT_METRICS {
        let decomposed = decompose(id, &by_cell, seeds.len(), k);
        print_effects(id, &decomposed, &levers)?;
        doc_effects.insert(id.info().key.to_string(), decomposed.to_json(&levers));
    }
    outln!(
        "\nEffects run from A's value to B's: a main effect is the mean change when one lever goes from \
         A's value to B's, an interaction the change with both at B's values minus the two changes \
         alone, and the Shapley values add up to B minus the cell with every listed lever at A's value; \
         ± is the 95% interval over the seeds."
    );
    if !covers {
        outln!(
            "The listed levers are not every difference between A and B, so that cell is not A: the \
             Shapley shares split only the part of the difference these levers explain."
        );
    }
    let dir = args.out.clone().unwrap_or_else(|| {
        report::default_out(&format!(
            "factorial-{}-vs-{}",
            report::slug(&label_a),
            report::slug(&label_b)
        ))
    });
    report::ensure_dir(&dir)?;
    report::write_file(&dir.join("factorial.csv"), &csv)?;
    let cells_doc: Vec<Value> = by_cell
        .iter()
        .enumerate()
        .map(|(mask, runs)| {
            let summaries: Vec<&Summary> = runs.iter().map(|r| &r.summary).collect();
            json!({
                "cell": mask,
                "levers_from_a": (0..k).filter(|j| mask & (1 << j) != 0)
                    .map(|j| levers[j].as_str()).collect::<Vec<_>>(),
                "summaries": summaries,
            })
        })
        .collect();
    let doc = json!({
        "a": {"label": label_a, "config": a},
        "b": {"label": label_b, "config": b},
        "levers": levers,
        "seeds": seeds,
        "covers_every_difference": covers,
        "cells": cells_doc,
        "effects": doc_effects,
    });
    report::write_json(&dir.join("factorial.json"), &doc)?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// The effects of one metric, each with its 95% interval over the seeds.
struct Decomposed {
    /// The cells' table means: B, and every lever at A's value.
    b: f32,
    all_a: f32,
    main: Vec<MeanInterval>,
    interactions: Vec<((usize, usize), MeanInterval)>,
    shapley: Vec<MeanInterval>,
}

impl Decomposed {
    fn to_json(&self, levers: &[String]) -> Value {
        let named = |v: &[MeanInterval]| -> Vec<Value> {
            levers
                .iter()
                .zip(v)
                .map(|(l, m)| json!({"lever": l, "effect": m}))
                .collect()
        };
        let pairs: Vec<Value> = self
            .interactions
            .iter()
            .map(|((j, l), m)| json!({"levers": [levers[*j], levers[*l]], "effect": m}))
            .collect();
        json!({
            "b": self.b, "all_from_a": self.all_a, "main": named(&self.main),
            "interactions": pairs, "shapley": named(&self.shapley),
        })
    }
}

/// Decomposes one metric seed by seed: the effects are linear in the cells,
/// so their mean over seeds is the effect of the mean table.
fn decompose(id: MetricId, cells: &[&[RunOutput]], seeds: usize, k: usize) -> Decomposed {
    let per_seed: Vec<Effects> = (0..seeds)
        .map(|s| {
            let y: Vec<f64> = cells
                .iter()
                .map(|runs| f64::from(runs[s].summary.get(id)))
                .collect();
            effects(&y, k)
        })
        .collect();
    let interval =
        |f: &dyn Fn(&Effects) -> f64| MeanInterval::of(per_seed.iter().map(|e| f(e) as f32));
    let cell_mean = |mask: usize| Stats::of(cells[mask].iter().map(|r| r.summary.get(id))).mean;
    Decomposed {
        b: cell_mean(0),
        all_a: cell_mean(cells.len() - 1),
        main: (0..k).map(|j| interval(&|e| e.main[j])).collect(),
        interactions: (0..k * (k - 1) / 2)
            .map(|i| {
                let pair = per_seed.first().map_or((0, 0), |e| e.interactions[i].0);
                (pair, interval(&|e| e.interactions[i].1))
            })
            .collect(),
        shapley: (0..k).map(|j| interval(&|e| e.shapley[j])).collect(),
    }
}

fn pm(id: MetricId, m: &MeanInterval) -> String {
    let sign = if m.mean >= 0.0 { "+" } else { "" };
    format!(
        "{sign}{} ±{}",
        fmt_value(id, m.mean),
        fmt_value(id, m.half_width())
    )
}

fn print_effects(id: MetricId, d: &Decomposed, levers: &[String]) -> Result<(), CliError> {
    let total: f32 = d.shapley.iter().map(|s| s.mean).sum();
    outln!(
        "\n{}: B {}, every listed lever at A's value {}, difference {:+.4}\n",
        id.info().name,
        fmt_value(id, d.b),
        fmt_value(id, d.all_a),
        total
    );
    let rows: Vec<Vec<String>> = levers
        .iter()
        .enumerate()
        .map(|(j, path)| {
            let share = if total.abs() > 1e-9 {
                format!("{:.0}%", 100.0 * d.shapley[j].mean / total)
            } else {
                "-".into()
            };
            vec![
                short(path).to_string(),
                pm(id, &d.main[j]),
                pm(id, &d.shapley[j]),
                share,
            ]
        })
        .collect();
    out!(
        "{}",
        table(&["lever", "main effect", "Shapley value", "share"], &rows)
    );
    if !d.interactions.is_empty() {
        let rows: Vec<Vec<String>> = d
            .interactions
            .iter()
            .map(|((j, l), m)| {
                vec![
                    format!("{} x {}", short(&levers[*j]), short(&levers[*l])),
                    pm(id, m),
                ]
            })
            .collect();
        outln!("");
        out!("{}", table(&["interaction", "effect"], &rows));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn two_levers_on_a_hand_made_table() {
        // Cell 0 is B, bit j sets lever j to A's value; cell 3 is A.
        let y = [10.0, 7.0, 12.0, 5.0];
        let e = effects(&y, 2);
        // Lever 0 from A to B: +3 with lever 1 at B's value, +7 at A's.
        assert!(close(e.main[0], 5.0) && close(e.main[1], 0.0), "{e:?}");
        assert_eq!(e.interactions.len(), 1);
        assert_eq!(e.interactions[0].0, (0, 1));
        assert!(close(e.interactions[0].1, -4.0));
        assert!(close(e.shapley[0], 5.0) && close(e.shapley[1], 0.0));
        assert!(close(e.shapley.iter().sum::<f64>(), y[0] - y[3]));
    }

    #[test]
    fn shapley_splits_a_threefold_interaction_equally() {
        // Additive costs c_j of setting lever j to A's value, plus t when all
        // three are at A's value.
        let c = [1.0, 2.0, 4.0];
        let t = 3.0;
        let y: Vec<f64> = (0..8usize)
            .map(|m| {
                let additive: f64 = (0..3).filter(|j| m & (1 << j) != 0).map(|j| c[j]).sum();
                10.0 - additive + if m == 7 { t } else { 0.0 }
            })
            .collect();
        let e = effects(&y, 3);
        for (j, cost) in c.iter().enumerate() {
            assert!(close(e.shapley[j], cost - t / 3.0), "{e:?}");
            assert!(close(e.main[j], cost - t / 4.0), "{e:?}");
        }
        for (_, v) in &e.interactions {
            assert!(close(*v, t / 2.0), "{e:?}");
        }
        assert!(close(e.shapley.iter().sum::<f64>(), y[0] - y[7]));
    }

    #[test]
    fn levers_are_design_paths() {
        assert_eq!(
            lever_path("w_reply").ok().as_deref(),
            Some("design.w_reply")
        );
        assert!(lever_path("design.chronological").is_ok());
        assert!(lever_path("world.habit_gain").is_err());
        assert!(lever_path("nope").is_err());
        let mut b = Config::default();
        let a = incentive_core::preset("calm-by-design")
            .map(|p| p.config())
            .unwrap_or_default();
        for path in ["design.reshare_depth_cap", "design.like_counts_visible"] {
            assert!(copy_lever(&a, &mut b, path).is_ok(), "{path}");
        }
        assert_eq!(b.design.reshare_depth_cap, Some(2));
        assert!(!b.design.like_counts_visible);
    }
}
