//! `report`: regenerates the calibration tables of docs/model.md (section 13)
//! as Markdown ready to paste: the presets at the long and the short horizon,
//! named paired comparisons with 95% intervals, the stylized facts and the
//! scenarios' verdicts. Every run of every table is collected first, run
//! once (identical runs are shared between tables) and in parallel, so the
//! report is deterministic for its options.

use std::fmt::Write as _;
use std::path::PathBuf;

use incentive_core::{
    Config, DailyMetrics, FactRun, Format, MOVING_STANDARD_ERRORS, MOVING_THRESHOLD, MetricId,
    PairedDifference, Stats, Verdict, Visibility, facts, preset, presets, scenarios,
};

use crate::error::CliError;
use crate::report::{self, fmt_value};
use crate::runs::{self, RunOutput, parse_seeds};

/// The short horizon of the preset table, in days: a field experiment's.
const SHORT_HORIZON: u32 = 90;
/// The long horizon unless `--days` gives another.
const LONG_HORIZON: u32 = 365;

/// The rows of the preset tables, with their labels in section 13.3.
const PRESET_ROWS: [(MetricId, &str); 22] = [
    (MetricId::EngagementIndex, "Engagement index (O)"),
    (MetricId::HarmIndex, "Harm index"),
    (
        MetricId::HarmIndexExPolarisation,
        "Harm index without polarisation",
    ),
    (MetricId::DiscourseQuality, "Discourse quality"),
    (MetricId::BenefitIndex, "Benefit index"),
    (
        MetricId::BenefitIndexExCreators,
        "Benefit index without creators",
    ),
    (MetricId::UserShare, "Users in society"),
    (MetricId::HumanDailyActives, "Human daily actives"),
    (MetricId::Impressions, "Impressions (O)"),
    (MetricId::NetRevenue, "Net revenue (O)"),
    (MetricId::ToxicShare, "Toxic impressions"),
    (MetricId::OutrageShare, "Outrage impressions"),
    (MetricId::MisinformationShare, "Misinformation impr."),
    (MetricId::HarassmentPer1000, "Harassment per 1,000"),
    (MetricId::PostShareOutrage, "Outrage posts"),
    (MetricId::WellbeingEverUsers, "Wellbeing, ever-users"),
    (MetricId::SurveySatisfaction, "Survey (O)"),
    (MetricId::PolarisationSociety, "Polarisation"),
    (MetricId::InformednessNonUsers, "Informed non-users"),
    (MetricId::CreatorsLiving, "Creators living"),
    (MetricId::HarmfulPayoutShare, "Harmful payouts"),
    (MetricId::BotEngagementShare, "Bot engagement"),
];

/// A preset's column label in the tables.
fn preset_label(id: &str) -> &str {
    match id {
        "engagement-maximiser" => "ER",
        "chronological" => "Chrono",
        "quality-bridging" => "Q&B",
        "calm-by-design" => "Calm",
        "subscription" => "Sub",
        "engagement-paid-creators" => "Paid",
        "integrity-ranking" => "Integrity",
        "max-engagement" => "Max",
        other => other,
    }
}

/// One side of a named comparison: a preset and assignments on top of it.
struct Side {
    preset: &'static str,
    set: &'static [&'static str],
}

/// Which days a comparison averages.
#[derive(Clone, Copy)]
enum Window {
    /// The run's summary window.
    Summary,
    /// The first days of the run.
    First(u32),
    /// The last days of the run.
    Last(u32),
}

/// A named paired comparison: A and B on the same seeds, with the
/// assignments in `both` applied to both sides (a world, for example).
struct Named {
    label: &'static str,
    a: Side,
    b: Side,
    both: &'static [&'static str],
    window: Window,
    metrics: &'static [MetricId],
    /// Whether the ratio of the means B / A is worth showing.
    ratio: bool,
}

const ER: &str = "engagement-maximiser";
const QB: &str = "quality-bridging";
const TRAP_SOLUTION: &[&str] = &[
    "design.w_reply=0",
    "design.review_capacity=10",
    "design.report_trust=1",
];
const OLD_TRAP_SOLUTION: &[&str] = &["design.w_reply=0", "design.w_see_less=5"];
const NO_TOXICITY_COST: &[&str] = &[
    "world.churn_base=0",
    "world.harassment_exit=0",
    "world.reputation_sensitivity=0",
];
const WEAK_HABIT: &[&str] = &[
    "world.habit_gain=0.01",
    "world.reputation_sensitivity=5",
    "world.harassment_exit=0.15",
];
const EVERYONE_BRIDGING: &[&str] = &["design.alt_feed_share=1", "design.alt_feed=bridging"];
const PLAIN_ER: Side = Side {
    preset: ER,
    set: &[],
};
const PLAIN_QB: Side = Side {
    preset: QB,
    set: &[],
};
const THRESHOLD_03: Side = Side {
    preset: ER,
    set: &["design.classifier_threshold=0.3"],
};
const BRIDGING_FEED: Side = Side {
    preset: ER,
    set: EVERYONE_BRIDGING,
};
const TOXIC_EFFECTS: &[MetricId] = &[
    MetricId::ToxicShare,
    MetricId::FalsePositives,
    MetricId::HarmIndex,
];
const TRAP_EFFECTS: &[MetricId] = &[
    MetricId::EngagementIndex,
    MetricId::ToxicShare,
    MetricId::FalsePositives,
    MetricId::HarmIndex,
];
const SEE_LESS_EFFECTS: &[MetricId] = &[
    MetricId::EngagementIndex,
    MetricId::ToxicShare,
    MetricId::HarmIndex,
    MetricId::BotFlagShare,
];
const POLISH_EFFECTS: &[MetricId] = &[
    MetricId::HarmIndex,
    MetricId::ToxicShare,
    MetricId::MisinformationShare,
    MetricId::PolishedShare,
];
const TRUST_EFFECTS: &[MetricId] = &[
    MetricId::ToxicShare,
    MetricId::HarassmentPer1000,
    MetricId::BotFlagShare,
    MetricId::HarmIndex,
];
const CHRONO_EFFECTS: &[MetricId] = &[
    MetricId::EngagementIndex,
    MetricId::TimeSpent,
    MetricId::ToxicShare,
    MetricId::OutrageShare,
    MetricId::MisinformationShare,
    MetricId::HarmIndex,
    MetricId::BenefitIndex,
    MetricId::PolarisationSociety,
];
const INTEGRITY: Side = Side {
    preset: "integrity-ranking",
    set: &[],
};
const LEAD: &[MetricId] = &[MetricId::EngagementIndex, MetricId::UserShare];
const BOT_GAMING: &[MetricId] = &[
    MetricId::MisinformationShare,
    MetricId::ToxicShare,
    MetricId::BotEngagementShare,
];

/// The named comparisons of the report, in order. Edit this table to add or
/// change one; every number of section 13.3 comes from a row here or from the
/// preset tables.
const COMPARISONS: &[Named] = &[
    Named {
        label: "Engagement-trap solution against the engagement ranking",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: TRAP_SOLUTION,
        },
        both: &[],
        window: Window::Summary,
        metrics: TRAP_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`w_reply` 0 alone",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.w_reply=0"],
        },
        both: &[],
        window: Window::Summary,
        metrics: TRAP_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`classifier_threshold` 0.55 alone",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.classifier_threshold=0.55"],
        },
        both: &[],
        window: Window::Summary,
        metrics: TRAP_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Trap solution without `report_trust` (`w_reply` 0, `review_capacity` 10)",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.w_reply=0", "design.review_capacity=10"],
        },
        both: &[],
        window: Window::Summary,
        metrics: TRAP_EFFECTS,
        ratio: true,
    },
    Named {
        label: "v0.1 trap solution (`w_reply` 0, `w_see_less` 5), bots brigading (default)",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: OLD_TRAP_SOLUTION,
        },
        both: &[],
        window: Window::Summary,
        metrics: SEE_LESS_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`w_see_less` 5 alone, bots brigading (default)",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.w_see_less=5"],
        },
        both: &[],
        window: Window::Summary,
        metrics: SEE_LESS_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`w_see_less` 5 alone, `bot_brigading` 0",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.w_see_less=5"],
        },
        both: &["world.bot_brigading=0"],
        window: Window::Summary,
        metrics: SEE_LESS_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Trap solution with no cost of toxicity (`churn_base`, `harassment_exit`, \
                `reputation_sensitivity` 0)",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: TRAP_SOLUTION,
        },
        both: NO_TOXICITY_COST,
        window: Window::Summary,
        metrics: &[MetricId::EngagementIndex, MetricId::ToxicShare],
        ratio: true,
    },
    Named {
        label: "`classifier_threshold` 0.3, `evasion_rate` 0.15 (default)",
        a: PLAIN_ER,
        b: THRESHOLD_03,
        both: &[],
        window: Window::Summary,
        metrics: TOXIC_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`classifier_threshold` 0.3, `evasion_rate` 0.05",
        a: PLAIN_ER,
        b: THRESHOLD_03,
        both: &["world.evasion_rate=0.05"],
        window: Window::Summary,
        metrics: TOXIC_EFFECTS,
        ratio: true,
    },
    Named {
        label: "`classifier_threshold` 0.3, `evasion_rate` 0.15, `classifier_retraining` 0",
        a: PLAIN_ER,
        b: THRESHOLD_03,
        both: &["world.classifier_retraining=0"],
        window: Window::Summary,
        metrics: TOXIC_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Engagement ranking against Q&B, default world, first 30 days",
        a: PLAIN_QB,
        b: PLAIN_ER,
        both: &[],
        window: Window::First(30),
        metrics: LEAD,
        ratio: true,
    },
    Named {
        label: "Engagement ranking against Q&B, default world, last 95 days",
        a: PLAIN_QB,
        b: PLAIN_ER,
        both: &[],
        window: Window::Last(95),
        metrics: LEAD,
        ratio: true,
    },
    Named {
        label: "Weak-habit lead (`habit_gain` 0.01, `reputation_sensitivity` 5, `harassment_exit` 0.15), \
                first 30 days",
        a: PLAIN_QB,
        b: PLAIN_ER,
        both: WEAK_HABIT,
        window: Window::First(30),
        metrics: LEAD,
        ratio: true,
    },
    Named {
        label: "Weak-habit lead, last 95 days",
        a: PLAIN_QB,
        b: PLAIN_ER,
        both: WEAK_HABIT,
        window: Window::Last(95),
        metrics: LEAD,
        ratio: true,
    },
    Named {
        label: "Everyone on the bridging alternative feed, `bot_strategy` shared-pool",
        a: PLAIN_ER,
        b: BRIDGING_FEED,
        both: &["world.bot_strategy=shared-pool"],
        window: Window::Summary,
        metrics: BOT_GAMING,
        ratio: false,
    },
    Named {
        label: "Everyone on the bridging alternative feed, `bot_strategy` own-side",
        a: PLAIN_ER,
        b: BRIDGING_FEED,
        both: &["world.bot_strategy=own-side"],
        window: Window::Summary,
        metrics: BOT_GAMING,
        ratio: false,
    },
    Named {
        label: "Everyone on the bridging alternative feed, `bot_strategy` adaptive (default)",
        a: PLAIN_ER,
        b: BRIDGING_FEED,
        both: &["world.bot_strategy=adaptive"],
        window: Window::Summary,
        metrics: BOT_GAMING,
        ratio: false,
    },
    Named {
        label: "Q&B against the engagement ranking, `polish_gain` 0",
        a: PLAIN_ER,
        b: PLAIN_QB,
        both: &["world.polish_gain=0"],
        window: Window::Summary,
        metrics: POLISH_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Q&B against the engagement ranking, `polish_gain` 0.3 (default)",
        a: PLAIN_ER,
        b: PLAIN_QB,
        both: &[],
        window: Window::Summary,
        metrics: POLISH_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Q&B against the engagement ranking, `polish_gain` 0.6",
        a: PLAIN_ER,
        b: PLAIN_QB,
        both: &["world.polish_gain=0.6"],
        window: Window::Summary,
        metrics: POLISH_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Q&B without its quality weight (`w_quality` 0) against Q&B",
        a: PLAIN_QB,
        b: Side {
            preset: QB,
            set: &["design.w_quality=0"],
        },
        both: &[],
        window: Window::Summary,
        metrics: POLISH_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Calm by design against the engagement ranking",
        a: PLAIN_ER,
        b: Side {
            preset: "calm-by-design",
            set: &[],
        },
        both: &[],
        window: Window::Summary,
        metrics: &[
            MetricId::FalsePositives,
            MetricId::HarmIndex,
            MetricId::HarmIndexExPolarisation,
            MetricId::MisinformationShare,
        ],
        ratio: false,
    },
    Named {
        label: "`account_cost` 1 against 0.2",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.account_cost=1"],
        },
        both: &[],
        window: Window::Summary,
        metrics: &[
            MetricId::BotEngagementShare,
            MetricId::MisinformationShare,
            MetricId::HarmIndex,
            MetricId::EngagementIndex,
            MetricId::UserShare,
        ],
        ratio: false,
    },
    Named {
        label: "`report_trust` 1 against 0",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.report_trust=1"],
        },
        both: &[],
        window: Window::Summary,
        metrics: TRUST_EFFECTS,
        ratio: false,
    },
    Named {
        label: "`report_trust` 1 against 0 under Q&B",
        a: PLAIN_QB,
        b: Side {
            preset: QB,
            set: &["design.report_trust=1"],
        },
        both: &[],
        window: Window::Summary,
        metrics: TRUST_EFFECTS,
        ratio: false,
    },
    Named {
        label: "Survey taken right after sessions (`survey_after_session` 1 against 0)",
        a: PLAIN_ER,
        b: Side {
            preset: ER,
            set: &["design.survey_after_session=1"],
        },
        both: &[],
        window: Window::Summary,
        metrics: &[MetricId::SurveySatisfaction, MetricId::WellbeingUsers],
        ratio: false,
    },
    Named {
        label: "Chronological against the engagement ranking, over the year",
        a: PLAIN_ER,
        b: Side {
            preset: "chronological",
            set: &[],
        },
        both: &[],
        window: Window::Summary,
        metrics: &[
            MetricId::TimeSpent,
            MetricId::PolarisationSociety,
            MetricId::ToxicShare,
            MetricId::MisinformationShare,
        ],
        ratio: true,
    },
    Named {
        label: "Chronological against the engagement ranking with integrity, first 90 days",
        a: INTEGRITY,
        b: Side {
            preset: "chronological",
            set: &[],
        },
        both: &[],
        window: Window::First(90),
        metrics: CHRONO_EFFECTS,
        ratio: true,
    },
    Named {
        label: "Chronological against the engagement ranking with integrity",
        a: INTEGRITY,
        b: Side {
            preset: "chronological",
            set: &[],
        },
        both: &[],
        window: Window::Summary,
        metrics: CHRONO_EFFECTS,
        ratio: true,
    },
];

/// Every run the report needs, grouped: a group is one configuration on
/// every seed.
struct Plan {
    seeds: Vec<u64>,
    jobs: Vec<(Config, u64)>,
}

impl Plan {
    /// Adds a configuration on every seed; returns its group.
    fn add(&mut self, config: Config) -> usize {
        let group = self.jobs.len() / self.seeds.len();
        for &s in &self.seeds {
            self.jobs.push((config.clone(), s));
        }
        group
    }
}

/// The finished runs, by group.
struct Done {
    outputs: Vec<RunOutput>,
    index: Vec<usize>,
    seeds: usize,
}

impl Done {
    /// The runs of a group, in seed order.
    fn group(&self, group: usize) -> Vec<&RunOutput> {
        self.index[group * self.seeds..(group + 1) * self.seeds]
            .iter()
            .map(|&i| &self.outputs[i])
            .collect()
    }
}

/// Sizes a configuration: days, population, no tracked agents.
fn sized(mut config: Config, days: u32, population: Option<u32>) -> Result<Config, CliError> {
    config.set_path("run.days", f64::from(days))?;
    if let Some(n) = population {
        config.set_path("run.population_size", f64::from(n))?;
    }
    config.run.tracked_agents = 0;
    config.validate()?;
    Ok(config)
}

/// A side's configuration with the shared assignments.
fn side_config(
    side: &Side,
    both: &[&str],
    days: u32,
    population: Option<u32>,
) -> Result<Config, CliError> {
    let mut config = preset(side.preset)?.config();
    for assignment in both.iter().chain(side.set) {
        config.set_assignment(assignment)?;
    }
    sized(config, days, population)
}

/// A side as text: the preset and its changes.
fn side_text(side: &Side, both: &[&str]) -> String {
    let changes: Vec<String> = both
        .iter()
        .chain(side.set)
        .map(|a| {
            let (path, value) = a.split_once('=').unwrap_or((a, ""));
            let name = path.rsplit('.').next().unwrap_or(path);
            format!("`{name}` {value}")
        })
        .collect();
    let name = preset(side.preset).map_or(side.preset, |p| p.name);
    if changes.is_empty() {
        name.to_string()
    } else {
        format!("{name} with {}", changes.join(", "))
    }
}

/// The mean of a metric over a window of one run.
fn window_value(run: &RunOutput, id: MetricId, window: Window) -> f32 {
    let days: &[DailyMetrics] = &run.history;
    let n = days.len();
    let slice = match window {
        Window::Summary => return run.summary.get(id),
        Window::First(d) => &days[..(d as usize).min(n)],
        Window::Last(d) => &days[n.saturating_sub(d as usize)..],
    };
    Stats::of(slice.iter().map(|d| d.get(id))).mean
}

/// A metric's value for a Markdown table: mean ± sd with the moving mark.
fn md_stat(id: MetricId, s: Stats, moving: bool) -> String {
    let mark = if moving { report::MOVING_MARK } else { "" };
    if s.mean.is_nan() {
        return "-".into();
    }
    if s.mean == 0.0 && s.sd == 0.0 {
        return format!("0{mark}");
    }
    let decimals = match id.info().format {
        Format::Count => 0,
        Format::Decimal | Format::Money if s.mean.abs() >= 1.0 => 2,
        _ => 3,
    };
    format!("{:.*}±{:.*}{mark}", decimals, s.mean, decimals, s.sd)
}

/// A Markdown table.
fn md_table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("| {} |\n", header.join(" | "));
    let _ = writeln!(
        out,
        "| {} |",
        header.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    );
    for row in rows {
        let _ = writeln!(out, "| {} |", row.join(" | "));
    }
    out
}

/// A signed value at a metric's precision.
fn signed(id: MetricId, v: f32) -> String {
    let text = fmt_value(id, v);
    if v >= 0.0 && !v.is_nan() {
        format!("+{text}")
    } else {
        text
    }
}

/// `report`: runs everything and writes report.md.
pub fn report(
    seeds: &str,
    population: Option<u32>,
    days: Option<u32>,
    out: Option<PathBuf>,
) -> Result<(), CliError> {
    let seeds = parse_seeds(seeds)?;
    let long = days.unwrap_or(LONG_HORIZON);
    let short = SHORT_HORIZON.min(long);
    let n = population.unwrap_or_else(|| Config::default().run.population_size);
    let mut plan = Plan {
        seeds: seeds.clone(),
        jobs: Vec::new(),
    };
    let horizons = [long, short];
    let mut preset_groups = Vec::new();
    for &h in &horizons {
        let mut row = Vec::new();
        for p in presets() {
            row.push(plan.add(sized(p.config(), h, population)?));
        }
        preset_groups.push(row);
    }
    let mut comparison_groups = Vec::new();
    for c in COMPARISONS {
        let a = plan.add(side_config(&c.a, c.both, long, population)?);
        let b = plan.add(side_config(&c.b, c.both, long, population)?);
        comparison_groups.push((a, b));
    }
    let fact_run = FactRun {
        seeds: seeds.clone(),
        population: n,
        days: long,
    };
    let mut fact_groups = Vec::new();
    for fact in facts() {
        let configs = fact.configs(&fact_run, seeds[0]);
        let groups: Vec<usize> = configs.into_iter().map(|c| plan.add(c)).collect();
        fact_groups.push(groups);
    }
    let mut scenario_groups = Vec::new();
    for sc in scenarios() {
        let scenario_days = days.unwrap_or(sc.config().run.days);
        let baseline = sized(sc.config(), scenario_days, population)?;
        let player = sized(
            sc.player_config(&sc.known_solution()?),
            scenario_days,
            population,
        )?;
        scenario_groups.push((plan.add(player), plan.add(baseline)));
    }
    let (outputs, index) = runs::run_unique(&plan.jobs)?;
    eprintln!(
        "{} runs ({} distinct) on {} seeds",
        plan.jobs.len(),
        outputs.len(),
        seeds.len()
    );
    let done = Done {
        outputs,
        index,
        seeds: seeds.len(),
    };

    let seed_text = match (seeds.first(), seeds.last()) {
        (Some(a), Some(b)) if seeds.len() > 1 && b - a + 1 == seeds.len() as u64 => {
            format!("{a}..{b}")
        }
        _ => seeds
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(","),
    };
    let mut command = format!("incentive report --seeds {seed_text}");
    if let Some(p) = population {
        let _ = write!(command, " --population {p}");
    }
    if let Some(d) = days {
        let _ = write!(command, " --days {d}");
    }
    let mut md = String::from("# Calibration report\n\n");
    let _ = writeln!(
        md,
        "Generated by `{command}`: seeds {seed_text} ({} paired seeds), `N = {n}`. Means ± standard \
         deviation across seeds; (O) marks observable metrics, the rest are truth. `~` marks a metric \
         that is still moving: its least-squares change across the summary window, averaged over the seeds, \
         exceeds {:.0}% of its mean (of its absolute value, at least 0.01, or 1 for counts) and {:.0} \
         standard errors of that average across the seeds.\n",
        seeds.len(),
        MOVING_THRESHOLD * 100.0,
        MOVING_STANDARD_ERRORS
    );

    for (h, groups) in horizons.iter().zip(&preset_groups) {
        let window = done.group(groups[0]).first().map_or(0, |r| r.summary.days);
        let _ = writeln!(
            md,
            "## Presets over {h} days\n\n{h} days, summary over the last {window} days.\n"
        );
        let mut header = vec!["Metric"];
        header.extend(presets().iter().map(|p| preset_label(p.id)));
        let rows: Vec<Vec<String>> = PRESET_ROWS
            .iter()
            .map(|&(id, label)| {
                let mut row = vec![label.to_string()];
                for &g in groups {
                    let runs = done.group(g);
                    let s = Stats::of(runs.iter().map(|r| r.summary.get(id)));
                    let moving = runs::moving_refs(&runs, id);
                    row.push(md_stat(id, s, moving));
                }
                row
            })
            .collect();
        md.push_str(&md_table(&header, &rows));
        md.push('\n');
    }

    let _ = writeln!(
        md,
        "## Paired comparisons\n\n{long} days, the summary window unless the comparison names days. \
         B - A is the mean paired difference with its 95% interval (Student's t over the seeds); B / A \
         is the ratio of the means.\n"
    );
    let mut rows = Vec::new();
    for (i, (c, &(ga, gb))) in COMPARISONS.iter().zip(&comparison_groups).enumerate() {
        let (ra, rb) = (done.group(ga), done.group(gb));
        for (k, &id) in c.metrics.iter().enumerate() {
            let values = |runs: &[&RunOutput]| -> Vec<f32> {
                runs.iter().map(|r| window_value(r, id, c.window)).collect()
            };
            let d = PairedDifference::of(&values(&ra), &values(&rb));
            let moving = |runs: &[&RunOutput]| {
                matches!(c.window, Window::Summary) && runs::moving_refs(runs, id)
            };
            let mark = |m: bool| if m { report::MOVING_MARK } else { "" };
            let ratio = if c.ratio && d.mean_a.abs() > 1e-9 {
                format!("{:.3}", d.mean_b / d.mean_a)
            } else {
                "-".into()
            };
            let name = if id.info().visibility == Visibility::Observable {
                format!("{} (O)", id.info().name)
            } else {
                id.info().name.to_string()
            };
            rows.push(vec![
                if k == 0 {
                    format!("{}. {}", i + 1, c.label)
                } else {
                    String::new()
                },
                name,
                format!("{}{}", fmt_value(id, d.mean_a), mark(moving(&ra))),
                format!("{}{}", fmt_value(id, d.mean_b), mark(moving(&rb))),
                signed(id, d.diff),
                format!("{} to {}", signed(id, d.low), signed(id, d.high)),
                ratio,
            ]);
        }
    }
    md.push_str(&md_table(
        &[
            "Comparison",
            "Metric",
            "A",
            "B",
            "B - A",
            "95% interval",
            "B / A",
        ],
        &rows,
    ));
    md.push_str("\nThe sides of each comparison:\n\n");
    for (i, c) in COMPARISONS.iter().enumerate() {
        let window = match c.window {
            Window::Summary => String::new(),
            Window::First(d) => format!("; first {d} days"),
            Window::Last(d) => format!("; last {d} days"),
        };
        let _ = writeln!(
            md,
            "{}. A: {}; B: {}{window}.",
            i + 1,
            side_text(&c.a, c.both),
            side_text(&c.b, c.both)
        );
    }
    md.push('\n');

    let _ = writeln!(
        md,
        "## Stylized facts\n\n{} paired seeds, `N = {n}`, {long} days. Effects are paired mean \
         differences, means, ratios of means or relative differences, ± the standard deviation of the \
         per-seed effect. The interval is the 95% t-interval of the per-seed effect (of the per-seed \
         ratios or relative differences, signed, for those checks); the verdict is pass when it lies \
         inside the bound, fail when it lies wholly outside, undecided otherwise. Means says whether the \
         effect on the seed means lies inside the bound.\n",
        seeds.len()
    );
    let mut rows = Vec::new();
    let mut all_results = Vec::new();
    for (i, (fact, groups)) in facts().iter().zip(&fact_groups).enumerate() {
        let histories: Vec<Vec<Vec<DailyMetrics>>> = groups
            .iter()
            .map(|&g| done.group(g).iter().map(|r| r.history.clone()).collect())
            .collect();
        for r in fact.evaluate(&histories) {
            rows.push(vec![
                (i + 1).to_string(),
                r.label.to_string(),
                format!("{:.3} ± {:.3}", r.effect, r.spread),
                crate::facts_cmd::interval_text(&r, 3),
                r.bound.text(),
                crate::facts_cmd::means_text(r.passed).to_string(),
                crate::facts_cmd::verdict(r.verdict, r.known_limit).to_lowercase(),
            ]);
            all_results.push(r);
        }
    }
    md.push_str(&md_table(
        &[
            "Fact", "Check", "Effect", "Interval", "Bound", "Means", "Verdict",
        ],
        &rows,
    ));
    let _ = writeln!(md, "\n{}", crate::facts_cmd::tally(&all_results));
    md.push('\n');

    let _ = writeln!(
        md,
        "## Scenarios\n\nEach scenario's known solution against its baseline, {} paired seeds, `N = {n}`, \
         the scenario's own days{}. The margin is how far each seed's solution lies on the passing side \
         of its own baseline's threshold, mean and 95% interval; a goal passes when the interval lies \
         wholly on the passing side, fails when it lies wholly on the failing side, and is undecided \
         otherwise. A scenario passes only when every goal passes.\n",
        seeds.len(),
        days.map_or(String::new(), |d| format!(" (here {d})"))
    );
    let mut rows = Vec::new();
    for (sc, &(gp, gb)) in scenarios().iter().zip(&scenario_groups) {
        let player: Vec<_> = done.group(gp).iter().map(|r| r.summary.clone()).collect();
        let baseline: Vec<_> = done.group(gb).iter().map(|r| r.summary.clone()).collect();
        let verdict = sc.judge(&player, &baseline);
        let solution: Vec<String> = sc
            .known_solution_changes()
            .iter()
            .map(|(path, value)| format!("`{}` {value}", path.rsplit('.').next().unwrap_or(path)))
            .collect();
        for (k, g) in verdict.goals.iter().enumerate() {
            rows.push(vec![
                if k == 0 {
                    sc.title.to_string()
                } else {
                    String::new()
                },
                if k == 0 {
                    solution.join(", ")
                } else {
                    String::new()
                },
                g.description.to_string(),
                fmt_value(g.metric, g.player),
                fmt_value(g.metric, g.baseline),
                format!(
                    "{:+.4} ({:+.4} to {:+.4})",
                    g.margin.mean, g.margin.low, g.margin.high
                ),
                g.verdict.label().to_lowercase(),
                if k == 0 {
                    verdict_text(verdict.verdict)
                } else {
                    String::new()
                },
            ]);
        }
    }
    md.push_str(&md_table(
        &[
            "Scenario",
            "Known solution",
            "Goal",
            "Solution",
            "Baseline",
            "Margin (95% interval)",
            "Goal verdict",
            "Scenario",
        ],
        &rows,
    ));

    let dir = out.unwrap_or_else(|| report::default_out("report"));
    report::ensure_dir(&dir)?;
    let file = dir.join("report.md");
    report::write_file(&file, &md)?;
    out!("{md}");
    outln!("\nwrote {}", file.display());
    Ok(())
}

fn verdict_text(v: Verdict) -> String {
    match v {
        Verdict::Pass => "pass".into(),
        Verdict::Fail => "FAIL".into(),
        Verdict::Undecided => "undecided".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_comparison_builds_valid_configurations() {
        for c in COMPARISONS {
            for side in [&c.a, &c.b] {
                assert!(
                    side_config(side, c.both, 30, Some(100)).is_ok(),
                    "{}",
                    c.label
                );
            }
            assert!(!c.metrics.is_empty(), "{}", c.label);
        }
        assert_eq!(
            side_text(&THRESHOLD_03, &["world.evasion_rate=0"]),
            "Engagement ranking with `evasion_rate` 0, `classifier_threshold` 0.3"
        );
    }

    #[test]
    fn markdown_values_follow_the_calibration_record() {
        let s = |mean, sd| Stats {
            mean,
            sd,
            min: mean,
            max: mean,
        };
        assert_eq!(
            md_stat(MetricId::HarmIndex, s(0.4992, 0.0381), false),
            "0.499±0.038"
        );
        assert_eq!(
            md_stat(MetricId::HumanDailyActives, s(850.4, 17.2), true),
            "850±17~"
        );
        assert_eq!(
            md_stat(MetricId::HarassmentPer1000, s(7.961, 1.46), false),
            "7.96±1.46"
        );
        assert_eq!(md_stat(MetricId::CreatorsLiving, s(0.0, 0.0), false), "0");
        assert_eq!(
            md_stat(MetricId::HarmfulPayoutShare, s(f32::NAN, 0.0), false),
            "-"
        );
        let t = md_table(&["a", "b"], &[vec!["1".into(), "2".into()]]);
        assert_eq!(t, "| a | b |\n| --- | --- |\n| 1 | 2 |\n");
    }
}
