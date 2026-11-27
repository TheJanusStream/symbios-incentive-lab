//! The out-of-sample validation battery (lab #31): ten findings from the
//! literature, V1 to V10, that were never used in calibration, as [`Fact`]s
//! with bands fixed before the model was run on them. They are kept apart from
//! the stylized facts (docs/model.md, section 15), which calibration tuned against:
//! the facts measure fit, this battery measures accuracy out of sample.
//!
//! - Every check runs under the integrity preset, the model's stand-in for a
//!   realistic ranked feed (the convention of facts 3, 6 and 7; the field
//!   studies ran on platforms with integrity measures), and is judged there
//!   ([`validation_battery`]). The same checks under the engagement ranking
//!   ([`Validation::engagement`]) are reported beside them as a sensitivity,
//!   not judged.
//! - The experiments (V1, V2, V3, V10) copy the field designs: a random share
//!   of the regular users is treated, [`VALIDATION_SHARE`], the field's
//!   individual randomisation, and each real run is paired with its placebo
//!   on the same seed, so the effect is measured on the same people, as fact
//!   6 measures it. Per-day measures and shares compare as relative changes,
//!   changes in standard deviations as differences.
//! - V5 copies Twitter's holdback (lab #33): a random share of the regular
//!   users reads the chronological feed of their in-network posts for the
//!   last 90 days, real against placebo on the follow graph the ranked feed
//!   built, measured on the treated.
//! - The other checks pair the design with its comparison design (V4 share
//!   friction 0.3 against 0, V9 `ads_share` 0.5 against 1), or read one run
//!   (V6, V7, V8), over the summary window of a full-size run, as the
//!   headline claims do. V6 and V8 pool their counts over the window
//!   (`Measure::Pooled`) instead of averaging daily ratios.
//! - The doses of V1 and V3 are manipulation checks, not outcomes: each was
//!   calibrated at the default world under the integrity preset, at full
//!   size on 8 seeds, before any outcome check was read, and is fixed here
//!   with the dose it achieved ([`Dose`]).
//! - Every check carries its raw change beside the effect ([`RawChange`]):
//!   the unscaled difference behind a change in standard deviations (the
//!   model's spread of informedness across participants is small, so changes
//!   in its SDs come out large), the difference of the means behind a
//!   relative change or a ratio, and the level behind a descriptive ratio.
//!   An effect in SDs also reports the SD it divides by
//!   ([`CheckResult::scale`](crate::CheckResult::scale)).
//! - Some checks carry other measures of the same quantity beside it
//!   ([`Validation::others`]): reported, never judged.
//! - Every ratio, relative change and elasticity is a ratio of seed means
//!   (the mean of one arm over the mean of the other, on paired seeds); its
//!   95% interval and verdict are of the per-seed values.

use serde::Serialize;

use crate::config::{Config, Deactivation, FeedExperiment, FeedExperimentKind};
use crate::facts::{Check, Effect, Fact, FactRun, Judge, Measure, Variant, check};
use crate::metrics::{DailyMetrics, MetricId};
use crate::presets::integrity_ranking;
use crate::stats::MeanInterval;

/// Share of the regular users an experiment treats: 0.2, the field's
/// individual randomisation (a random share of consenting users), as the
/// feed experiment's default; also for V2's break, where fact 6 sends
/// everyone (share 1).
pub const VALIDATION_SHARE: f32 = 0.2;
/// V1's days, the field's three months (Nyhan et al. 2023).
pub const V1_DAYS: u32 = 90;
/// V2's break, 35 days (Allcott et al. 2020: four weeks; 2024: six).
pub const V2_DAYS: u32 = 35;
/// Days between the end of V2's second break and the end of the run, over
/// which its later use is measured (the field's weeks after the break).
pub const V2_LATER_DAYS: u32 = 28;
/// V3's days, the field's six weeks (Beknazar-Yuzbashev et al.).
pub const V3_DAYS: u32 = 42;
/// V10's days, the field's three months (Guess et al. 2023b).
pub const V10_DAYS: u32 = 90;
/// V5's days on the chronological feed, the last 90 of the run (lab #33;
/// Huszar et al. measured their holdback over a span of months).
pub const V5_DAYS: u32 = 90;
/// V4's share friction, against 0 (Pennycook et al. 2021: the prompt's dose
/// does not map, so only the direction is tested).
pub const V4_FRICTION: f32 = 0.3;
/// V9's ad funding share, against 1: half the ad load.
pub const V9_ADS_SHARE: f32 = 0.5;

/// V1's penalty on posts by the viewer's own group (`fewer_same_group`),
/// calibrated so that the treated's same-group impression share falls by a
/// third against the placebo.
pub const V1_PENALTY: f32 = 0.28;
/// V1's target dose: the relative change of the treated's same-group share
/// of impressions, real against placebo (field: 53.7% to 36.2%).
pub const V1_DOSE_TARGET: f32 = -0.33;
/// V1's dose achieved at [`V1_PENALTY`] in the calibration (default world,
/// integrity preset, full size, seeds 1 to 8): -0.331, 95% interval -0.423
/// to -0.239. Searched coarse to fine: 0.5, 1, 2 and 4 gave -0.580, -0.858,
/// -0.971 and -0.980; 0.1, 0.2 and 0.3 gave -0.111, -0.240 and -0.360;
/// 0.25, 0.27 and 0.28 gave -0.299, -0.324 and -0.331.
pub const V1_DOSE_ACHIEVED: f32 = -0.331;
/// V3's classifier score threshold (`hide_flagged`), calibrated so that about
/// 7% of the treated's impressions would be hidden.
pub const V3_THRESHOLD: f32 = 0.56;
/// V3's target dose: the share of the treated's impressions in the placebo
/// run scored above the threshold (field: 7% of posts, comments and replies
/// hidden).
pub const V3_DOSE_TARGET: f32 = 0.07;
/// V3's dose achieved at [`V3_THRESHOLD`] in the calibration (as V1's):
/// 0.0729, 95% interval 0.0656 to 0.0802. Searched coarse to fine: 0.5 gave
/// 0.193, and 0.6, 0.7 and 0.8 gave 0 (the integrity preset demotes posts
/// scored above its classifier threshold of 0.6, and no demoted post reached
/// the treated's feeds, so the threshold hides posts scored between it and
/// 0.6); 0.52, 0.54, 0.56 and 0.58 gave 0.153, 0.115, 0.073 and 0.034.
pub const V3_DOSE_ACHIEVED: f32 = 0.0729;

/// One out-of-sample check: its judged fact, its engagement-ranking twin,
/// its source and design, its dose and its raw changes.
#[derive(Clone, Copy, Debug)]
pub struct Validation {
    /// Stable id, V1 to V10.
    pub id: &'static str,
    /// The checks under the integrity preset: judged.
    pub fact: &'static Fact,
    /// The same checks under the engagement ranking: a sensitivity, reported
    /// beside the judged value and never judged.
    pub engagement: &'static Fact,
    /// The finding as the source states it, quoted from the research notes
    /// (exports/research-validation/notes.md).
    pub source: &'static str,
    /// How the field design is copied: arms, days, share, measure window.
    pub design: &'static str,
    /// The experiment's dose, a manipulation check; `None` where the dose is
    /// fixed by the design.
    pub dose: Option<Dose>,
    raws: &'static [Raw],
    others: &'static [Raw],
}

/// A dose: the lever value fixed in the battery and the manipulation check
/// that calibrated it.
#[derive(Clone, Copy, Debug)]
pub struct Dose {
    /// What the dose measures.
    pub label: &'static str,
    /// The lever and its value, as text.
    pub lever: &'static str,
    /// The lever's value.
    pub value: f32,
    /// The field's dose.
    pub target: f32,
    /// The dose the calibration achieved at `value`.
    pub achieved: f32,
    measure: Measure,
    effect: Effect,
}

/// A dose measured in a set of runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct DoseResult {
    /// The dose on the seed means.
    pub effect: f32,
    /// Standard deviation of the per-seed dose.
    pub spread: f32,
    /// The per-seed dose's mean and 95% interval.
    pub interval: MeanInterval,
}

/// How a check's raw change is measured.
#[derive(Clone, Copy, Debug)]
struct Raw {
    label: &'static str,
    measure: Measure,
    effect: Effect,
}

/// A check's raw change: the unscaled quantity behind its effect.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RawChange {
    /// What the raw change is, with its unit.
    pub label: &'static str,
    /// On the seed means.
    pub effect: f32,
    /// Standard deviation of the per-seed value.
    pub spread: f32,
    /// The per-seed value's mean and 95% interval.
    pub interval: MeanInterval,
}

/// Evaluates an effect of a measure on histories indexed `[variant][seed]`.
fn measured(
    measure: Measure,
    effect: Effect,
    histories: &[Vec<Vec<DailyMetrics>>],
) -> (f32, f32, MeanInterval) {
    let values = |v: usize| -> Vec<f32> {
        histories
            .get(v)
            .map(|runs| runs.iter().map(|h| measure.of(h)).collect())
            .unwrap_or_default()
    };
    let (effect, spread, per_seed) = effect.of(&values);
    (effect, spread, MeanInterval::of(per_seed.into_iter()))
}

impl Dose {
    /// The dose in histories indexed `[variant][seed]`, as the battery's
    /// variants run them.
    pub fn evaluate(&self, histories: &[Vec<Vec<DailyMetrics>>]) -> DoseResult {
        let (effect, spread, interval) = measured(self.measure, self.effect, histories);
        DoseResult {
            effect,
            spread,
            interval,
        }
    }
}

impl Validation {
    /// Each check's raw change, in the order of the fact's checks, from
    /// histories indexed `[variant][seed]` (of either design).
    pub fn raw_changes(&self, histories: &[Vec<Vec<DailyMetrics>>]) -> Vec<RawChange> {
        self.raws
            .iter()
            .map(|r| {
                let (effect, spread, interval) = measured(r.measure, r.effect, histories);
                RawChange {
                    label: r.label,
                    effect,
                    spread,
                    interval,
                }
            })
            .collect()
    }

    /// Other measures of the checks' quantities, reported beside them and
    /// never judged (V5's reach without posts later removed or demoted, V6's
    /// and V8's means of daily ratios, the measure before lab #39), from
    /// histories indexed `[variant][seed]` (of either design); empty for
    /// most checks.
    pub fn others(&self, histories: &[Vec<Vec<DailyMetrics>>]) -> Vec<RawChange> {
        self.others
            .iter()
            .map(|r| {
                let (effect, spread, interval) = measured(r.measure, r.effect, histories);
                RawChange {
                    label: r.label,
                    effect,
                    spread,
                    interval,
                }
            })
            .collect()
    }

    /// The configurations of the judged fact's variants for one seed with
    /// the dose set to `value` instead of [`Dose::value`]: for calibrating
    /// the dose. Without a dose, the variants as they are.
    pub fn configs_with_dose(
        &self,
        base: &Config,
        run: &FactRun,
        seed: u64,
        value: f32,
    ) -> Vec<Config> {
        let mut configs = self.fact.configs_from(base, run, seed);
        for c in &mut configs {
            if let Some(f) = c.run.feed_experiment.as_mut() {
                match &mut f.kind {
                    FeedExperimentKind::FewerSameGroup { penalty } => *penalty = value,
                    FeedExperimentKind::HideFlagged { threshold } => *threshold = value,
                    FeedExperimentKind::NoReshares | FeedExperimentKind::Chronological => {}
                }
            }
        }
        configs
    }
}

/// The validation battery's judged facts, V1 to V10, under the integrity
/// preset: what `incentive robustness` runs across worlds.
pub fn validation_battery() -> &'static [Fact] {
    &INTEGRITY
}

/// Every out-of-sample check, V1 to V10, with its engagement-ranking twin,
/// source, design, dose and raw changes.
pub fn validations() -> &'static [Validation] {
    &VALIDATIONS
}

/// The design under test: the integrity preset, or the engagement ranking
/// (the base design as it is) for the twin.
fn design<const ER: bool>(c: &mut Config) {
    if !ER {
        integrity_ranking(&mut c.design);
    }
}

/// A feed experiment of `kind` over the last `days` days under the design.
fn feed<const ER: bool>(c: &mut Config, kind: FeedExperimentKind, days: u32, placebo: bool) {
    design::<ER>(c);
    c.run.feed_experiment = Some(FeedExperiment {
        kind,
        days,
        share: VALIDATION_SHARE,
        placebo,
    });
}

fn v1<const ER: bool, const PLACEBO: bool>(c: &mut Config) {
    let kind = FeedExperimentKind::FewerSameGroup {
        penalty: V1_PENALTY,
    };
    feed::<ER>(c, kind, V1_DAYS, PLACEBO);
}

fn v2<const ER: bool, const PLACEBO: bool, const LATER: bool>(c: &mut Config) {
    design::<ER>(c);
    c.run.deactivation = Some(Deactivation {
        days: V2_DAYS,
        share: VALIDATION_SHARE,
        placebo: PLACEBO,
        ends_before_end: if LATER { V2_LATER_DAYS } else { 0 },
    });
}

fn v3<const ER: bool, const PLACEBO: bool>(c: &mut Config) {
    let kind = FeedExperimentKind::HideFlagged {
        threshold: V3_THRESHOLD,
    };
    feed::<ER>(c, kind, V3_DAYS, PLACEBO);
}

fn v4<const ER: bool, const FRICTION: bool>(c: &mut Config) {
    design::<ER>(c);
    c.design.share_friction = if FRICTION { V4_FRICTION } else { 0.0 };
}

fn v5<const ER: bool, const PLACEBO: bool>(c: &mut Config) {
    feed::<ER>(c, FeedExperimentKind::Chronological, V5_DAYS, PLACEBO);
}

fn v7<const ER: bool>(c: &mut Config) {
    design::<ER>(c);
    c.design.community_notes = true;
}

fn v9<const ER: bool, const HALF: bool>(c: &mut Config) {
    design::<ER>(c);
    c.design.ads_share = if HALF { V9_ADS_SHARE } else { 1.0 };
}

fn v10<const ER: bool, const PLACEBO: bool>(c: &mut Config) {
    feed::<ER>(c, FeedExperimentKind::NoReshares, V10_DAYS, PLACEBO);
}

/// A band check on the paired real (`0`) and placebo (`1`) runs.
const fn paired(label: &'static str, id: MetricId, effect: Effect, low: f32, high: f32) -> Check {
    check(label, Measure::Last(id), Judge::Band { effect, low, high })
}

/// A band check on the paired runs, its effect in standard deviations of
/// the participants' values at the start, held by `scale`.
const fn paired_sd(
    label: &'static str,
    id: MetricId,
    scale: MetricId,
    low: f32,
    high: f32,
) -> Check {
    paired(label, id, DIFF, low, high).with_scale(scale)
}

const REL: Effect = Effect::Relative { a: 0, b: 1 };
const DIFF: Effect = Effect::Difference { a: 0, b: 1 };
const MEAN: Effect = Effect::Mean { a: 0 };

/// A band check on the summary window.
const fn summary(label: &'static str, id: MetricId, effect: Effect, low: f32, high: f32) -> Check {
    check(
        label,
        Measure::Summary(id),
        Judge::Band { effect, low, high },
    )
}

const V1_CHECKS: &[Check] = &[
    paired_sd(
        "polarisation change of the treated, real minus placebo, in SDs",
        MetricId::FeedExperimentPolarisation,
        MetricId::FeedExperimentPolarisationSd,
        -0.12,
        0.12,
    ),
    paired(
        "toxic share of the treated's impressions, relative change",
        MetricId::FeedExperimentToxicShare,
        REL,
        -0.35,
        -0.02,
    ),
    paired(
        "the treated's time per day, relative change",
        MetricId::FeedExperimentDwell,
        REL,
        -0.10,
        0.05,
    ),
    paired(
        "misinformation share of the treated's impressions, relative change",
        MetricId::FeedExperimentMisinformationShare,
        REL,
        -0.5,
        0.0,
    ),
];

const V2_CHECKS: &[Check] = &[
    paired_sd(
        "break group's affective polarisation change, break minus placebo, in SDs",
        MetricId::BreakPolarisation,
        MetricId::BreakPolarisationSd,
        -0.14,
        0.02,
    ),
    paired_sd(
        "break group's informedness change, break minus placebo, in SDs",
        MetricId::BreakInformedness,
        MetricId::BreakInformednessSd,
        -0.30,
        -0.05,
    ),
    paired(
        "break group's time per day in the 28 days after a break, relative change",
        MetricId::PostBreakDwell,
        Effect::Relative { a: 2, b: 3 },
        -0.35,
        -0.08,
    ),
];

const V3_CHECKS: &[Check] = &[
    paired(
        "the treated's time per day, relative change",
        MetricId::FeedExperimentDwell,
        REL,
        -0.25,
        -0.02,
    ),
    paired(
        "the treated's engagement actions per day, relative change",
        MetricId::FeedExperimentEngagement,
        REL,
        -0.30,
        0.0,
    ),
    // The model has no channel from exposure to the toxicity a person
    // writes, so a fail is expected and informative (lab #31): judged and
    // reported, not required.
    Check {
        label: "toxicity of the treated's own posts, relative change",
        measure: Measure::Last(MetricId::FeedExperimentPostToxicity),
        judge: Judge::Band {
            effect: REL,
            low: -0.45,
            high: -0.10,
        },
        known_limit: true,
        neutral: None,
        scale: None,
    },
];

const V4_CHECKS: &[Check] = &[summary(
    "mean truth of human shares, share friction 0.3 against 0, relative change",
    MetricId::SharedTruth,
    REL,
    0.0,
    0.25,
)];

/// The ranked arm over the chronological arm: V5's placebo (`1`, kept on
/// the ranked feed) over its real run (`0`, on the chronological feed).
const RANKED_OVER_CHRONOLOGICAL: Effect = Effect::Ratio { a: 1, b: 0 };

const V5_CHECKS: &[Check] = &[
    paired(
        "the treated's daily impressions of the more partisan half of authors, ranked (placebo) / \
            chronological (real)",
        MetricId::FeedExperimentPartisanHalfImpressions,
        RANKED_OVER_CHRONOLOGICAL,
        1.0,
        5.0,
    ),
    check(
        "the treated's impressions of the most partisan tenth over the middle fifth, ranked (placebo) / \
            chronological (real)",
        Measure::Last(MetricId::FeedExperimentExtremeMiddleRatio),
        Judge::RatioBelow {
            a: 1,
            b: 0,
            ratio: 1.1,
        },
    ),
];

/// A ratio of two pooled ratios over the summary window.
const fn pooled(num: MetricId, den: MetricId, over_num: MetricId, over_den: MetricId) -> Measure {
    Measure::Pooled {
        num,
        den,
        over: Some((over_num, over_den)),
    }
}

/// A ratio pooled over the summary window.
const fn pooled_share(num: MetricId, den: MetricId) -> Measure {
    Measure::Pooled {
        num,
        den,
        over: None,
    }
}

/// V6's first measure: the out-group share of in-network impressions over
/// that of in-network candidates, pooled.
const V6_RANKING: Measure = pooled(
    MetricId::InNetworkOutgroupImpressionCount,
    MetricId::InNetworkImpressionCount,
    MetricId::InNetworkOutgroupCandidateCount,
    MetricId::InNetworkCandidateCount,
);
/// V6's second: the out-group share of likes on in-network impressions over
/// that of in-network impressions, pooled.
const V6_LIKES: Measure = pooled(
    MetricId::InNetworkOutgroupLikeCount,
    MetricId::InNetworkLikeCount,
    MetricId::InNetworkOutgroupImpressionCount,
    MetricId::InNetworkImpressionCount,
);
/// V8's measure: human shares per impression of Outrage posts over those of
/// the non-hostile styles, pooled.
const V8_RATE: Measure = pooled(
    MetricId::OutrageStyleShareCount,
    MetricId::OutrageStyleImpressionCount,
    MetricId::CalmStyleShareCount,
    MetricId::CalmStyleImpressionCount,
);

/// A band check on a mean of variant 0's measure, whose neutral value is 1.
const fn mean_of_ratio(label: &'static str, measure: Measure, low: f32, high: f32) -> Check {
    check(
        label,
        measure,
        Judge::Band {
            effect: MEAN,
            low,
            high,
        },
    )
    .with_neutral(1.0)
}

// V6, V7 and V8 read a ratio or a share of one run, so their neutral value
// is 1, not the mean's natural 0: equal shares, every share before the note,
// equal share rates. V6 and V8 pool their counts over the summary window
// (lab #39); V7 still averages its daily shares.
const V6_CHECKS: &[Check] = &[
    mean_of_ratio(
        "out-group share of in-network impressions over that of in-network candidates, pooled",
        V6_RANKING,
        0.85,
        1.00,
    ),
    mean_of_ratio(
        "out-group share of likes over that of in-network impressions, pooled",
        V6_LIKES,
        0.75,
        0.97,
    ),
];

const V7_CHECKS: &[Check] = &[summary(
    "share of a noted post's human shares made before its note",
    MetricId::NotedSharesBeforeNote,
    MEAN,
    0.70,
    0.95,
)
.with_neutral(1.0)];

const V8_CHECKS: &[Check] = &[mean_of_ratio(
    "shares per impression of Outrage posts over the non-hostile styles, pooled",
    V8_RATE,
    1.3,
    3.0,
)];

const V9_CHECKS: &[Check] = &[summary(
    "elasticity of time spent to ad load, ads_share 0.5 against 1",
    MetricId::TimeSpent,
    Effect::Elasticity {
        a: 0,
        b: 1,
        change: V9_ADS_SHARE - 1.0,
    },
    -0.15,
    -0.01,
)];

const V10_CHECKS: &[Check] = &[
    paired(
        "misinformation share of the treated's impressions, relative change",
        MetricId::FeedExperimentMisinformationShare,
        REL,
        -0.8,
        -0.05,
    ),
    paired(
        "the treated's engagement actions per day, relative change",
        MetricId::FeedExperimentEngagement,
        REL,
        -0.5,
        -0.01,
    ),
    paired_sd(
        "polarisation change of the treated, real minus placebo, in SDs",
        MetricId::FeedExperimentPolarisation,
        MetricId::FeedExperimentPolarisationSd,
        -0.1,
        0.1,
    ),
    paired_sd(
        "informedness change of the treated, real minus placebo, in SDs",
        MetricId::FeedExperimentInformedness,
        MetricId::FeedExperimentInformednessSd,
        -0.5,
        0.0,
    ),
];

const REAL: &str = "real";
const PLACEBO: &str = "placebo";

/// The battery under one design: the integrity preset (`ER` false) or the
/// engagement ranking (`ER` true).
struct Battery<const ER: bool>;

impl<const ER: bool> Battery<ER> {
    const FACTS: [Fact; 10] = [
        Fact {
            id: "V1",
            title: "Fewer like-minded posts for three months: exposure shifts, polarisation does not",
            reference: "Nyhan et al. (2023), Nature 620, 137-144",
            variants: &[
                Variant {
                    label: REAL,
                    apply: v1::<ER, false>,
                },
                Variant {
                    label: PLACEBO,
                    apply: v1::<ER, true>,
                },
            ],
            checks: V1_CHECKS,
        },
        Fact {
            id: "V2",
            title: "A five-week break: affective polarisation barely moves, knowledge and later use fall",
            reference: "Allcott et al. (2020), AER 110(3); Allcott et al. (2024), PNAS 121(21)",
            variants: &[
                Variant {
                    label: "break",
                    apply: v2::<ER, false, false>,
                },
                Variant {
                    label: PLACEBO,
                    apply: v2::<ER, true, false>,
                },
                Variant {
                    label: "break ending 28 days before the end",
                    apply: v2::<ER, false, true>,
                },
                Variant {
                    label: "placebo ending 28 days before the end",
                    apply: v2::<ER, true, true>,
                },
            ],
            checks: V2_CHECKS,
        },
        Fact {
            id: "V3",
            title: "Hiding flagged posts for six weeks cuts time and engagement",
            reference: "Beknazar-Yuzbashev, Jimenez-Duran, McCrosky and Stalinski (working paper, CAGE WP \
                741/2025)",
            variants: &[
                Variant {
                    label: REAL,
                    apply: v3::<ER, false>,
                },
                Variant {
                    label: PLACEBO,
                    apply: v3::<ER, true>,
                },
            ],
            checks: V3_CHECKS,
        },
        Fact {
            id: "V4",
            title: "Friction on sharing raises the truth of what is shared",
            reference: "Pennycook et al. (2021), Nature 592, 590-595",
            variants: &[
                Variant {
                    label: "share friction 0.3",
                    apply: v4::<ER, true>,
                },
                Variant {
                    label: "share friction 0",
                    apply: v4::<ER, false>,
                },
            ],
            checks: V4_CHECKS,
        },
        Fact {
            id: "V5",
            title: "Ranking amplifies partisan authors, but not the most extreme more than the middle",
            reference: "Huszar et al. (2022), PNAS 119(1)",
            variants: &[
                Variant {
                    label: "real: the treated on the chronological feed",
                    apply: v5::<ER, false>,
                },
                Variant {
                    label: "placebo: the treated kept on the ranked feed",
                    apply: v5::<ER, true>,
                },
            ],
            checks: V5_CHECKS,
        },
        Fact {
            id: "V6",
            title: "Ranking trims cross-cutting exposure a little, people's choices trim it more",
            reference: "Bakshy, Messing and Adamic (2015), Science 348, 1130-1132",
            variants: &[Variant {
                label: "ranked feed",
                apply: design::<ER>,
            }],
            checks: V6_CHECKS,
        },
        Fact {
            id: "V7",
            title: "Most of a noted post's shares come before its note",
            reference: "Slaughter, Peytavin, Ugander and Saveski (2025), PNAS 122(38)",
            variants: &[Variant {
                label: "community notes on",
                apply: v7::<ER>,
            }],
            checks: V7_CHECKS,
        },
        Fact {
            id: "V8",
            title: "Out-group hostility is shared more per view",
            reference: "Rathje, Van Bavel and van der Linden (2021), PNAS 118(26); Brady et al. (2017), PNAS \
                114(28)",
            variants: &[Variant {
                label: "ranked feed",
                apply: design::<ER>,
            }],
            checks: V8_CHECKS,
        },
        Fact {
            id: "V9",
            title: "Time spent responds little to ad load",
            reference: "Goli, Huang, Reiley and Riabov (2024), arXiv 2412.05516; Beknazar-Yuzbashev, \
                Jimenez-Duran, Simonov and Stalinski (CAGE WP 792/2026)",
            variants: &[
                Variant {
                    label: "ads_share 0.5",
                    apply: v9::<ER, true>,
                },
                Variant {
                    label: "ads_share 1",
                    apply: v9::<ER, false>,
                },
            ],
            checks: V9_CHECKS,
        },
        Fact {
            id: "V10",
            title: "A feed without reshares for three months: less misinformation and engagement, \
                polarisation unchanged",
            reference: "Guess et al. (2023b), Science 381(6656), 404-408 (abstract only)",
            variants: &[
                Variant {
                    label: REAL,
                    apply: v10::<ER, false>,
                },
                Variant {
                    label: PLACEBO,
                    apply: v10::<ER, true>,
                },
            ],
            checks: V10_CHECKS,
        },
    ];
}

static INTEGRITY: [Fact; 10] = Battery::<false>::FACTS;
static ENGAGEMENT: [Fact; 10] = Battery::<true>::FACTS;

const fn raw(label: &'static str, id: MetricId, effect: Effect, last: bool) -> Raw {
    Raw {
        label,
        measure: if last {
            Measure::Last(id)
        } else {
            Measure::Summary(id)
        },
        effect,
    }
}

const RAW_TIME: &str = "time per day, real minus placebo";
const RAW_ACTIONS: &str = "actions per day, real minus placebo";
const RAW_MISINFORMATION: &str = "misinformation share of impressions, real minus placebo";
const RAW_POLARISATION: &str = "polarisation change, unscaled, real minus placebo";

static VALIDATIONS: [Validation; 10] = [
    Validation {
        id: "V1",
        fact: &INTEGRITY[0],
        engagement: &ENGAGEMENT[0],
        source: "Nyhan et al. (2023), Facebook, 23,377 adults, three months, content from like-minded \
            sources down-ranked: like-minded share of exposures 53.7% -> 36.2%; uncivil content 3.15% -> 2.81% \
            (-11% relative); misinformation repeat offenders 0.76% -> 0.55%; time spent \"no significant \
            effect ... (-0.02 s.d., 95% confidence interval: -0.050, 0.004)\"; attitudes: \"We can \
            confidently rule out effects of ±0.12 s.d. or more on each of these outcomes\". The attitudes \
            are reported in s.d. only, with no raw change.",
        design: "Feed experiment fewer_same_group over the last 90 days, share 0.2 of the regular users, real \
            against placebo on the same seed, measured on the treated.",
        dose: Some(Dose {
            label: "the treated's same-group share of impressions, relative change, real against placebo",
            lever: "fewer_same_group penalty",
            value: V1_PENALTY,
            target: V1_DOSE_TARGET,
            achieved: V1_DOSE_ACHIEVED,
            measure: Measure::Last(MetricId::FeedExperimentSameGroupShare),
            effect: REL,
        }),
        raws: &[
            raw(
                RAW_POLARISATION,
                MetricId::FeedExperimentPolarisationRaw,
                DIFF,
                true,
            ),
            raw(
                "toxic share of impressions, real minus placebo",
                MetricId::FeedExperimentToxicShare,
                DIFF,
                true,
            ),
            raw(RAW_TIME, MetricId::FeedExperimentDwell, DIFF, true),
            raw(
                RAW_MISINFORMATION,
                MetricId::FeedExperimentMisinformationShare,
                DIFF,
                true,
            ),
        ],
        others: &[],
    },
    Validation {
        id: "V2",
        fact: &INTEGRITY[1],
        engagement: &ENGAGEMENT[1],
        source: "Allcott et al. (2020), four-week Facebook deactivation: party affective polarization -0.06 \
            SD (SE 0.04, not significant); news knowledge index -0.19 SD (0.04); Facebook app use weeks later \
            \"a 23 percent reduction relative to the Control group mean\". Allcott et al. (2024), six weeks: \
            affective polarization Facebook -0.031 SD (95% CI -0.062, -0.000); Facebook news knowledge -0.098 \
            SD; reductions \"of 23% and 15% in average time spent over the postdeactivation period for \
            Facebook and Instagram\". Polarization and knowledge are reported in SDs of the control group \
            only, with no raw change.",
        design: "Deactivation of 35 days at share 0.2 of the regular users, break against placebo on the same \
            seed, measured on the break group at the break's end (the run's end); later use from a second \
            pair whose break ends 28 days before the run's end, over those 28 days.",
        dose: None,
        raws: &[
            raw(
                "polarisation change, unscaled, break minus placebo",
                MetricId::BreakPolarisationRaw,
                DIFF,
                true,
            ),
            raw(
                "informedness change, unscaled, break minus placebo",
                MetricId::BreakInformednessRaw,
                DIFF,
                true,
            ),
            raw(
                "time per day after the break, break minus placebo",
                MetricId::PostBreakDwell,
                Effect::Difference { a: 2, b: 3 },
                true,
            ),
        ],
        others: &[],
    },
    Validation {
        id: "V3",
        fact: &INTEGRITY[2],
        engagement: &ENGAGEMENT[2],
        source: "Beknazar-Yuzbashev et al. (working paper, May 2026 draft), browser extension, six weeks, \
            text above a toxicity score of 0.3 hidden: \"7% of posts, comments and replies hidden\"; Facebook \
            time \"an average decrease of 1.3 minutes per day on Facebook, or 10% relative to the mean\"; \
            engagement index -0.056 SD; users' own posts and comments \"by 30% and 27% relative to the mean\" \
            lower.",
        design: "Feed experiment hide_flagged over the last 42 days, share 0.2 of the regular users, real \
            against placebo on the same seed, measured on the treated.",
        dose: Some(Dose {
            label: "share of the treated's impressions in the placebo run scored above the threshold",
            lever: "hide_flagged threshold",
            value: V3_THRESHOLD,
            target: V3_DOSE_TARGET,
            achieved: V3_DOSE_ACHIEVED,
            measure: Measure::Last(MetricId::FeedExperimentFlaggedShare),
            effect: Effect::Mean { a: 1 },
        }),
        raws: &[
            raw(RAW_TIME, MetricId::FeedExperimentDwell, DIFF, true),
            raw(RAW_ACTIONS, MetricId::FeedExperimentEngagement, DIFF, true),
            raw(
                "mean toxicity of own posts, real minus placebo",
                MetricId::FeedExperimentPostToxicity,
                DIFF,
                true,
            ),
        ],
        others: &[],
    },
    Validation {
        id: "V4",
        fact: &INTEGRITY[3],
        engagement: &ENGAGEMENT[3],
        source: "Pennycook et al. (2021), Twitter, accuracy prompt: quality of shared news \"increases of 4.8% \
            and 9.0%\" (average and summed, user-days with tweets); \"including user-days with no tweets \
            yields an increase of 2.1% and 4.0%\". The prompt's dose does not map onto share friction: \
            direction only.",
        design: "share_friction 0.3 against 0, a full run each, the mean truth of human shares over the \
            summary window.",
        dose: None,
        raws: &[raw(
            "mean truth of human shares, friction minus none",
            MetricId::SharedTruth,
            DIFF,
            false,
        )],
        others: &[],
    },
    Validation {
        id: "V5",
        fact: &INTEGRITY[4],
        engagement: &ENGAGEMENT[4],
        source: "Huszar et al. (2022), Twitter's holdback: \"all parties enjoy an amplification effect\"; \
            more partisan US news sources amplified more than the centre; far-left and far-right parties: no \
            evidence of extra amplification over moderates. Amplification is the reach in the ranked group \
            over that in the holdback group on the chronological feed.",
        design: "Feed experiment chronological over the last 90 days (lab #33): share 0.2 of the regular \
            users read the chronological feed of their in-network posts, on the follow graph the ranked \
            feed built; real against placebo on the same seed, measured on the treated; impressions per \
            treated viewer and day, pooled over the group and the days; authors ranked by the partisanship \
            trait. Amplification is the placebo (ranked) over the real run (chronological).",
        dose: None,
        raws: &[
            raw(
                "daily impressions of the more partisan half per treated viewer, ranked minus chronological",
                MetricId::FeedExperimentPartisanHalfImpressions,
                Effect::Difference { a: 1, b: 0 },
                true,
            ),
            raw(
                "impressions of the most partisan tenth over the middle fifth, ranked minus chronological",
                MetricId::FeedExperimentExtremeMiddleRatio,
                Effect::Difference { a: 1, b: 0 },
                true,
            ),
        ],
        others: &[
            raw(
                "the more partisan half's daily impressions, ranked / chronological, without posts \
                    removed or demoted",
                MetricId::FeedExperimentPartisanHalfImpressionsKept,
                RANKED_OVER_CHRONOLOGICAL,
                true,
            ),
            raw(
                "the most partisan tenth over the middle fifth, ranked / chronological, without posts \
                    removed or demoted",
                MetricId::FeedExperimentExtremeMiddleRatioKept,
                RANKED_OVER_CHRONOLOGICAL,
                true,
            ),
            raw(
                "the treated's daily impressions of all posts, ranked / chronological",
                MetricId::FeedExperimentImpressions,
                RANKED_OVER_CHRONOLOGICAL,
                true,
            ),
            raw(
                "the treated's daily impressions of all posts, ranked / chronological, without posts \
                    removed or demoted",
                MetricId::FeedExperimentImpressionsKept,
                RANKED_OVER_CHRONOLOGICAL,
                true,
            ),
        ],
    },
    Validation {
        id: "V6",
        fact: &INTEGRITY[5],
        engagement: &ENGAGEMENT[5],
        source: "Bakshy, Messing and Adamic (2015), the authors' summary: cross-cutting share \"29.5% shared, \
            28.5% seen, 24.9% clicked\"; liberals 23% -> 22% seen -> 20% clicked, conservatives 34% -> 33% -> \
            29%.",
        design: "One full run of the design, the out-group shares of the in-network stages over the summary \
            window, each pooled from the window's counts.",
        dose: None,
        raws: &[
            Raw {
                label: "out-group share of in-network impressions, pooled",
                measure: pooled_share(
                    MetricId::InNetworkOutgroupImpressionCount,
                    MetricId::InNetworkImpressionCount,
                ),
                effect: MEAN,
            },
            Raw {
                label: "out-group share of likes on in-network impressions, pooled",
                measure: pooled_share(
                    MetricId::InNetworkOutgroupLikeCount,
                    MetricId::InNetworkLikeCount,
                ),
                effect: MEAN,
            },
        ],
        others: &[
            raw(
                "out-group impressions over candidates as a mean of daily ratios (before lab #39)",
                MetricId::OutgroupRankingRatio,
                MEAN,
                false,
            ),
            raw(
                "out-group likes over impressions as a mean of daily ratios (before lab #39)",
                MetricId::OutgroupLikeRatio,
                MEAN,
                false,
            ),
        ],
    },
    Validation {
        id: "V7",
        fact: &INTEGRITY[6],
        engagement: &ENGAGEMENT[6],
        source: "Slaughter et al. (2025), X, 40,078 posts with proposed notes: \"reductions of 46.1% in \
            reposts ... after being attached. Over the posts' entire lifespans, these reductions amount to \
            11.6% fewer reposts\". Derived: about 0.85 of a noted post's reposts come before its note (the \
            -46% after the note is circular, the model assumes it).",
        design: "One full run of the design with community notes on, over the summary window.",
        dose: None,
        raws: &[raw(
            "share of noted posts' human shares before the note",
            MetricId::NotedSharesBeforeNote,
            MEAN,
            false,
        )],
        others: &[],
    },
    Validation {
        id: "V8",
        fact: &INTEGRITY[7],
        engagement: &ENGAGEMENT[7],
        source: "Rathje et al. (2021): each out-group word \"increased the odds of a social media post being \
            shared by 67%\" (news media 35-57%, congressional accounts 65-180%). Brady et al. (2017): \
            moral-emotional words \"increased their diffusion by a factor of 20% for each additional word\". \
            Direction built into the model, magnitude not tuned.",
        design: "One full run of the design, human shares per impression of Outrage posts over those of \
            Informative, Entertaining and Personal posts, over the summary window, pooled from the window's \
            counts.",
        dose: None,
        raws: &[Raw {
            label: "human shares per impression of Outrage posts, pooled",
            measure: pooled_share(
                MetricId::OutrageStyleShareCount,
                MetricId::OutrageStyleImpressionCount,
            ),
            effect: MEAN,
        }],
        others: &[raw(
            "Outrage share rate ratio as a mean of daily ratios (before lab #39)",
            MetricId::OutrageShareRatio,
            MEAN,
            false,
        )],
    },
    Validation {
        id: "V9",
        fact: &INTEGRITY[8],
        engagement: &ENGAGEMENT[8],
        source: "Goli et al. (2024), Pandora, 21 months: elasticities of listening hours to ad load \"start \
            out at around -0.02% and -0.025%, slowly increasing to effects of -0.070% and -0.076%\" per 1% \
            more ads. Beknazar-Yuzbashev et al. (2026), Facebook: short-run elasticity of active time to ad \
            load -0.07.",
        design: "ads_share 0.5 against 1 (half the ad load), full runs, total time spent over the summary \
            window (the end of a year), elasticity = relative change of time / -0.5.",
        dose: None,
        raws: &[raw(
            "total time spent per day, half the ads minus full",
            MetricId::TimeSpent,
            Effect::Difference { a: 0, b: 1 },
            false,
        )],
        others: &[],
    },
    Validation {
        id: "V10",
        fact: &INTEGRITY[9],
        engagement: &ENGAGEMENT[9],
        source: "Guess et al. (2023b), abstract: removing reshares \"substantially decreases the amount of \
            political news, including content from untrustworthy sources\"; \"decreases overall clicks and \
            reactions\"; \"clear decreases in news knowledge within the sample\"; \"does not significantly \
            affect political polarization\". Directions only; no raw change is reported.",
        design: "Feed experiment no_reshares over the last 90 days, share 0.2 of the regular users, real \
            against placebo on the same seed, measured on the treated.",
        dose: None,
        raws: &[
            raw(
                RAW_MISINFORMATION,
                MetricId::FeedExperimentMisinformationShare,
                DIFF,
                true,
            ),
            raw(RAW_ACTIONS, MetricId::FeedExperimentEngagement, DIFF, true),
            raw(
                RAW_POLARISATION,
                MetricId::FeedExperimentPolarisationRaw,
                DIFF,
                true,
            ),
            raw(
                "informedness change, unscaled, real minus placebo",
                MetricId::FeedExperimentInformednessRaw,
                DIFF,
                true,
            ),
        ],
        others: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DesignLevers;
    use crate::simulation::Simulation;

    fn run() -> FactRun {
        FactRun {
            seeds: vec![1],
            population: 300,
            days: 365,
        }
    }

    fn integrity() -> DesignLevers {
        let mut d = DesignLevers::default();
        integrity_ranking(&mut d);
        d
    }

    #[test]
    fn the_battery_is_well_formed() {
        let all = validations();
        assert_eq!(all.len(), 10);
        for (k, v) in all.iter().enumerate() {
            let id = format!("V{}", k + 1);
            assert_eq!((v.id, v.fact.id, v.engagement.id), (&*id, &*id, &*id));
            assert!(std::ptr::eq(v.fact, &validation_battery()[k]));
            assert_eq!(v.fact.variants.len(), v.engagement.variants.len());
            let labels = |f: &Fact| f.checks.iter().map(|c| c.label).collect::<Vec<_>>();
            assert_eq!(labels(v.fact), labels(v.engagement));
            assert_eq!(v.raws.len(), v.fact.checks.len(), "{id}");
            assert!(!v.source.is_empty() && !v.design.is_empty());
        }
        // Only V3's own-toxicity check is a known limit, fixed before any run.
        let limits: Vec<(&str, &str)> = all
            .iter()
            .flat_map(|v| {
                v.fact
                    .checks
                    .iter()
                    .filter(|c| c.known_limit)
                    .map(move |c| (v.id, c.label))
            })
            .collect();
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].0, "V3");
        // Exactly the effects in standard deviations carry their scale.
        for v in all {
            for c in v.fact.checks {
                assert_eq!(
                    c.scale.is_some(),
                    c.label.ends_with("in SDs"),
                    "{}",
                    c.label
                );
            }
        }
        // A ratio or a share read as a mean has the neutral 1 (V6, V7, V8);
        // every other check keeps its effect's natural neutral.
        for v in all {
            for c in v.fact.checks.iter().chain(v.engagement.checks) {
                let expected = ["V6", "V7", "V8"].contains(&v.id).then_some(1.0);
                assert_eq!(c.neutral, expected, "{} {}", v.id, c.label);
            }
        }
    }

    /// The variants copy the field designs under the integrity preset, and
    /// the twin under the base design, the engagement ranking.
    #[test]
    fn the_variants_copy_the_field_designs() {
        let base = Config::default();
        let configs = |v: &Validation, er: bool| {
            let fact = if er { v.engagement } else { v.fact };
            let configs = fact.configs_from(&base, &run(), 1);
            for c in &configs {
                assert!(c.validate().is_ok(), "{}: {:?}", v.id, c.validate());
            }
            configs
        };
        let design = |er: bool| if er { base.design.clone() } else { integrity() };
        for er in [false, true] {
            let v = |id: &str| {
                configs(
                    validations().iter().find(|v| v.id == id).expect("listed"),
                    er,
                )
            };
            let feed = |c: &Config| c.run.feed_experiment.clone().expect("feed experiment");
            for (id, kind, days) in [
                (
                    "V1",
                    FeedExperimentKind::FewerSameGroup {
                        penalty: V1_PENALTY,
                    },
                    V1_DAYS,
                ),
                (
                    "V3",
                    FeedExperimentKind::HideFlagged {
                        threshold: V3_THRESHOLD,
                    },
                    V3_DAYS,
                ),
                ("V10", FeedExperimentKind::NoReshares, V10_DAYS),
                ("V5", FeedExperimentKind::Chronological, V5_DAYS),
            ] {
                let c = v(id);
                for (arm, placebo) in [(0, false), (1, true)] {
                    let expected = FeedExperiment {
                        kind,
                        days,
                        share: VALIDATION_SHARE,
                        placebo,
                    };
                    assert_eq!(feed(&c[arm]), expected, "{id}");
                    assert_eq!(c[arm].design, design(er), "{id}");
                }
            }
            let c = v("V2");
            let breaks: Vec<(u32, f32, bool, u32)> = c
                .iter()
                .map(|c| {
                    let d = c.run.deactivation.clone().expect("break");
                    (d.days, d.share, d.placebo, d.ends_before_end)
                })
                .collect();
            assert_eq!(
                breaks,
                [
                    (35, 0.2, false, 0),
                    (35, 0.2, true, 0),
                    (35, 0.2, false, 28),
                    (35, 0.2, true, 28)
                ]
            );
            assert!(c.iter().all(|c| c.design == design(er)));
            let c = v("V4");
            assert_eq!(
                (c[0].design.share_friction, c[1].design.share_friction),
                (0.3, 0.0)
            );
            for id in ["V6", "V8"] {
                assert_eq!(v(id)[0].design, design(er), "{id}");
            }
            let mut notes = design(er);
            notes.community_notes = true;
            assert_eq!(v("V7")[0].design, notes);
            let c = v("V9");
            assert_eq!((c[0].design.ads_share, c[1].design.ads_share), (0.5, 1.0));
            // Nothing else changes: the run, the population and the world.
            for c in &v("V9") {
                assert_eq!(c.world, base.world);
                assert_eq!(c.population, base.population);
                assert!(c.run.feed_experiment.is_none() && c.run.deactivation.is_none());
            }
        }
    }

    #[test]
    fn the_doses_are_the_calibrated_values() {
        assert_eq!((V1_PENALTY, V3_THRESHOLD), (0.28, 0.56));
        assert!((V1_DOSE_ACHIEVED - V1_DOSE_TARGET).abs() < 0.005);
        assert!((V3_DOSE_ACHIEVED - V3_DOSE_TARGET).abs() < 0.005);
        let dose = |id: &str| {
            validations()
                .iter()
                .find(|v| v.id == id)
                .and_then(|v| v.dose)
        };
        let d1 = dose("V1").expect("V1 has a dose");
        assert_eq!((d1.value, d1.achieved), (V1_PENALTY, V1_DOSE_ACHIEVED));
        let d3 = dose("V3").expect("V3 has a dose");
        assert_eq!((d3.value, d3.achieved), (V3_THRESHOLD, V3_DOSE_ACHIEVED));
        let with_doses: Vec<&str> = validations()
            .iter()
            .filter(|v| v.dose.is_some())
            .map(|v| v.id)
            .collect();
        assert_eq!(with_doses, ["V1", "V3"]);
        // The calibration's configurations set the dose and nothing else.
        let v1 = validations().iter().find(|v| v.id == "V1").expect("listed");
        let base = Config::default();
        let mut expected = v1.fact.configs_from(&base, &run(), 1);
        for c in &mut expected {
            if let Some(f) = c.run.feed_experiment.as_mut() {
                f.kind = FeedExperimentKind::FewerSameGroup { penalty: 0.5 };
            }
        }
        assert_eq!(v1.configs_with_dose(&base, &run(), 1, 0.5), expected);
    }

    /// The wiring of every check on a tiny run, under both designs: each
    /// check and raw change yields one value per seed. Whether the checks
    /// hold is a question for full size (`incentive validate`).
    #[test]
    fn every_check_evaluates() {
        let run = FactRun {
            seeds: vec![1, 2],
            population: 300,
            days: 120,
        };
        for v in validations() {
            for fact in [v.fact, v.engagement] {
                let mut histories = vec![Vec::new(); fact.variants.len()];
                for &seed in &run.seeds {
                    for (k, config) in fact.configs(&run, seed).into_iter().enumerate() {
                        let mut sim = Simulation::new(config).expect("valid");
                        sim.run_to_end();
                        histories[k].push(sim.history().to_vec());
                    }
                }
                let details = fact.evaluate_detailed(&histories);
                assert_eq!(details.len(), fact.checks.len());
                for d in &details {
                    assert_eq!(d.per_seed.len(), 2, "{}: {}", v.id, d.result.label);
                }
                let raws = v.raw_changes(&histories);
                assert_eq!(raws.len(), fact.checks.len());
                for other in v.others(&histories) {
                    assert!(other.effect.is_finite(), "{}: {other:?}", v.id);
                }
                // An effect in standard deviations reports the one it used.
                for (c, d) in fact.checks.iter().zip(&details) {
                    assert_eq!(c.scale.is_some(), d.result.scale.is_some());
                    if let Some(sd) = d.result.scale {
                        assert!(sd.is_finite() && sd > 0.0, "{}: {sd}", v.id);
                    }
                }
                if matches!(v.id, "V1" | "V2" | "V3" | "V5" | "V10") {
                    for d in &details {
                        assert!(d.result.effect.is_finite(), "{}: {:?}", v.id, d.result);
                    }
                    for r in &raws {
                        assert!(r.effect.is_finite(), "{}: {r:?}", v.id);
                    }
                }
                if let Some(dose) = v.dose {
                    assert!(dose.evaluate(&histories).effect.is_finite(), "{}", v.id);
                }
            }
        }
    }
}
