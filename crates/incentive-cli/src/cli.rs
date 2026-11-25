//! Command-line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Batch runs, comparisons, sweeps, scenarios and fact checks of the incentive lab.
#[derive(Debug, Parser)]
#[command(name = "incentive", version, about)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the preset platform designs.
    Presets,
    /// List the scenarios with their goals.
    Scenarios,
    /// List every design lever and world assumption with its range.
    Levers,
    /// Print a preset's configuration as JSON, for editing.
    Config {
        /// Preset id.
        #[arg(long, default_value = "engagement-maximiser")]
        preset: String,
    },
    /// Run one design on several seeds and summarise it.
    Run {
        /// The design to run.
        #[command(flatten)]
        source: Source,
        /// Run options.
        #[command(flatten)]
        options: RunOptions,
    },
    /// Compare two designs paired by seed.
    Compare {
        /// Design A: a preset id or a JSON configuration file.
        #[arg(long)]
        a: String,
        /// Design B: a preset id or a JSON configuration file.
        #[arg(long)]
        b: String,
        /// Extra assignments for A only, `path=value`.
        #[arg(long = "set-a", value_name = "PATH=VALUE")]
        set_a: Vec<String>,
        /// Extra assignments for B only, `path=value`.
        #[arg(long = "set-b", value_name = "PATH=VALUE")]
        set_b: Vec<String>,
        /// Run options (assignments apply to both).
        #[command(flatten)]
        options: RunOptions,
    },
    /// Run one lever over a list of values.
    Sweep {
        /// The design to start from.
        #[command(flatten)]
        source: Source,
        /// Dotted path of the lever or assumption, for example `design.w_quality`.
        #[arg(long)]
        lever: String,
        /// Comma-separated values.
        #[arg(long, value_delimiter = ',', required = true)]
        values: Vec<String>,
        /// Run options.
        #[command(flatten)]
        options: RunOptions,
    },
    /// Play a scenario: run your design and the baseline on the same seeds.
    /// `--set` changes your design only; `--days` and `--population` shrink both runs.
    Scenario {
        /// Scenario id.
        id: String,
        /// Your design.
        #[command(flatten)]
        source: Source,
        /// Use the scenario's recorded known solution as your design.
        #[arg(long, conflicts_with_all = ["preset", "config"])]
        solution: bool,
        /// Run options.
        #[command(flatten)]
        options: RunOptions,
    },
    /// Search the design space with the world held fixed: a map of where
    /// designs land, a Pareto front, or a proxy optimised alone.
    Explore {
        /// The search.
        #[command(subcommand)]
        mode: ExploreMode,
    },
    /// Check the stylized facts at full size.
    Facts {
        /// Number of seeds, paired across variants.
        #[arg(long, default_value_t = 8)]
        seeds: u64,
        /// Individuals per run.
        #[arg(long, default_value_t = 2000)]
        population: u32,
        /// Days per run.
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// Output directory (default: the repository's exports/facts).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Where a design comes from.
#[derive(Debug, Args)]
pub struct Source {
    /// Preset id (default: engagement-maximiser).
    #[arg(long, conflicts_with = "config")]
    pub preset: Option<String>,
    /// JSON configuration file; missing fields take their defaults.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// Options shared by the commands that run simulations.
#[derive(Debug, Args)]
pub struct RunOptions {
    /// Days to simulate (default: the configuration's).
    #[arg(long)]
    pub days: Option<u32>,
    /// Individuals (default: the configuration's).
    #[arg(long)]
    pub population: Option<u32>,
    /// Seeds: an inclusive range `1..8` or a list `1,5,9`.
    #[arg(long, default_value = "1..8")]
    pub seeds: String,
    /// Configuration assignments, `path=value`, for example `design.w_quality=1`.
    #[arg(long = "set", value_name = "PATH=VALUE")]
    pub set: Vec<String>,
    /// Comma-separated metric keys for the table (default: the headline metrics).
    #[arg(long, value_delimiter = ',')]
    pub metrics: Vec<String>,
    /// Output directory (default: a folder under the repository's exports/).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// The searches of `incentive explore`.
#[derive(Debug, Subcommand)]
pub enum ExploreMode {
    /// MAP-Elites: the highest benefit index found in every cell of a grid of
    /// engagement index (across) against harm index (up).
    /// Defaults: 50 generations of 64.
    Map {
        /// Search options.
        #[command(flatten)]
        search: SearchOptions,
        /// Cells per axis of the grid.
        #[arg(long, default_value_t = 16)]
        grid: u32,
        /// Designs evaluated before the first generation: the six presets,
        /// then random designs.
        #[arg(long, default_value_t = 128)]
        initial: u32,
    },
    /// NSGA-II: the Pareto front of maximising engagement, minimising harm
    /// and maximising benefit. Defaults: 40 generations of 64 (the
    /// population, which starts with the six presets).
    Pareto {
        /// Search options.
        #[command(flatten)]
        search: SearchOptions,
    },
    /// Maximise one observable proxy alone, from random designs, and record
    /// the truth of every generation's best design.
    /// Defaults: 25 generations of 40.
    Goodhart {
        /// Search options.
        #[command(flatten)]
        search: SearchOptions,
        /// Key of the observable metric to maximise, for example
        /// `engagement_index` or `survey_satisfaction`.
        #[arg(long, default_value = "engagement_index")]
        proxy: String,
    },
}

/// Options shared by the explore modes.
#[derive(Debug, Args)]
pub struct SearchOptions {
    /// JSON configuration whose world assumptions and population are kept
    /// (default: the defaults); its design is ignored, every lever is
    /// searched.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Seed of the search's own random choices.
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Simulation seeds per candidate design: seeds 1 to this number, and
    /// the metrics are their mean.
    #[arg(long, default_value_t = 2)]
    pub seeds: u64,
    /// Individuals per simulated run.
    #[arg(long, default_value_t = 600)]
    pub population: u32,
    /// Days per simulated run.
    #[arg(long, default_value_t = 180)]
    pub days: u32,
    /// Generations (default: per mode).
    #[arg(long)]
    pub generations: Option<u32>,
    /// Candidates per generation (default: per mode).
    #[arg(long)]
    pub batch: Option<u32>,
    /// Chance that each gene mutates in a child.
    #[arg(long, default_value_t = 0.1)]
    pub mutation_rate: f32,
    /// Output directory (default: a folder under the repository's exports/).
    #[arg(long)]
    pub out: Option<PathBuf>,
}
