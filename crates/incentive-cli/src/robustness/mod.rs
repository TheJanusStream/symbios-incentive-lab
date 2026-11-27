//! `incentive robustness`: runs a battery of paired checks (the stylized
//! facts, the headline claims, the out-of-sample validation checks) in many
//! sampled worlds and reports how often
//! each check, fact and claim holds, with the spread of its effect and, for
//! global sampling, the world assumptions that drive it.
//!
//! - Worlds come from the sample seed ([`sample`]); world 0 is always the
//!   default world, the reference whose signs the summary compares against,
//!   and the summary counts the sampled worlds 1 to K only.
//! - Each world runs every variant of every selected fact or claim on the
//!   same seeds 1 to S, paired across variants and the same in every world, and
//!   the default design on those seeds as the world's baseline: a world is
//!   degenerate when the baseline's user share on its last day, averaged over
//!   the seeds, lies below [`DEGENERATE_USER_SHARE`].
//! - Runs go through the runner's parallel batch (`runs::run_unique`) a chunk
//!   of worlds at a time, identical runs once (the default design is variant
//!   0 of several facts and claims); every run is a pure function of its
//!   configuration, so a command reproduces its files exactly. The files hold
//!   no times; the elapsed time goes to standard error.
//!
//! A battery is a list of [`Fact`]s with its full run size ([`Battery`]); a
//! new battery needs a variant of [`BatteryChoice`] and an entry in
//! [`batteries`]. The validation battery runs its judged checks, under the
//! integrity preset; its engagement-ranking twin is `incentive validate`'s
//! sensitivity and does not run here.

pub mod sample;
pub mod stats;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use clap::ValueEnum;
use incentive_core::{
    Bound, CheckDetail, CheckResult, Config, Fact, FactRun, MetricId, facts, headline_claims,
    validation_battery,
};
use serde::Serialize;
use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::report::{self, table};
use crate::runs;
use sample::{Kind, Plan, Ranges, Value, Varied};

/// User share below which a world is degenerate: on the last day of the
/// baseline (the default design), averaged over the seeds.
pub const DEGENERATE_USER_SHARE: f32 = 0.05;

/// A band end at least this large in magnitude stands in for infinity (the
/// headline claims' open bands use 1e9).
const OPEN_BOUND: f32 = 1.0e8;

/// Worlds whose runs go to the parallel batch together.
const CHUNK_WORLDS: usize = 8;

/// How worlds are sampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Multiplicative jitter around the defaults.
    Local,
    /// A Latin hypercube over the ranges to explore.
    Global,
}

impl Mode {
    /// The mode's name, as on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }
}

/// Which battery runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum BatteryChoice {
    /// The stylized facts (docs/model.md, section 15).
    Facts,
    /// The headline claims H1 to H8 (docs/model.md, 13.3).
    Headline,
    /// The out-of-sample validation checks V1 to V10 (lab #31), under the
    /// integrity preset.
    Validation,
    /// Every battery.
    All,
}

impl BatteryChoice {
    /// The choice's name, as on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Facts => "facts",
            Self::Headline => "headline",
            Self::Validation => "validation",
            Self::All => "all",
        }
    }
}

/// A battery: facts or claims with the run size they are judged at.
#[derive(Clone, Copy, Debug)]
pub struct Battery {
    /// Short id, as on the command line.
    pub id: &'static str,
    /// The facts or claims.
    pub items: &'static [Fact],
    /// Individuals per run at full size.
    pub population: u32,
    /// Days per run at full size.
    pub days: u32,
}

/// The selected batteries, in order.
pub fn batteries(choice: BatteryChoice) -> Vec<Battery> {
    let facts_battery = Battery {
        id: "facts",
        items: facts(),
        population: 2_000,
        days: 365,
    };
    let headline = Battery {
        id: "headline",
        items: headline_claims(),
        population: 2_000,
        days: 365,
    };
    let validation = Battery {
        id: "validation",
        items: validation_battery(),
        population: 2_000,
        days: 365,
    };
    match choice {
        BatteryChoice::Facts => vec![facts_battery],
        BatteryChoice::Headline => vec![headline],
        BatteryChoice::Validation => vec![validation],
        BatteryChoice::All => vec![facts_battery, headline, validation],
    }
}

/// The options of `incentive robustness`.
#[derive(Clone, Debug)]
pub struct Options {
    /// Local or global sampling.
    pub mode: Mode,
    /// Sampled worlds, beside the default world.
    pub worlds: usize,
    /// Seeds 1 to this, paired across variants.
    pub seeds: u64,
    /// Seed of the world sampling.
    pub sample_seed: u64,
    /// Which battery.
    pub battery: BatteryChoice,
    /// Individuals per run instead of each battery's full size.
    pub population: Option<u32>,
    /// Days per run instead of each battery's full size.
    pub days: Option<u32>,
    /// Half-width of the local factor's range.
    pub jitter: f64,
    /// The global ranges: as documented, or revised by lab #31's rule.
    pub ranges: Ranges,
    /// Output directory.
    pub out: Option<PathBuf>,
}

/// One check in one world.
#[derive(Clone, Debug, Serialize)]
struct CheckRecord {
    label: &'static str,
    /// The effect on the seed means.
    effect: f32,
    /// The effect from its neutral value ([`CheckDetail::direction`]).
    direction: f32,
    per_seed: Vec<f32>,
    bound: Bound,
    bound_text: String,
    in_band: bool,
    known_limit: bool,
}

/// One fact or claim in one world.
#[derive(Clone, Debug, Serialize)]
struct ItemRecord {
    id: &'static str,
    /// Every check that is not a known limit lies in its band.
    holds: bool,
    checks: Vec<CheckRecord>,
}

/// One battery in one world.
#[derive(Clone, Debug, Serialize)]
struct BatteryRecord {
    battery: &'static str,
    population: u32,
    days: u32,
    baseline_user_share: f32,
    degenerate: bool,
    items: Vec<ItemRecord>,
}

/// One world.
#[derive(Clone, Debug, Serialize)]
struct WorldRecord {
    world: usize,
    reference: bool,
    values: BTreeMap<&'static str, Json>,
    batteries: Vec<BatteryRecord>,
}

/// Where a group's runs sit in a chunk's job list: `start + seed * k + v`.
struct Slot {
    start: usize,
    variants: usize,
}

/// `incentive robustness`.
pub fn robustness(options: &Options) -> Result<(), CliError> {
    if !(0.0..=1.0).contains(&options.jitter) {
        return Err(CliError::Explore(format!(
            "--jitter must lie in 0 to 1, not {}",
            options.jitter
        )));
    }
    let varied = sample::varied(options.ranges)?;
    let plan = match options.mode {
        Mode::Local => Plan::Local {
            jitter: options.jitter,
        },
        Mode::Global => Plan::Global,
    };
    let selected = batteries(options.battery);
    let seeds: Vec<u64> = (1..=options.seeds.max(1)).collect();
    let mut values = vec![default_values(&varied)];
    values.extend(sample::sample(
        &varied,
        plan,
        options.worlds,
        options.sample_seed,
    ));
    let configs = values
        .iter()
        .map(|v| sample::world_config(&varied, v))
        .collect::<Result<Vec<_>, _>>()?;
    let runs_of = |b: &Battery| FactRun {
        seeds: seeds.clone(),
        population: options.population.unwrap_or(b.population),
        days: options.days.unwrap_or(b.days),
    };
    print_header(options, &varied, &selected, &runs_of)?;

    let started = Instant::now();
    let mut records = Vec::with_capacity(configs.len());
    for (chunk_index, chunk) in configs.chunks(CHUNK_WORLDS).enumerate() {
        let first_world = chunk_index * CHUNK_WORLDS;
        let mut jobs: Vec<(Config, u64)> = Vec::new();
        let mut slots: Vec<Vec<(Slot, Vec<Slot>)>> = Vec::new();
        for world in chunk {
            let mut per_battery = Vec::new();
            for b in &selected {
                let run = runs_of(b);
                let baseline = Slot {
                    start: jobs.len(),
                    variants: 1,
                };
                for &s in &seeds {
                    let mut c = world.clone();
                    c.run.seed = s;
                    c.run.population_size = run.population;
                    c.run.days = run.days;
                    c.run.tracked_agents = 0;
                    jobs.push((c, s));
                }
                let mut items = Vec::new();
                for item in b.items {
                    let slot = Slot {
                        start: jobs.len(),
                        variants: item.variants.len(),
                    };
                    for &s in &seeds {
                        for c in item.configs_from(world, &run, s) {
                            c.validate()?;
                            jobs.push((c, s));
                        }
                    }
                    items.push(slot);
                }
                per_battery.push((baseline, items));
            }
            slots.push(per_battery);
        }
        let (outputs, index) = runs::run_unique(&jobs)?;
        let history = |slot: &Slot, seed: usize, variant: usize| {
            &outputs[index[slot.start + seed * slot.variants + variant]].history
        };
        for (w, per_battery) in slots.iter().enumerate() {
            let world = first_world + w;
            let mut battery_records = Vec::new();
            for (b, (baseline, items)) in selected.iter().zip(per_battery) {
                let run = runs_of(b);
                let share = mean_f32(
                    (0..seeds.len())
                        .map(|s| last_value(history(baseline, s, 0), MetricId::UserShare)),
                );
                let mut item_records = Vec::new();
                for (item, slot) in b.items.iter().zip(items) {
                    let histories: Vec<Vec<_>> = (0..slot.variants)
                        .map(|v| {
                            (0..seeds.len())
                                .map(|s| history(slot, s, v).clone())
                                .collect()
                        })
                        .collect();
                    item_records.push(item_record(item, item.evaluate_detailed(&histories)));
                }
                battery_records.push(BatteryRecord {
                    battery: b.id,
                    population: run.population,
                    days: run.days,
                    baseline_user_share: share,
                    degenerate: share < DEGENERATE_USER_SHARE,
                    items: item_records,
                });
            }
            records.push(WorldRecord {
                world,
                reference: world == 0,
                values: world_values(&varied, &configs[world])?,
                batteries: battery_records,
            });
        }
        eprintln!(
            "worlds {} to {} of 0 to {} done ({} runs, {} distinct), {:.0} s",
            first_world,
            first_world + chunk.len() - 1,
            configs.len() - 1,
            jobs.len(),
            outputs.len(),
            started.elapsed().as_secs_f32()
        );
    }
    let elapsed = started.elapsed().as_secs_f32();
    eprintln!(
        "{} worlds (the default world included) in {elapsed:.1} s, {:.1} s per world",
        records.len(),
        elapsed / records.len() as f32
    );

    let summary = summarise(options, &varied, &selected, &records);
    print_summary(&summary)?;
    let dir = out_dir(options);
    report::ensure_dir(&dir)?;
    let settings = settings_json(options, &varied, &selected, &runs_of);
    report::write_json(
        &dir.join("worlds.json"),
        &json!({"settings": settings, "worlds": records}),
    )?;
    report::write_file(&dir.join("worlds.csv"), &worlds_csv(&varied, &records))?;
    report::write_file(&dir.join("checks.csv"), &checks_csv(&records))?;
    report::write_json(
        &dir.join("summary.json"),
        &json!({"settings": settings, "summary": summary}),
    )?;
    report::write_file(
        &dir.join("summary.md"),
        &summary_md(options, &varied, &selected, &runs_of, &summary),
    )?;
    outln!("wrote {}", dir.display());
    Ok(())
}

/// The default world's values.
fn default_values(varied: &[Varied]) -> Vec<Value> {
    varied
        .iter()
        .map(|v| match v.kind {
            Kind::Continuous { default, .. } => Value::Number(default),
            Kind::Categorical { default, .. } => Value::Choice(default),
        })
        .collect()
}

/// A world's values as its configuration holds them.
fn world_values(
    varied: &[Varied],
    config: &Config,
) -> Result<BTreeMap<&'static str, Json>, CliError> {
    varied
        .iter()
        .map(|v| {
            let value = config.get_path(v.info.path)?;
            let value = value.as_f64().map_or(value, |x| json!(sample::shortest(x)));
            Ok((short(v.info.path), value))
        })
        .collect()
}

/// A path without its `world.` or `population.` prefix.
fn short(path: &str) -> &str {
    path.strip_prefix("world.")
        .or_else(|| path.strip_prefix("population."))
        .unwrap_or(path)
}

fn last_value(history: &[incentive_core::DailyMetrics], id: MetricId) -> f32 {
    history.last().map_or(f32::NAN, |d| d.get(id))
}

fn mean_f32(values: impl Iterator<Item = f32>) -> f32 {
    let v: Vec<f32> = values.collect();
    if v.is_empty() {
        f32::NAN
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}

/// A band's ends, a bound standing in for infinity read as infinite.
fn open_ends(bound: Bound) -> (f32, f32) {
    let (low, high) = match bound {
        Bound::Above(t) => (t, f32::INFINITY),
        Bound::Below(t) => (f32::NEG_INFINITY, t),
        Bound::Between(a, b) => (a, b),
    };
    let open = |x: f32| x.abs() >= OPEN_BOUND;
    (
        if open(low) { f32::NEG_INFINITY } else { low },
        if open(high) { f32::INFINITY } else { high },
    )
}

/// A band as text: `> 0`, `< 0.01` or `0.15 to 0.6`.
fn bound_text(bound: Bound) -> String {
    let n = |x: f32| num(f64::from(x));
    match open_ends(bound) {
        (low, high) if low.is_infinite() && high.is_infinite() => "any".into(),
        (low, high) if high.is_infinite() => format!("> {}", n(low)),
        (low, high) if low.is_infinite() => format!("< {}", n(high)),
        (low, high) => format!("{} to {}", n(low), n(high)),
    }
}

fn item_record(item: &Fact, details: Vec<CheckDetail>) -> ItemRecord {
    let results: Vec<CheckResult> = details.iter().map(|d| d.result.clone()).collect();
    ItemRecord {
        id: item.id,
        holds: Fact::holds(&results),
        checks: details
            .into_iter()
            .map(|d| CheckRecord {
                label: d.result.label,
                effect: d.result.effect,
                direction: d.direction,
                per_seed: d.per_seed,
                bound: d.result.bound,
                bound_text: bound_text(d.result.bound),
                in_band: d.result.passed,
                known_limit: d.result.known_limit,
            })
            .collect(),
    }
}

/// Across worlds, one check.
#[derive(Clone, Debug, Serialize)]
struct CheckSummary {
    label: &'static str,
    bound: String,
    known_limit: bool,
    reference_effect: f32,
    reference_in_band: bool,
    /// Share of the sampled worlds where the effect lies in its band.
    in_band_share: f64,
    /// The same over the non-degenerate worlds.
    in_band_share_nondegenerate: f64,
    /// Share of the sampled worlds whose direction has the reference's sign.
    same_sign_share: f64,
    /// The same over the non-degenerate worlds.
    same_sign_share_nondegenerate: f64,
    /// Quantiles of the effect over the sampled worlds with a value.
    median: f64,
    q05: f64,
    q95: f64,
    /// The same over the non-degenerate worlds.
    median_nondegenerate: f64,
    q05_nondegenerate: f64,
    q95_nondegenerate: f64,
    /// Sampled worlds with a value (not NaN).
    with_value: usize,
    /// Global mode only: what drives the effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    sensitivity: Option<Sensitivity>,
}

/// Across worlds, one fact or claim.
#[derive(Clone, Debug, Serialize)]
struct ItemSummary {
    id: &'static str,
    title: &'static str,
    reference_holds: bool,
    holds_share: f64,
    holds_share_nondegenerate: f64,
    checks: Vec<CheckSummary>,
}

/// Across worlds, one battery.
#[derive(Clone, Debug, Serialize)]
struct BatterySummary {
    battery: &'static str,
    worlds: usize,
    degenerate_worlds: usize,
    reference_degenerate: bool,
    items: Vec<ItemSummary>,
}

/// One assumption's rank correlations with a check's effect.
#[derive(Clone, Debug, Serialize)]
struct Driver {
    assumption: &'static str,
    spearman: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    prcc: Option<f64>,
}

/// A check's sensitivity to the varied assumptions (global mode).
#[derive(Clone, Debug, Serialize)]
struct Sensitivity {
    /// Non-degenerate worlds with a value, the observations.
    worlds: usize,
    /// Why PRCC is missing, when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    /// The five largest by absolute PRCC, or by absolute Spearman without it.
    top: Vec<Driver>,
    /// Every continuous assumption, in catalogue order.
    all: Vec<Driver>,
    /// The median effect per choice of each categorical assumption.
    by_choice: BTreeMap<&'static str, BTreeMap<&'static str, (usize, f64)>>,
}

/// Where a check sits in a world's record: battery, item, check.
#[derive(Clone, Copy, Debug)]
struct Index {
    bi: usize,
    ii: usize,
    ci: usize,
}

/// The parts of a check's record the summary reads.
#[derive(Clone, Copy, Debug)]
struct Point {
    effect: f32,
    direction: f32,
    in_band: bool,
}

impl Point {
    fn of(c: &CheckRecord) -> Self {
        Self {
            effect: c.effect,
            direction: c.direction,
            in_band: c.in_band,
        }
    }
}

fn item_at(w: &WorldRecord, bi: usize, ii: usize) -> &ItemRecord {
    &w.batteries[bi].items[ii]
}

fn check_at(w: &WorldRecord, at: Index) -> &CheckRecord {
    &item_at(w, at.bi, at.ii).checks[at.ci]
}

fn share(flags: impl Iterator<Item = bool>) -> f64 {
    let (mut n, mut yes) = (0usize, 0usize);
    for f in flags {
        n += 1;
        yes += usize::from(f);
    }
    if n == 0 {
        f64::NAN
    } else {
        yes as f64 / n as f64
    }
}

fn sign(x: f32) -> Option<i8> {
    if x.is_nan() {
        None
    } else if x > 0.0 {
        Some(1)
    } else if x < 0.0 {
        Some(-1)
    } else {
        Some(0)
    }
}

fn summarise(
    options: &Options,
    varied: &[Varied],
    selected: &[Battery],
    records: &[WorldRecord],
) -> Vec<BatterySummary> {
    let reference = &records[0];
    let sampled = &records[1..];
    selected
        .iter()
        .enumerate()
        .map(|(bi, b)| {
            let degenerate: Vec<bool> =
                sampled.iter().map(|w| w.batteries[bi].degenerate).collect();
            let items = b
                .items
                .iter()
                .enumerate()
                .map(|(ii, item)| {
                    let ref_item = &reference.batteries[bi].items[ii];
                    let item_of = |w: &WorldRecord| item_at(w, bi, ii).holds;
                    let checks = (0..item.checks.len())
                        .map(|ci| {
                            let rc = &ref_item.checks[ci];
                            let at = Index { bi, ii, ci };
                            let check_of = |w: &WorldRecord| Point::of(check_at(w, at));
                            let effects: Vec<f64> = sampled
                                .iter()
                                .map(|w| f64::from(check_of(w).effect))
                                .collect();
                            let nondeg: Vec<f64> = effects
                                .iter()
                                .zip(&degenerate)
                                .filter(|(_, d)| !**d)
                                .map(|(e, _)| *e)
                                .collect();
                            let ref_sign = sign(rc.direction);
                            let same = |w: &WorldRecord| {
                                ref_sign.is_some() && sign(check_of(w).direction) == ref_sign
                            };
                            let sensitivity = (options.mode == Mode::Global)
                                .then(|| sensitivity(varied, sampled, &degenerate, at));
                            CheckSummary {
                                label: rc.label,
                                bound: rc.bound_text.clone(),
                                known_limit: rc.known_limit,
                                reference_effect: rc.effect,
                                reference_in_band: rc.in_band,
                                in_band_share: share(sampled.iter().map(|w| check_of(w).in_band)),
                                in_band_share_nondegenerate: share(
                                    sampled
                                        .iter()
                                        .zip(&degenerate)
                                        .filter(|(_, d)| !**d)
                                        .map(|(w, _)| check_of(w).in_band),
                                ),
                                same_sign_share: share(sampled.iter().map(same)),
                                same_sign_share_nondegenerate: share(
                                    sampled
                                        .iter()
                                        .zip(&degenerate)
                                        .filter(|(_, d)| !**d)
                                        .map(|(w, _)| same(w)),
                                ),
                                median: stats::quantile(&effects, 0.5),
                                q05: stats::quantile(&effects, 0.05),
                                q95: stats::quantile(&effects, 0.95),
                                median_nondegenerate: stats::quantile(&nondeg, 0.5),
                                q05_nondegenerate: stats::quantile(&nondeg, 0.05),
                                q95_nondegenerate: stats::quantile(&nondeg, 0.95),
                                with_value: effects.iter().filter(|e| !e.is_nan()).count(),
                                sensitivity,
                            }
                        })
                        .collect();
                    ItemSummary {
                        id: item.id,
                        title: item.title,
                        reference_holds: ref_item.holds,
                        holds_share: share(sampled.iter().map(item_of)),
                        holds_share_nondegenerate: share(
                            sampled
                                .iter()
                                .zip(&degenerate)
                                .filter(|(_, d)| !**d)
                                .map(|(w, _)| item_of(w)),
                        ),
                        checks,
                    }
                })
                .collect();
            BatterySummary {
                battery: b.id,
                worlds: sampled.len(),
                degenerate_worlds: degenerate.iter().filter(|d| **d).count(),
                reference_degenerate: reference.batteries[bi].degenerate,
                items,
            }
        })
        .collect()
}

/// Spearman and, with enough worlds, PRCC between every varied continuous
/// assumption and a check's effect, over the non-degenerate worlds with a
/// value. The categorical assumptions enter PRCC as dummy controls and get
/// their median effect per choice.
fn sensitivity(
    varied: &[Varied],
    sampled: &[WorldRecord],
    degenerate: &[bool],
    at: Index,
) -> Sensitivity {
    let check_of = |w: &WorldRecord| Point::of(check_at(w, at));
    let used: Vec<&WorldRecord> = sampled
        .iter()
        .zip(degenerate)
        .filter(|(w, d)| !**d && !check_of(w).effect.is_nan())
        .map(|(w, _)| w)
        .collect();
    let y: Vec<f64> = used.iter().map(|w| f64::from(check_of(w).effect)).collect();
    let number = |w: &WorldRecord, path: &str| {
        w.values
            .get(short(path))
            .and_then(Json::as_f64)
            .unwrap_or(f64::NAN)
    };
    // An assumption whose range is one value (leave_threshold under the
    // revised ranges) is constant across worlds: it drives nothing.
    let continuous: Vec<&Varied> = varied
        .iter()
        .filter(|v| matches!(v.kind, Kind::Continuous { .. }) && v.spans())
        .collect();
    let inputs: Vec<Vec<f64>> = continuous
        .iter()
        .map(|v| used.iter().map(|w| number(w, v.info.path)).collect())
        .collect();
    let mut controls = Vec::new();
    let mut by_choice = BTreeMap::new();
    for v in varied {
        if let Kind::Categorical { choices, .. } = v.kind {
            let chosen: Vec<Option<&str>> = used
                .iter()
                .map(|w| w.values.get(short(v.info.path)).and_then(Json::as_str))
                .collect();
            for c in choices.iter().skip(1) {
                controls.push(
                    chosen
                        .iter()
                        .map(|x| f64::from(u8::from(*x == Some(c.id))))
                        .collect(),
                );
            }
            let medians = choices
                .iter()
                .map(|c| {
                    let e: Vec<f64> = chosen
                        .iter()
                        .zip(&y)
                        .filter(|(x, _)| **x == Some(c.id))
                        .map(|(_, e)| *e)
                        .collect();
                    (c.id, (e.len(), stats::quantile(&e, 0.5)))
                })
                .collect();
            by_choice.insert(short(v.info.path), medians);
        }
    }
    let n = used.len();
    let spearman: Vec<f64> = inputs.iter().map(|x| stats::spearman(x, &y)).collect();
    let enough = n > varied.len() + 10;
    let prcc = enough.then(|| stats::prcc(&inputs, &controls, &y));
    let all: Vec<Driver> = continuous
        .iter()
        .enumerate()
        .map(|(j, v)| Driver {
            assumption: short(v.info.path),
            spearman: spearman[j],
            prcc: prcc.as_ref().map(|p| p[j]),
        })
        .collect();
    let mut top = all.clone();
    let key = |d: &Driver| {
        let k = d.prcc.unwrap_or(d.spearman).abs();
        if k.is_nan() { -1.0 } else { k }
    };
    top.sort_by(|a, b| key(b).total_cmp(&key(a)));
    top.truncate(5);
    Sensitivity {
        worlds: n,
        note: (!enough).then(|| {
            format!(
                "PRCC needs more than {} usable worlds (the {} varied assumptions plus 10); {n} here, \
                 so Spearman alone",
                varied.len() + 10,
                varied.len()
            )
        }),
        top,
        all,
        by_choice,
    }
}

fn fmt(x: f64, digits: usize) -> String {
    if x.is_nan() {
        "-".into()
    } else {
        format!("{x:.digits$}")
    }
}

fn pct(x: f64) -> String {
    if x.is_nan() {
        "-".into()
    } else {
        format!("{:.0}%", 100.0 * x)
    }
}

/// The output directory: `--out`, or `exports/robustness/<mode>-<sample
/// seed>`, with `-documented` for the documented global ranges.
fn out_dir(options: &Options) -> PathBuf {
    options.out.clone().unwrap_or_else(|| {
        report::default_out(&format!(
            "robustness/{}-{}{}",
            options.mode.name(),
            options.sample_seed,
            match (options.mode, options.ranges) {
                (Mode::Global, Ranges::Documented) => "-documented",
                _ => "",
            }
        ))
    })
}

/// The command that reproduces the run, its output directory included.
fn command_text(options: &Options) -> String {
    let mut c = format!(
        "incentive robustness --mode {} --worlds {} --seeds {} --sample-seed {} --battery {}",
        options.mode.name(),
        options.worlds,
        options.seeds.max(1),
        options.sample_seed,
        options.battery.name()
    );
    match options.mode {
        Mode::Local => {
            let _ = write!(c, " --jitter {}", options.jitter);
        }
        Mode::Global => {
            let _ = write!(c, " --ranges {}", options.ranges.name());
        }
    }
    if let Some(p) = options.population {
        let _ = write!(c, " --population {p}");
    }
    if let Some(d) = options.days {
        let _ = write!(c, " --days {d}");
    }
    let _ = write!(c, " --out {}", report::out_arg(&out_dir(options)));
    c
}

/// The sampling rule of an assumption in this mode, as text.
fn rule_text(options: &Options, v: &Varied) -> (String, String) {
    match (options.mode, v.kind) {
        (
            Mode::Local,
            Kind::Continuous {
                default, catalogue, ..
            },
        ) => {
            if default == 0.0 {
                ("default (0: mechanism off)".into(), "-".into())
            } else {
                let (a, b) = (
                    default * (1.0 - options.jitter),
                    default * (1.0 + options.jitter),
                );
                let (lo, hi) = (a.min(b).max(catalogue.0), a.max(b).min(catalogue.1));
                (
                    format!(
                        "x U({:.2}, {:.2})",
                        1.0 - options.jitter,
                        1.0 + options.jitter
                    ),
                    format!("{} to {}", num(lo), num(hi)),
                )
            }
        }
        (Mode::Local, Kind::Categorical { .. }) => ("default (categorical)".into(), "-".into()),
        (Mode::Global, Kind::Continuous { global, source, .. }) => (
            format!("Latin hypercube, {}", source.label()),
            format!("{} to {}", num(global.0), num(global.1)),
        ),
        (Mode::Global, Kind::Categorical { choices, .. }) => (
            "uniform choice".into(),
            choices.iter().map(|c| c.id).collect::<Vec<_>>().join(", "),
        ),
    }
}

/// A number in its shortest form.
fn num(x: f64) -> String {
    let s = format!("{:.6}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn default_text(v: &Varied) -> String {
    match v.kind {
        Kind::Continuous { default, .. } => num(default),
        Kind::Categorical { choices, default } => choices[default].id.to_string(),
    }
}

fn print_header(
    options: &Options,
    varied: &[Varied],
    selected: &[Battery],
    runs_of: &dyn Fn(&Battery) -> FactRun,
) -> Result<(), CliError> {
    let sizes: Vec<String> = selected
        .iter()
        .map(|b| {
            let r = runs_of(b);
            format!("{} at N = {}, {} days", b.id, r.population, r.days)
        })
        .collect();
    outln!(
        "Robustness, {} mode: the default world and {} sampled worlds (sample seed {}), {} paired seeds; {}.",
        options.mode.name(),
        options.worlds,
        options.sample_seed,
        options.seeds.max(1),
        sizes.join("; ")
    );
    let names: Vec<&str> = varied.iter().map(|v| short(v.info.path)).collect();
    outln!(
        "Varied ({}): {}.\nKept at their defaults: {} (docs/model.md, 6.1 and 13.2), and every run and \
         design setting and the rest of the population.",
        varied.len(),
        names.join(", "),
        sample::EXCLUDED
            .iter()
            .map(|p| short(p))
            .collect::<Vec<_>>()
            .join(", ")
    );
    match options.mode {
        Mode::Local => {
            let kept: Vec<&str> = varied
                .iter()
                .filter(|v| !v.jittered())
                .map(|v| short(v.info.path))
                .collect();
            outln!(
                "Local: each varied number with a nonzero default times U({:.2}, {:.2}), clamped to its \
                 catalogue range; kept at their defaults: {} (default 0, a mechanism off, or categorical).\n",
                1.0 - options.jitter,
                1.0 + options.jitter,
                kept.join(", ")
            );
        }
        Mode::Global => outln!(
            "Global: a Latin hypercube over each number's range; categorical assumptions drawn uniformly. \
             {}\n",
            ranges_text(options.ranges, varied, "")
        ),
    }
    Ok(())
}

/// The global ranges in use, as a sentence listing the assumptions by where
/// their range comes from; `quote` wraps each name.
fn ranges_text(ranges: Ranges, varied: &[Varied], quote: &str) -> String {
    use sample::RangeSource as S;
    let rule = match ranges {
        Ranges::Documented => {
            "Ranges as documented in docs/model.md, section 11, before lab #31 (`--ranges \
            documented`)"
        }
        Ranges::Revised => {
            "Ranges revised by lab #31's rule (`--ranges revised`, docs/model.md, section 11)"
        }
    };
    let groups: Vec<String> = [
        S::Docs,
        S::DocsClipped,
        S::Catalogue,
        S::Scaled,
        S::DefaultThirds,
        S::Population,
    ]
    .iter()
    .filter_map(|&src| {
        let names: Vec<String> = varied
            .iter()
            .filter(|v| matches!(v.kind, Kind::Continuous { source, .. } if source == src))
            .map(|v| format!("{quote}{}{quote}", short(v.info.path)))
            .collect();
        match (src, names.is_empty()) {
            (_, true) => None,
            (S::Docs, false) => Some(format!("{} from section 11", names.len())),
            (_, false) => Some(format!("{}: {}", src.label(), names.join(", "))),
        }
    })
    .collect();
    let fixed: Vec<String> = varied
        .iter()
        .filter(|v| !v.spans())
        .map(|v| format!("{quote}{}{quote}", short(v.info.path)))
        .collect();
    let mut text = format!("{rule}: {}.", groups.join("; "));
    if !fixed.is_empty() {
        text.push_str(&format!(
            " A range of one value keeps {} at its default in every world.",
            fixed.join(", ")
        ));
    }
    text
}

fn print_summary(summary: &[BatterySummary]) -> Result<(), CliError> {
    for b in summary {
        outln!(
            "{}: {} sampled worlds, {} degenerate (user share below {DEGENERATE_USER_SHARE} on the baseline's \
             last day){}",
            b.battery,
            b.worlds,
            b.degenerate_worlds,
            if b.reference_degenerate {
                "; the default world is degenerate at this size"
            } else {
                ""
            }
        );
        let mut rows = Vec::new();
        for item in &b.items {
            rows.push(vec![
                format!("{}: holds", item.id),
                if item.reference_holds { "yes" } else { "no" }.into(),
                pct(item.holds_share),
                pct(item.holds_share_nondegenerate),
                String::new(),
                String::new(),
            ]);
            for c in &item.checks {
                rows.push(vec![
                    format!(
                        "  {}{}",
                        truncate(c.label, 64),
                        if c.known_limit { " (known limit)" } else { "" }
                    ),
                    format!(
                        "{} {}",
                        fmt(f64::from(c.reference_effect), 3),
                        if c.reference_in_band { "in" } else { "out" }
                    ),
                    pct(c.in_band_share),
                    pct(c.in_band_share_nondegenerate),
                    pct(c.same_sign_share),
                    format!(
                        "{} [{}, {}]",
                        fmt(c.median, 3),
                        fmt(c.q05, 3),
                        fmt(c.q95, 3)
                    ),
                ]);
            }
        }
        out!(
            "{}",
            table(
                &[
                    "item and check",
                    "default",
                    "holds",
                    "non-degen.",
                    "sign",
                    "median [5%, 95%]"
                ],
                &rows
            )
        );
        outln!("");
    }
    outln!(
        "\"default\" is the default world's effect and whether it lies in its band; \"holds\" the share of \
         sampled worlds in which the check lies in its band (a fact or claim: every check that is not a known \
         limit), also over the non-degenerate worlds; \"sign\" the share whose effect lies on the default \
         world's side of its neutral value (0, 1 for a ratio or for a ratio or share read as a mean); then \
         the effect's quantiles."
    );
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n - 3).collect::<String>())
    }
}

fn settings_json(
    options: &Options,
    varied: &[Varied],
    selected: &[Battery],
    runs_of: &dyn Fn(&Battery) -> FactRun,
) -> Json {
    let assumptions: Vec<Json> = varied
        .iter()
        .map(|v| {
            let (rule, range) = rule_text(options, v);
            let mut entry = json!({
                "assumption": short(v.info.path),
                "default": default_text(v),
                "rule": rule,
                "range": range,
            });
            if let Kind::Continuous {
                catalogue,
                global,
                source,
                ..
            } = v.kind
            {
                entry["catalogue_range"] = json!([catalogue.0, catalogue.1]);
                entry["global_range"] = json!([global.0, global.1]);
                entry["global_range_source"] = json!(source);
                entry["jittered_in_local_mode"] = json!(v.jittered());
            }
            entry
        })
        .collect();
    json!({
        "command": command_text(options),
        "mode": options.mode.name(),
        "worlds": options.worlds,
        "seeds": (1..=options.seeds.max(1)).collect::<Vec<u64>>(),
        "sample_seed": options.sample_seed,
        "jitter": (options.mode == Mode::Local).then_some(options.jitter),
        "ranges": (options.mode == Mode::Global).then_some(options.ranges),
        "batteries": selected.iter().map(|b| {
            let r = runs_of(b);
            json!({"battery": b.id, "population": r.population, "days": r.days,
                   "items": b.items.iter().map(|i| json!({"id": i.id, "title": i.title, "reference": i.reference}))
                       .collect::<Vec<_>>()})
        }).collect::<Vec<_>>(),
        "degenerate_user_share": DEGENERATE_USER_SHARE,
        "excluded": sample::EXCLUDED.iter().map(|p| short(p)).collect::<Vec<_>>(),
        "varied": assumptions,
    })
}

fn csv_cell(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn json_cell(v: Option<&Json>) -> String {
    match v {
        Some(Json::String(s)) => csv_cell(s),
        Some(Json::Number(n)) => n.to_string(),
        Some(other) => csv_cell(&other.to_string()),
        None => String::new(),
    }
}

fn f32_cell(x: f32) -> String {
    if x.is_nan() {
        String::new()
    } else {
        x.to_string()
    }
}

/// One row per world: its values, then each battery's baseline user share
/// and degenerate flag.
fn worlds_csv(varied: &[Varied], records: &[WorldRecord]) -> String {
    let mut out = String::from("world,reference");
    for v in varied {
        let _ = write!(out, ",{}", short(v.info.path));
    }
    if let Some(first) = records.first() {
        for b in &first.batteries {
            let _ = write!(out, ",{0}_baseline_user_share,{0}_degenerate", b.battery);
        }
    }
    out.push('\n');
    for w in records {
        let _ = write!(out, "{},{}", w.world, w.reference);
        for v in varied {
            let _ = write!(out, ",{}", json_cell(w.values.get(short(v.info.path))));
        }
        for b in &w.batteries {
            let _ = write!(out, ",{},{}", f32_cell(b.baseline_user_share), b.degenerate);
        }
        out.push('\n');
    }
    out
}

/// One row per world and check; per-seed effects joined by `;`.
fn checks_csv(records: &[WorldRecord]) -> String {
    let mut out = String::from(
        "world,reference,battery,degenerate,item,item_holds,check,label,known_limit,effect,direction,low,high,\
         in_band,per_seed\n",
    );
    for w in records {
        for b in &w.batteries {
            for item in &b.items {
                for (ci, c) in item.checks.iter().enumerate() {
                    let (low, high) = open_ends(c.bound);
                    let per_seed: Vec<String> = c.per_seed.iter().map(|x| f32_cell(*x)).collect();
                    let _ = writeln!(
                        out,
                        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                        w.world,
                        w.reference,
                        b.battery,
                        b.degenerate,
                        item.id,
                        item.holds,
                        ci + 1,
                        csv_cell(c.label),
                        c.known_limit,
                        f32_cell(c.effect),
                        f32_cell(c.direction),
                        low,
                        high,
                        c.in_band,
                        per_seed.join(";")
                    );
                }
            }
        }
    }
    out
}

fn summary_md(
    options: &Options,
    varied: &[Varied],
    selected: &[Battery],
    runs_of: &dyn Fn(&Battery) -> FactRun,
    summary: &[BatterySummary],
) -> String {
    let mut md = String::from("# Robustness\n\n");
    let sizes: Vec<String> = selected
        .iter()
        .map(|b| {
            let r = runs_of(b);
            format!("{} at `N = {}`, {} days", b.id, r.population, r.days)
        })
        .collect();
    let _ = writeln!(
        md,
        "Generated by `{}`: the default world (world 0, the reference) and {} sampled worlds, seeds 1 to {} \
         paired across variants; {}. A check holds in a world when its effect on the seed means lies in its \
         band; a fact or claim when every check that is not a known limit holds. A world is degenerate when \
         the default design's user share on its last day, averaged over the seeds, lies below {}.\n",
        command_text(options),
        options.worlds,
        options.seeds.max(1),
        sizes.join("; "),
        DEGENERATE_USER_SHARE
    );
    let _ = writeln!(
        md,
        "## Varied assumptions\n\n{} varied: the world assumptions and two population settings, the troll \
         share and the bot share. Kept at their defaults: {} (the checking habits and the interruption cost, \
         off by the owner's decision, docs/model.md 6.1 and 13.2), and every run and design setting and the \
         rest of the population.{}\n",
        varied.len(),
        sample::EXCLUDED
            .iter()
            .map(|p| format!("`{}`", short(p)))
            .collect::<Vec<_>>()
            .join(", "),
        match options.mode {
            Mode::Local => format!(
                " Local mode multiplies each number with a nonzero default by U({:.2}, {:.2}) and clamps it \
                 to its catalogue range; assumptions whose default is 0 (a mechanism off) and categorical \
                 ones keep their defaults.",
                1.0 - options.jitter,
                1.0 + options.jitter
            ),
            Mode::Global => format!(
                " Global mode draws a Latin hypercube over each number's range and draws categorical \
                 assumptions uniformly. {}",
                ranges_text(options.ranges, varied, "`")
            ),
        }
    );
    md.push_str("| Assumption | Default | Rule | Range |\n| --- | --- | --- | --- |\n");
    for v in varied {
        let (rule, range) = rule_text(options, v);
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} |",
            short(v.info.path),
            default_text(v),
            rule,
            range
        );
    }
    for b in summary {
        let _ = writeln!(
            md,
            "\n## Battery: {}\n\n{} sampled worlds, {} degenerate{}. \"Default\" is the default world's effect; \
             \"holds\" the share of sampled worlds where the check, fact or claim holds, over all and over \
             the non-degenerate worlds; \"sign\" the share whose effect lies on the default world's side of \
             its neutral value (0, 1 for a ratio or for a ratio or share read as a mean); then the effect's median and 5% and 95% quantiles over \
             all sampled worlds.\n",
            b.battery,
            b.worlds,
            b.degenerate_worlds,
            if b.reference_degenerate {
                "; the default world itself is degenerate at this size"
            } else {
                ""
            }
        );
        md.push_str(
            "| Item | Check | Bound | Default | Holds | Non-degenerate | Sign | Median | 5% | 95% |\n\
             | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for item in &b.items {
            let _ = writeln!(
                md,
                "| **{}** | {} | | {} | {} | {} | | | | |",
                item.id,
                item.title,
                if item.reference_holds {
                    "holds"
                } else {
                    "fails"
                },
                pct(item.holds_share),
                pct(item.holds_share_nondegenerate)
            );
            for c in &item.checks {
                let _ = writeln!(
                    md,
                    "| | {}{} | {} | {:.4} ({}) | {} | {} | {} | {} | {} | {} |",
                    c.label,
                    if c.known_limit { " (known limit)" } else { "" },
                    c.bound,
                    c.reference_effect,
                    if c.reference_in_band { "in" } else { "out" },
                    pct(c.in_band_share),
                    pct(c.in_band_share_nondegenerate),
                    pct(c.same_sign_share),
                    fmt(c.median, 4),
                    fmt(c.q05, 4),
                    fmt(c.q95, 4)
                );
            }
        }
        if options.mode == Mode::Global {
            let _ = writeln!(
                md,
                "\n### Sensitivity: {}\n\nPer check, the five varied assumptions with the largest absolute \
                 partial rank correlation (PRCC) with the effect, or the largest absolute Spearman \
                 correlation where there are too few worlds for PRCC, over the non-degenerate worlds with a \
                 value. Every assumption's values are in summary.json.\n",
                b.battery
            );
            for item in &b.items {
                for c in &item.checks {
                    let Some(s) = &c.sensitivity else { continue };
                    let _ = writeln!(md, "**{}**, {} ({} worlds)", item.id, c.label, s.worlds);
                    if let Some(note) = &s.note {
                        let _ = writeln!(md, "\n{note}.");
                    }
                    md.push_str("\n| Assumption | PRCC | Spearman |\n| --- | --- | --- |\n");
                    for d in &s.top {
                        let _ = writeln!(
                            md,
                            "| `{}` | {} | {} |",
                            d.assumption,
                            d.prcc.map_or("-".into(), |p| fmt(p, 3)),
                            fmt(d.spearman, 3)
                        );
                    }
                    for (name, choices) in &s.by_choice {
                        let parts: Vec<String> = choices
                            .iter()
                            .map(|(id, (n, m))| format!("{id} {} ({n})", fmt(*m, 4)))
                            .collect();
                        let _ = writeln!(md, "\nMedian effect by `{name}`: {}.", parts.join(", "));
                    }
                    md.push('\n');
                }
            }
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batteries_select_and_size() {
        assert_eq!(batteries(BatteryChoice::Facts).len(), 1);
        assert_eq!(batteries(BatteryChoice::Headline)[0].items.len(), 8);
        let validation = batteries(BatteryChoice::Validation);
        assert_eq!(validation[0].items.len(), 10);
        assert!(std::ptr::eq(validation[0].items, validation_battery()));
        let all = batteries(BatteryChoice::All);
        assert_eq!(
            all.iter().map(|b| b.id).collect::<Vec<_>>(),
            ["facts", "headline", "validation"]
        );
        assert!(all.iter().all(|b| b.population == 2_000 && b.days == 365));
    }

    /// The "Generated by" command names the output directory, relative to
    /// the repository for the default one.
    #[test]
    fn the_command_text_names_the_output_directory() {
        let mut options = Options {
            mode: Mode::Global,
            worlds: 64,
            seeds: 2,
            sample_seed: 2,
            battery: BatteryChoice::All,
            population: None,
            days: None,
            jitter: 0.25,
            ranges: Ranges::Documented,
            out: None,
        };
        assert!(
            command_text(&options).ends_with(
                "--battery all --ranges documented --out exports/robustness/global-2-documented"
            ),
            "{}",
            command_text(&options)
        );
        options.out = Some(PathBuf::from("/elsewhere/run"));
        assert!(command_text(&options).ends_with(" --out /elsewhere/run"));
    }

    /// The command's help lists exactly the varied set and the excluded
    /// assumptions.
    #[test]
    fn help_lists_the_varied_set() {
        use clap::CommandFactory;
        let command = crate::cli::Cli::command();
        let help = command
            .find_subcommand("robustness")
            .and_then(|c| c.get_long_about())
            .map(ToString::to_string)
            .unwrap_or_default();
        let (listed, kept) = help
            .split_once("Kept at their defaults")
            .unwrap_or_default();
        let varied = sample::varied(Ranges::Revised).unwrap_or_default();
        assert!(listed.contains(&format!("Varied, {} assumptions", varied.len())));
        // Each name, or for a population setting the part before its dot.
        let expected: Vec<&str> = varied
            .iter()
            .map(|v| short(v.info.path).split('.').next().unwrap_or_default())
            .collect();
        let words: Vec<&str> = listed
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| w.contains('_') || expected.contains(w))
            .collect();
        for v in &varied {
            assert!(listed.contains(short(v.info.path)), "{}", v.info.path);
        }
        assert_eq!(words.len(), varied.len(), "{words:?}");
        for path in sample::EXCLUDED {
            assert!(kept.contains(short(path)), "{path}");
        }
    }

    #[test]
    fn numbers_print_short() {
        assert_eq!(num(0.25), "0.25");
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(0.00015), "0.00015");
    }
}
