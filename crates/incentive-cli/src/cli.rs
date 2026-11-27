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
    /// Print a preset's configuration as JSON, for editing, or a design
    /// from an atlas file with that search's base configuration.
    Config {
        /// Preset id.
        #[arg(long, default_value = "engagement-maximiser")]
        preset: String,
        /// An atlas file written by `incentive explore` (map, pareto or
        /// goodhart); needs `--design`.
        #[arg(long, value_name = "FILE", requires = "design")]
        from_atlas: Option<PathBuf>,
        /// Which design of the atlas: a front design's number from 1 (pareto),
        /// a cell `x,y` as printed (map), or a generation number or `last`
        /// (goodhart).
        #[arg(long, requires = "from_atlas")]
        design: Option<String>,
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
    /// Regenerate the calibration tables of docs/model.md as report.md: the
    /// presets at 365 and 90 days, named paired comparisons, the facts and the
    /// scenarios' verdicts.
    Report {
        /// Seeds: an inclusive range `1..8` or a list `1,5,9`.
        #[arg(long, default_value = "1..8")]
        seeds: String,
        /// Individuals per run (default: the configuration's, 2,000).
        #[arg(long)]
        population: Option<u32>,
        /// The long horizon in days (default 365); the short horizon is 90
        /// days, or this when it is shorter. Scenarios run this long too
        /// when it is given.
        #[arg(long)]
        days: Option<u32>,
        /// Output directory (default: the repository's exports/report).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A full factorial between two designs: design B with every subset of
    /// the listed levers set to design A's values, paired by seed, with main
    /// effects, two-way interactions and Shapley shares.
    Factorial {
        /// Design A: a preset id or a JSON configuration file.
        #[arg(long)]
        a: String,
        /// Design B: a preset id or a JSON configuration file.
        #[arg(long)]
        b: String,
        /// Comma-separated design levers, at most 6, for example
        /// `notification_intensity,classifier_threshold` (`design.` may be
        /// left out).
        #[arg(long, value_delimiter = ',', required = true)]
        levers: Vec<String>,
        /// Seeds: an inclusive range `9..16` or a list `9,10,11`.
        #[arg(long, default_value = "9..16")]
        seeds: String,
        /// Days to simulate (default: the configurations').
        #[arg(long)]
        days: Option<u32>,
        /// Individuals (default: the configurations').
        #[arg(long)]
        population: Option<u32>,
        /// Output directory (default: a folder under the repository's exports/).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check the stylized facts at full size.
    Facts {
        /// Number of seeds, paired across variants.
        #[arg(long, default_value_t = 8)]
        seeds: u64,
        /// Individuals per run.
        #[arg(long, default_value_t = 2000)]
        population: u32,
        /// Days per run (fact 3 moves people to a chronological feed for the
        /// last 90 days and needs more than 90: pick the other facts with
        /// --fact for a shorter run).
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// Assignments applied to every arm before the arm's own change,
        /// `path=value`, for example `world.outrage_appeal=0`: a world in
        /// which to check the facts.
        #[arg(long = "set", value_name = "PATH=VALUE")]
        set: Vec<String>,
        /// Check only these facts, by id, comma-separated (default: all),
        /// for example `deactivation`.
        #[arg(long, value_delimiter = ',')]
        fact: Vec<String>,
        /// Output directory (default: the repository's exports/facts).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Run the out-of-sample validation battery (lab #31): ten literature
    /// findings, V1 to V10, never used in calibration, with bands fixed
    /// before the model was run on them.
    ///
    /// Every check runs under the integrity preset and is judged there, as
    /// the stylized facts are; the same check under the engagement ranking is
    /// reported beside it as a sensitivity, not judged. The experiments (V1,
    /// V2, V3, V5, V10) treat a random 0.2 of the regular users and pair each
    /// real run with its placebo on the same seed; V4 and V9 pair the design
    /// with their comparison design; V6, V7 and V8 read one run. Each check
    /// reports its effect with the standard deviation across seeds, the 95%
    /// interval, the band, the verdict and the raw (unscaled) change; V1 and
    /// V3 also report their doses, the manipulation checks fixed by
    /// calibration.
    Validate {
        /// Number of seeds, paired across variants.
        #[arg(long, default_value_t = 8)]
        seeds: u64,
        /// Individuals per run (the battery's full size is 2,000).
        #[arg(long, default_value_t = 2000)]
        population: u32,
        /// Days per run (the battery's full size is 365; V1, V5 and V10 need
        /// more than 90).
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// Assignments applied to every arm before the arm's own change,
        /// `path=value`, for example `world.homophily=0.8`: a world in which
        /// to check the findings.
        #[arg(long = "set", value_name = "PATH=VALUE")]
        set: Vec<String>,
        /// Check only these, by id, comma-separated (default: all), for
        /// example `V1,V3`.
        #[arg(long, value_delimiter = ',')]
        check: Vec<String>,
        /// Output directory (default: the repository's exports/validation).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Run the stylized facts, the headline claims and the out-of-sample
    /// validation checks in sampled worlds: stability under noise on the
    /// world assumptions (local) and sensitivity across their ranges
    /// (global).
    ///
    /// Varied, 54 assumptions: 52 world assumptions, outrage_appeal,
    /// arousal_appeal, social_proof, harassment_harm, comparison_cost,
    /// regret_cost, time_cost, wrongful_sanction_cost, learning_rate,
    /// imitation_rate, choice_temperature, evasion_rate, evasion_decay,
    /// evasion_cost, classifier_retraining, intensity_rate, deterrence,
    /// exposure_contagion (lab #38), habit_gain, stop_bias, churn_base,
    /// leave_threshold, harassment_exit, join_base, reputation_sensitivity,
    /// account_cost_deterrence, homophily,
    /// polarisation_rate, contact_rate, spillover_rate, credulity_scale,
    /// polarisation_backfire, ad_rate, subscription_price, subscription_hurdle,
    /// subscription_uptake, subscription_elasticity, ad_annoyance,
    /// living_income, bot_farm_floor, bot_strategy (categorical),
    /// bot_brigading, polish_gain, polish_cost, reflective_quality_weight,
    /// in_group_outrage_cost, outgroup_like_cut, clickbait_penalty,
    /// style_quality_gap, quality_signal_noise, survey_impulsive_weight,
    /// survey_habit_weight; and two population settings, the troll share
    /// archetype_shares.troll (normalised with the other archetypes' shares
    /// when the population is drawn) and bot_share. Kept at their defaults:
    /// recheck_rate, notification_pull, extra_session_share and
    /// interruption_cost (off by the owner's decision, docs/model.md 6.1 and
    /// 13.2), base_session_items (the session length, held by lab #35 for the
    /// same reason: like the checking habits it sets the time scale at which
    /// every per-impression rate was calibrated), and every run and design
    /// setting and the rest of the population.
    ///
    /// Local: each varied number with a nonzero default is multiplied by
    /// U(1 - jitter, 1 + jitter) and clamped to its catalogue range; those
    /// whose default is 0 (leave_threshold, polarisation_backfire) and
    /// bot_strategy keep their defaults. Global: a Latin hypercube over each
    /// number's range, and bot_strategy drawn uniformly, except under the
    /// plausible ranges, which keep it at its default. The revised ranges
    /// (the default) follow lab #31's rule, as docs/model.md section 11 lists
    /// them: a documented range written for a text default that calibration
    /// changed (13.1) is scaled by the same factor as the default; an
    /// assumption without a documented range gets default/3 to 3 x default;
    /// the population settings get default/2 to 2 x default; judgement
    /// gives leave_threshold -0.1 to 0.1 and outgroup_like_cut 0 to 1
    /// (lab #35) and exposure_contagion 0 to 0.56 (lab #38); all clamped to
    /// the catalogue. The plausible ranges (lab #35, section 11's Plausible
    /// column) give a positive scale default/1.5 to 1.5 x default, a share or
    /// weight in [0, 1] default +- 0.15 (clamped), leave_threshold -0.05 to
    /// 0.05 and the population settings default/1.5 to 1.5 x default. The
    /// documented ranges are section 11's before lab #31, clipped to the
    /// catalogue, with the catalogue range where it gave none. World 0 is
    /// always the default world, the reference; the summary counts the
    /// sampled worlds.
    ///
    /// The noisy facts and claims, whose seed share exceeded 0.4 in lab #31
    /// (outrage-learning, harassment-exit, H5, V1, V3, V4, V5, V9, V10, and
    /// H7 in global mode), run on --seeds-noisy seeds; noisiness is per fact
    /// or claim, since a fact's variants run on the same seeds for all its
    /// checks.
    Robustness {
        /// How worlds are sampled.
        #[arg(long, value_enum, default_value = "local")]
        mode: crate::robustness::Mode,
        /// Sampled worlds, beside the default world (0 runs the default world
        /// alone).
        #[arg(long, default_value_t = 20)]
        worlds: usize,
        /// Seeds 1 to this, paired across variants and the same in every world.
        #[arg(long, default_value_t = 2)]
        seeds: u64,
        /// Seeds 1 to this for the noisy facts and claims (default: --seeds).
        #[arg(long)]
        seeds_noisy: Option<u64>,
        /// Seed of the world sampling.
        #[arg(long, default_value_t = 1)]
        sample_seed: u64,
        /// Which battery: the stylized facts, the headline claims, the
        /// validation checks, or all three.
        #[arg(long, value_enum, default_value = "all")]
        battery: crate::robustness::BatteryChoice,
        /// Global mode: the ranges to explore (revised by lab #31's rule,
        /// with lab #35's judgement ranges), the plausible ranges of lab #35,
        /// or section 11's ranges as documented before lab #31.
        #[arg(long, value_enum, default_value = "revised")]
        ranges: crate::robustness::sample::Ranges,
        /// Individuals per run (default: each battery's full size, 2,000).
        #[arg(long)]
        population: Option<u32>,
        /// Days per run (default: each battery's full size, 365).
        #[arg(long)]
        days: Option<u32>,
        /// Local mode: the half-width of the factor's range, 0 to 1.
        #[arg(long, default_value_t = 0.25)]
        jitter: f64,
        /// Output directory (default: the repository's
        /// `exports/robustness/<mode>-<sample-seed>`, with -documented or
        /// -plausible for global mode on those ranges).
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
        /// Designs evaluated before the first generation: the presets, then
        /// random designs.
        #[arg(long, default_value_t = 128)]
        initial: u32,
    },
    /// NSGA-II: the Pareto front of maximising engagement, minimising harm
    /// and maximising benefit. Defaults: 40 generations of 64 (the
    /// population, which starts with the presets).
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
    /// Fresh seeds every design in the atlas is re-evaluated on after the
    /// search, at the same size: a range `9..16`, a list, or `none`.
    #[arg(long, default_value = "9..16")]
    pub confirm_seeds: String,
    /// Output directory (default: a folder under the repository's exports/).
    #[arg(long)]
    pub out: Option<PathBuf>,
}
