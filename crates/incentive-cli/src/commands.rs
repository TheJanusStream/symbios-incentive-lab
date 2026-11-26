//! The commands that run simulations: run, compare, sweep and scenario.

use std::fmt::Write as _;
use std::path::PathBuf;

use incentive_core::{Config, MetricId, Summary, Verdict, scenario};
use serde_json::json;

use crate::cli::{RunOptions, Source};
use crate::error::CliError;
use crate::report::{self, MOVING_NOTE, fmt_value, moving_mark, table, table_metrics, view_label};
use crate::runs::{
    self, RunOutput, apply_options, is_moving, mean_slope, metric_stats, parse_seeds, run_seeds,
};

fn out_dir(options: &RunOptions, name: &str) -> Result<PathBuf, CliError> {
    let dir = options
        .out
        .clone()
        .unwrap_or_else(|| report::default_out(name));
    report::ensure_dir(&dir)?;
    Ok(dir)
}

fn summaries(runs: &[RunOutput]) -> Vec<&Summary> {
    runs.iter().map(|r| &r.summary).collect()
}

/// `run`: one design on several seeds.
pub fn run(source: &Source, options: &RunOptions) -> Result<(), CliError> {
    let (label, mut config) = runs::load_source(source)?;
    apply_options(&mut config, options, &[])?;
    let seeds = parse_seeds(&options.seeds)?;
    let metrics = table_metrics(&options.metrics)?;
    let outputs = run_seeds(&config, &seeds)?;
    outln!(
        "{label}: {} seeds, N = {}, {} days, summary over the last {} days\n",
        seeds.len(),
        config.run.population_size,
        config.run.days,
        outputs.first().map_or(0, |o| o.summary.days)
    );
    let rows: Vec<Vec<String>> = metrics
        .iter()
        .map(|&id| {
            let s = metric_stats(&outputs, id);
            vec![
                id.info().name.to_string(),
                view_label(id).to_string(),
                format!(
                    "{}{}",
                    fmt_value(id, s.mean),
                    moving_mark(is_moving(&outputs, id))
                ),
                fmt_value(id, s.sd),
                fmt_value(id, s.min),
                fmt_value(id, s.max),
            ]
        })
        .collect();
    out!(
        "{}",
        table(&["metric", "view", "mean", "sd", "min", "max"], &rows)
    );
    moving_note(metrics.iter().any(|&id| is_moving(&outputs, id)))?;
    let dir = out_dir(options, &format!("run-{}", report::slug(&label)))?;
    report::remove_daily_csvs(&dir)?;
    for o in &outputs {
        report::write_file(
            &dir.join(format!("daily-seed{}.csv", o.seed)),
            &report::daily_csv(&o.history),
        )?;
    }
    let stats: serde_json::Map<String, serde_json::Value> = incentive_core::metric_infos()
        .iter()
        .map(|i| (i.key.to_string(), json!(metric_stats(&outputs, i.id))))
        .collect();
    let slopes: serde_json::Map<String, serde_json::Value> = incentive_core::metric_infos()
        .iter()
        .map(|i| {
            let v = mean_slope(&outputs, i.id);
            (i.key.to_string(), json!(v.is_finite().then_some(v)))
        })
        .collect();
    let moving: Vec<&str> = incentive_core::metric_infos()
        .iter()
        .filter(|i| is_moving(&outputs, i.id))
        .map(|i| i.key)
        .collect();
    let per_seed: Vec<_> = outputs
        .iter()
        .map(|o| json!({"seed": o.seed, "summary": o.summary}))
        .collect();
    let doc = json!({
        "label": label, "config": config, "seeds": seeds, "stats": stats,
        "slope": slopes, "moving": moving, "runs": per_seed,
    });
    report::write_json(&dir.join("summary.json"), &doc)?;
    outln!("\nwrote {}", dir.display());
    Ok(())
}

/// `compare`: two designs paired by seed.
pub fn compare(
    a: &str,
    b: &str,
    set_a: &[String],
    set_b: &[String],
    options: &RunOptions,
) -> Result<(), CliError> {
    let (label_a, mut config_a) = runs::load_spec(a)?;
    let (label_b, mut config_b) = runs::load_spec(b)?;
    apply_options(&mut config_a, options, set_a)?;
    apply_options(&mut config_b, options, set_b)?;
    let seeds = parse_seeds(&options.seeds)?;
    let metrics = table_metrics(&options.metrics)?;
    let jobs: Vec<(Config, u64)> = seeds
        .iter()
        .flat_map(|&s| [(config_a.clone(), s), (config_b.clone(), s)])
        .collect();
    let outputs = runs::run_many(&jobs)?;
    let (runs_a, runs_b): (Vec<_>, Vec<_>) = outputs
        .into_iter()
        .enumerate()
        .partition(|(i, _)| i % 2 == 0);
    let runs_a: Vec<RunOutput> = runs_a.into_iter().map(|(_, r)| r).collect();
    let runs_b: Vec<RunOutput> = runs_b.into_iter().map(|(_, r)| r).collect();
    outln!(
        "A = {label_a}, B = {label_b}: {} paired seeds, N = {}, {} days\n",
        seeds.len(),
        config_a.run.population_size,
        config_a.run.days
    );
    let mut csv = String::from("metric,view,mean_a,mean_b,diff,ci_low,ci_high\n");
    let rows: Vec<Vec<String>> = metrics
        .iter()
        .map(|&id| {
            let row = paired_row(id, &runs_a, &runs_b);
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{},{}",
                id.info().key,
                view_label(id),
                row[0],
                row[1],
                row[2],
                row[3],
                row[4]
            );
            let marker = if row[3] > 0.0 || row[4] < 0.0 {
                "*"
            } else {
                ""
            };
            let mut cells = vec![id.info().name.to_string(), view_label(id).to_string()];
            cells.extend(row.iter().map(|&v| fmt_value(id, v)));
            cells[2].push_str(moving_mark(is_moving(&runs_a, id)));
            cells[3].push_str(moving_mark(is_moving(&runs_b, id)));
            cells.push(marker.to_string());
            cells
        })
        .collect();
    out!(
        "{}",
        table(
            &[
                "metric", "view", "A", "B", "B - A", "95% low", "95% high", ""
            ],
            &rows
        )
    );
    outln!("\n* the 95% confidence interval of the paired difference excludes zero");
    if metrics
        .iter()
        .any(|&id| is_moving(&runs_a, id) || is_moving(&runs_b, id))
    {
        outln!("{MOVING_NOTE}");
    }
    let dir = out_dir(
        options,
        &format!(
            "compare-{}-vs-{}",
            report::slug(&label_a),
            report::slug(&label_b)
        ),
    )?;
    report::write_file(&dir.join("compare.csv"), &csv)?;
    let doc = json!({
        "a": {"label": label_a, "config": config_a, "summaries": summaries(&runs_a)},
        "b": {"label": label_b, "config": config_b, "summaries": summaries(&runs_b)},
        "seeds": seeds,
    });
    report::write_json(&dir.join("compare.json"), &doc)?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// Mean of A, mean of B, mean paired difference and its 95% interval.
fn paired_row(id: MetricId, a: &[RunOutput], b: &[RunOutput]) -> [f32; 5] {
    let d = runs::paired(id, a, b);
    [d.mean_a, d.mean_b, d.diff, d.low, d.high]
}

/// `sweep`: one lever over several values.
pub fn sweep(
    source: &Source,
    lever: &str,
    values: &[String],
    options: &RunOptions,
) -> Result<(), CliError> {
    let (label, mut base) = runs::load_source(source)?;
    apply_options(&mut base, options, &[])?;
    let seeds = parse_seeds(&options.seeds)?;
    let metrics = table_metrics(&options.metrics)?;
    let mut configs = Vec::new();
    for v in values {
        let mut config = base.clone();
        config.set_assignment(&format!("{lever}={v}"))?;
        configs.push(config);
    }
    let jobs: Vec<(Config, u64)> = configs
        .iter()
        .flat_map(|c| seeds.iter().map(move |&s| (c.clone(), s)))
        .collect();
    let outputs = runs::run_many(&jobs)?;
    outln!(
        "{label}: sweep of {lever}, {} seeds each, N = {}, {} days\n",
        seeds.len(),
        base.run.population_size,
        base.run.days
    );
    let mut header = vec![lever.to_string()];
    let mut csv = lever.to_string();
    for id in &metrics {
        header.push(id.info().key.to_string());
        let _ = write!(csv, ",{0}_mean,{0}_sd", id.info().key);
    }
    csv.push('\n');
    let mut rows = Vec::new();
    let mut any_moving = false;
    for (value, chunk) in values.iter().zip(outputs.chunks(seeds.len())) {
        let mut row = vec![value.clone()];
        csv.push_str(value);
        for &id in &metrics {
            let s = metric_stats(chunk, id);
            let moving = is_moving(chunk, id);
            any_moving |= moving;
            row.push(format!(
                "{} ±{}{}",
                fmt_value(id, s.mean),
                fmt_value(id, s.sd),
                moving_mark(moving)
            ));
            let _ = write!(csv, ",{},{}", s.mean, s.sd);
        }
        csv.push('\n');
        rows.push(row);
    }
    let header_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    out!("{}", table(&header_refs, &rows));
    moving_note(any_moving)?;
    let dir = out_dir(
        options,
        &format!("sweep-{}-{}", report::slug(&label), report::slug(lever)),
    )?;
    report::write_file(&dir.join("sweep.csv"), &csv)?;
    outln!("\nwrote {}", dir.display());
    Ok(())
}

/// `scenario`: the player's design against the baseline on the same seeds.
pub fn scenario_cmd(
    id: &str,
    source: &Source,
    solution: bool,
    options: &RunOptions,
) -> Result<(), CliError> {
    let sc = scenario(id)?;
    let design = if solution {
        sc.known_solution()?
    } else if source.preset.is_none() && source.config.is_none() {
        sc.config().design
    } else {
        runs::load_source(source)?.1.design
    };
    let mut player = sc.player_config(&design);
    let mut baseline = sc.config();
    apply_options(&mut player, options, &[])?;
    let size_only = RunOptions {
        set: Vec::new(),
        metrics: Vec::new(),
        out: None,
        seeds: options.seeds.clone(),
        ..*options
    };
    apply_options(&mut baseline, &size_only, &[])?;
    if player.run != baseline.run
        || player.population != baseline.population
        || player.world != baseline.world
    {
        return Err(CliError::Config(incentive_core::ConfigError::Invalid(
            "a scenario fixes the run, population and world; --set may only change design levers"
                .into(),
        )));
    }
    let seeds = parse_seeds(&options.seeds)?;
    let jobs: Vec<(Config, u64)> = seeds
        .iter()
        .flat_map(|&s| [(player.clone(), s), (baseline.clone(), s)])
        .collect();
    let outputs = runs::run_many(&jobs)?;
    let (player_out, baseline_out): (Vec<_>, Vec<_>) = outputs
        .into_iter()
        .enumerate()
        .partition(|(i, _)| i % 2 == 0);
    let player_out: Vec<RunOutput> = player_out.into_iter().map(|(_, r)| r).collect();
    let baseline_out: Vec<RunOutput> = baseline_out.into_iter().map(|(_, r)| r).collect();
    let player_runs: Vec<Summary> = player_out.iter().map(|o| o.summary.clone()).collect();
    let baseline_runs: Vec<Summary> = baseline_out.iter().map(|o| o.summary.clone()).collect();
    let (Some(p), Some(b)) = (Summary::mean(&player_runs), Summary::mean(&baseline_runs)) else {
        return Err(CliError::Seeds(options.seeds.clone()));
    };
    outln!(
        "{}: {} paired seeds, N = {}, {} days\n{}\n",
        sc.title,
        seeds.len(),
        player.run.population_size,
        player.run.days,
        sc.briefing
    );
    let verdict = sc.judge(&player_runs, &baseline_runs);
    let mut any_moving = false;
    let rows: Vec<Vec<String>> = verdict
        .goals
        .iter()
        .map(|g| {
            let (pm, bm) = (
                is_moving(&player_out, g.metric),
                is_moving(&baseline_out, g.metric),
            );
            any_moving |= pm || bm;
            vec![
                g.description.to_string(),
                format!("{}{}", fmt_value(g.metric, g.player), moving_mark(pm)),
                format!("{}{}", fmt_value(g.metric, g.baseline), moving_mark(bm)),
                fmt_value(g.metric, g.threshold),
                format!("{:+.4}", g.margin.mean),
                format!("{:+.4}", g.margin.low),
                format!("{:+.4}", g.margin.high),
                g.verdict.label().to_string(),
            ]
        })
        .collect();
    out!(
        "{}",
        table(
            &[
                "goal",
                "you",
                "baseline",
                "threshold",
                "margin",
                "95% low",
                "95% high",
                ""
            ],
            &rows
        )
    );
    outln!(
        "\nmargin: how far each seed's run is on the passing side of its own baseline's threshold, mean \
         over the paired seeds with its 95% interval. A goal passes when the whole interval is on the \
         passing side, fails when it is wholly on the failing side, and is undecided otherwise."
    );
    moving_note(any_moving)?;
    let failed = verdict
        .goals
        .iter()
        .filter(|g| g.verdict == Verdict::Fail)
        .count();
    let undecided = verdict
        .goals
        .iter()
        .filter(|g| g.verdict == Verdict::Undecided)
        .count();
    outln!(
        "\n{}",
        match verdict.verdict {
            Verdict::Pass => "Scenario passed.".to_string(),
            _ => format!(
                "Scenario not passed: {failed} goal(s) failed, {undecided} undecided{}.",
                if undecided > 0 {
                    " (more seeds may decide them)"
                } else {
                    ""
                }
            ),
        }
    );
    let dir = out_dir(options, &format!("scenario-{id}"))?;
    let doc = json!({
        "scenario": id, "design": design, "seeds": seeds, "verdict": verdict,
        "player": p, "baseline": b,
    });
    report::write_json(&dir.join("scenario.json"), &doc)?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// The note under a table, when any of its metrics is marked moving.
fn moving_note(any: bool) -> Result<(), CliError> {
    if any {
        outln!("{MOVING_NOTE}");
    }
    Ok(())
}
