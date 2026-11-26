//! Building configurations and running them on many seeds in parallel. Each
//! run is single-threaded; rayon spreads runs over the CPUs it may use.

use std::collections::BTreeMap;
use std::path::Path;

use incentive_core::{Config, DailyMetrics, MetricId, PairedDifference, Stats, Summary, preset};
use rayon::prelude::*;

use crate::cli::{RunOptions, Source};
use crate::error::CliError;

/// One finished run.
pub struct RunOutput {
    /// Its seed.
    pub seed: u64,
    /// Every day's metrics.
    pub history: Vec<DailyMetrics>,
    /// The summary window's averages.
    pub summary: Summary,
}

/// Reads a design source: a preset (default engagement-maximiser) or a file.
pub fn load_source(source: &Source) -> Result<(String, Config), CliError> {
    match (&source.preset, &source.config) {
        (_, Some(path)) => Ok((file_label(path), load_file(path)?)),
        (Some(id), None) => Ok((id.clone(), preset(id)?.config())),
        (None, None) => Ok(("engagement-maximiser".into(), Config::default())),
    }
}

/// Reads a design given as a preset id or a JSON file path.
pub fn load_spec(spec: &str) -> Result<(String, Config), CliError> {
    let path = Path::new(spec);
    if spec.ends_with(".json") || path.is_file() {
        Ok((file_label(path), load_file(path)?))
    } else {
        Ok((spec.to_string(), preset(spec)?.config()))
    }
}

fn file_label(path: &Path) -> String {
    path.file_stem()
        .map_or_else(|| "config".into(), |s| s.to_string_lossy().into_owned())
}

fn load_file(path: &Path) -> Result<Config, CliError> {
    let text = std::fs::read_to_string(path).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Config::from_json(&text)?)
}

/// Applies `--days`, `--population` and `--set` to a configuration.
pub fn apply_options(
    config: &mut Config,
    options: &RunOptions,
    extra: &[String],
) -> Result<(), CliError> {
    if let Some(days) = options.days {
        config.set_path("run.days", f64::from(days))?;
    }
    if let Some(n) = options.population {
        config.set_path("run.population_size", f64::from(n))?;
    }
    for assignment in options.set.iter().chain(extra) {
        config.set_assignment(assignment)?;
    }
    config.run.tracked_agents = 0;
    config.validate()?;
    Ok(())
}

/// Reads `1..8` (inclusive) or `1,5,9`.
pub fn parse_seeds(spec: &str) -> Result<Vec<u64>, CliError> {
    let bad = || CliError::Seeds(spec.to_string());
    let seeds: Vec<u64> = if let Some((a, b)) = spec.split_once("..") {
        let (a, b): (u64, u64) = (
            a.trim().parse().map_err(|_| bad())?,
            b.trim().parse().map_err(|_| bad())?,
        );
        if b < a {
            return Err(bad());
        }
        (a..=b).collect()
    } else {
        spec.split(',')
            .map(|s| s.trim().parse().map_err(|_| bad()))
            .collect::<Result<_, _>>()?
    };
    if seeds.is_empty() {
        Err(bad())
    } else {
        Ok(seeds)
    }
}

/// Runs `config` on every seed in parallel.
pub fn run_seeds(config: &Config, seeds: &[u64]) -> Result<Vec<RunOutput>, CliError> {
    run_many(
        &seeds
            .iter()
            .map(|&s| (config.clone(), s))
            .collect::<Vec<_>>(),
    )
}

/// Runs a batch of `(configuration, seed)` pairs in parallel, in input order.
pub fn run_many(jobs: &[(Config, u64)]) -> Result<Vec<RunOutput>, CliError> {
    jobs.par_iter()
        .map(|(config, seed)| {
            let mut config = config.clone();
            config.run.seed = *seed;
            let mut sim = incentive_core::Simulation::new(config)?;
            let summary = sim.run_to_end();
            Ok(RunOutput {
                seed: *seed,
                history: sim.history().to_vec(),
                summary,
            })
        })
        .collect()
}

/// Statistics of one metric over the runs' summaries.
pub fn metric_stats(runs: &[RunOutput], id: MetricId) -> Stats {
    Stats::of(runs.iter().map(|r| r.summary.get(id)))
}

/// The mean over the runs of a metric's relative change across the summary
/// window ([`Summary::slope`]).
pub fn mean_slope(runs: &[RunOutput], id: MetricId) -> f32 {
    Stats::of(runs.iter().map(|r| r.summary.slope(id))).mean
}

/// Whether a metric moves in these runs ([`incentive_core::is_moving_over`]):
/// the mean of its relative changes across the summary window exceeds
/// [`incentive_core::MOVING_THRESHOLD`] and two standard errors, across the
/// seeds or, for one seed, from its regression.
pub fn is_moving(runs: &[RunOutput], id: MetricId) -> bool {
    incentive_core::is_moving_over(runs.iter().map(|r| &r.summary), id)
}

/// [`is_moving`] for borrowed runs.
pub fn moving_refs(runs: &[&RunOutput], id: MetricId) -> bool {
    incentive_core::is_moving_over(runs.iter().map(|r| &r.summary), id)
}

/// Runs a batch of `(configuration, seed)` pairs in parallel and returns, for
/// each job in input order, the index of its output: identical jobs (the same
/// configuration on the same seed) run once.
pub fn run_unique(jobs: &[(Config, u64)]) -> Result<(Vec<RunOutput>, Vec<usize>), CliError> {
    let mut keys: BTreeMap<(String, u64), usize> = BTreeMap::new();
    let mut unique: Vec<(Config, u64)> = Vec::new();
    let mut index = Vec::with_capacity(jobs.len());
    for (config, seed) in jobs {
        let mut keyed = config.clone();
        keyed.run.seed = *seed;
        let key = (keyed.to_json_pretty(), *seed);
        let next = unique.len();
        let i = *keys.entry(key).or_insert(next);
        if i == next {
            unique.push((config.clone(), *seed));
        }
        index.push(i);
    }
    Ok((run_many(&unique)?, index))
}

/// One metric of two designs paired by seed: `a[i]` and `b[i]` ran on the
/// same seed.
pub fn paired(id: MetricId, a: &[RunOutput], b: &[RunOutput]) -> PairedDifference {
    let values =
        |runs: &[RunOutput]| -> Vec<f32> { runs.iter().map(|r| r.summary.get(id)).collect() };
    PairedDifference::of(&values(a), &values(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_parse() {
        assert_eq!(parse_seeds("1..3").ok(), Some(vec![1, 2, 3]));
        assert_eq!(parse_seeds("4, 7").ok(), Some(vec![4, 7]));
        assert!(parse_seeds("3..1").is_err());
        assert!(parse_seeds("x").is_err());
    }
}
