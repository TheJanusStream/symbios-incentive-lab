//! The lab's headline findings as claims to test (docs/model.md, 13.3, and
//! the app's "Things to discover"): each claim is a [`Fact`] with paired
//! variants and band checks, built from the machinery of the stylized facts,
//! so the runner judges both the same way. A claim holds in a world when
//! every check lies in its band on the seed means ([`Fact::holds`]).
//!
//! Every check measures the window its comparison has in the calibration
//! report (`incentive report`): the summary window, the last 30% of the run
//! (110 days of 365), unless the claim names days. The bands were fixed by
//! the owner before the claims were run across worlds (lab #31); a band
//! open on one side uses a large finite bound.
//!
//! Each variant replaces the design with its preset's and keeps the run, the
//! population and the world of the configuration it starts from, so a claim
//! can be checked in any world ([`Fact::configs_from`]).

use crate::config::{Config, DesignLevers};
use crate::facts::{Check, Effect, Fact, Judge, Measure, Variant, check};
use crate::metrics::MetricId;
use crate::presets::quality_bridging;

/// The bound standing in for infinity in a band open on one side.
const OPEN: f32 = 1.0e9;

/// Every headline claim, H1 to H8, in the order of lab #31.
pub fn headline_claims() -> &'static [Fact] {
    &CLAIMS
}

/// The default design, the engagement ranking.
const ER: Variant = Variant {
    label: "engagement ranking",
    apply: |c| c.design = DesignLevers::default(),
};
/// Quality and bridging.
const QB: Variant = Variant {
    label: "quality and bridging",
    apply: |c| c.design = qb(),
};

fn qb() -> DesignLevers {
    let mut design = DesignLevers::default();
    quality_bridging(&mut design);
    design
}

/// The engagement ranking with a change on top.
fn er_with(c: &mut Config, change: fn(&mut DesignLevers)) {
    c.design = DesignLevers::default();
    change(&mut c.design);
}

/// A band check on the summary window.
const fn summary(label: &'static str, id: MetricId, effect: Effect, low: f32, high: f32) -> Check {
    check(
        label,
        Measure::Summary(id),
        Judge::Band { effect, low, high },
    )
}

const CLAIMS: [Fact; 8] = [
    Fact {
        id: "H1",
        title: "A calmer feed cuts harm far more than engagement",
        reference: "docs/model.md 13.3, preset table over 365 days (row 20): harm index -80%, engagement \
            index -17%",
        variants: &[ER, QB],
        checks: &[
            summary(
                "harm index, relative change, Q&B against the engagement ranking",
                MetricId::HarmIndex,
                Effect::Relative { a: 1, b: 0 },
                -1.0,
                -0.5,
            ),
            summary(
                "engagement index, relative change, Q&B against the engagement ranking",
                MetricId::EngagementIndex,
                Effect::Relative { a: 1, b: 0 },
                -0.33,
                0.33,
            ),
        ],
    },
    Fact {
        id: "H2",
        title: "The survey misranks: it puts the engagement ranking above Q&B while wellbeing puts it below",
        reference: "docs/model.md 13.3, findings of v0.2: the survey over the year",
        variants: &[ER, QB],
        checks: &[
            summary(
                "survey satisfaction, engagement ranking minus Q&B",
                MetricId::SurveySatisfaction,
                Effect::Difference { a: 0, b: 1 },
                0.0,
                OPEN,
            ),
            summary(
                "wellbeing of ever-users, engagement ranking minus Q&B",
                MetricId::WellbeingEverUsers,
                Effect::Difference { a: 0, b: 1 },
                -OPEN,
                0.0,
            ),
        ],
    },
    Fact {
        id: "H3",
        title: "A strict classifier cuts toxicity but sanctions the innocent",
        reference: "docs/model.md 13.3, findings of v0.1 (row 9): toxic share -95%, wrongful sanctions 14 times",
        variants: &[
            ER,
            Variant {
                label: "engagement ranking, classifier threshold 0.3",
                apply: |c| er_with(c, |d| d.classifier_threshold = 0.3),
            },
        ],
        checks: &[
            summary(
                "toxic impressions, threshold 0.3 / default",
                MetricId::ToxicShare,
                Effect::Ratio { a: 1, b: 0 },
                0.0,
                0.5,
            ),
            summary(
                "false-positive actions, threshold 0.3 / default",
                MetricId::FalsePositives,
                Effect::Ratio { a: 1, b: 0 },
                3.0,
                OPEN,
            ),
        ],
    },
    Fact {
        id: "H4",
        title: "The engagement trap's solution cuts harm and keeps engagement",
        reference: "docs/model.md 13.3 (row 1): harm index -0.153, engagement index +10%",
        variants: &[
            ER,
            Variant {
                label: "engagement ranking, w_reply 0, review_capacity 10, report_trust 1",
                apply: |c| {
                    er_with(c, |d| {
                        d.w_reply = 0.0;
                        d.review_capacity = 10.0;
                        d.report_trust = 1.0;
                    })
                },
            },
        ],
        checks: &[
            summary(
                "harm index, trap solution minus engagement ranking",
                MetricId::HarmIndex,
                Effect::Difference { a: 1, b: 0 },
                -OPEN,
                0.0,
            ),
            summary(
                "engagement index, trap solution / engagement ranking",
                MetricId::EngagementIndex,
                Effect::Ratio { a: 1, b: 0 },
                0.9,
                OPEN,
            ),
        ],
    },
    Fact {
        id: "H5",
        title: "Notifications raise engagement, not wellbeing, under the engagement ranking",
        reference: "docs/model.md 13.3 (row 31): engagement index +22%, users' wellbeing -0.001",
        variants: &[
            ER,
            Variant {
                label: "engagement ranking, notification intensity 1",
                apply: |c| er_with(c, |d| d.notification_intensity = 1.0),
            },
        ],
        checks: &[
            summary(
                "engagement index, intensity 1 / 0.6",
                MetricId::EngagementIndex,
                Effect::Ratio { a: 1, b: 0 },
                1.0,
                OPEN,
            ),
            summary(
                "wellbeing of users, intensity 1 minus 0.6",
                MetricId::WellbeingUsers,
                Effect::Difference { a: 1, b: 0 },
                -OPEN,
                0.01,
            ),
        ],
    },
    Fact {
        id: "H6",
        title: "The engagement ranking's engagement lead over Q&B lasts",
        reference: "docs/model.md 13.3 (row 13): 1.20 times in the last 95 days",
        variants: &[QB, ER],
        checks: &[check(
            "engagement index, last 95 days, engagement ranking / Q&B",
            Measure::LastDays(MetricId::EngagementIndex, 95),
            Judge::Band {
                effect: Effect::Ratio { a: 1, b: 0 },
                low: 1.0,
                high: OPEN,
            },
        )],
    },
    Fact {
        id: "H7",
        title: "Report trust cuts harassment under Q&B",
        reference: "docs/model.md 13.3, findings of v0.2 (row 26): harassment 3.68 to 1.73 per 1,000",
        variants: &[
            QB,
            Variant {
                label: "quality and bridging, report trust 1",
                apply: |c| {
                    c.design = qb();
                    c.design.report_trust = 1.0;
                },
            },
        ],
        checks: &[summary(
            "harassment per 1,000 users, report trust 1 minus 0",
            MetricId::HarassmentPer1000,
            Effect::Difference { a: 1, b: 0 },
            -OPEN,
            0.0,
        )],
    },
    Fact {
        id: "H8",
        title: "A strong see-less penalty backfires under bot brigading",
        reference: "docs/model.md 13.3, findings of v0.1 (row 6): harm index +0.251",
        variants: &[
            ER,
            Variant {
                label: "engagement ranking, w_see_less 5",
                apply: |c| er_with(c, |d| d.w_see_less = 5.0),
            },
        ],
        checks: &[summary(
            "harm index, w_see_less 5 minus 0",
            MetricId::HarmIndex,
            Effect::Difference { a: 1, b: 0 },
            0.0,
            OPEN,
        )],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{CheckResult, FactRun};
    use crate::presets::preset;
    use crate::simulation::Simulation;

    #[test]
    fn claims_are_well_formed() {
        let claims = headline_claims();
        assert_eq!(claims.len(), 8);
        for (i, claim) in claims.iter().enumerate() {
            assert_eq!(claim.id, format!("H{}", i + 1));
            assert!(claim.variants.len() >= 2 && !claim.checks.is_empty());
            assert!(claim.checks.iter().all(|c| !c.known_limit));
        }
    }

    /// The variants set the presets' designs and keep the world they start
    /// from, as the report's sides do.
    #[test]
    fn variants_match_the_report_sides() {
        let mut base = Config::default();
        base.world.outrage_appeal = 2.0;
        let run = FactRun {
            seeds: vec![3],
            population: 300,
            days: 20,
        };
        let h1 = &headline_claims()[0].configs_from(&base, &run, 3);
        let expected = |id: &str| {
            let mut c = base.clone();
            preset(id).map(|p| p.apply_to(&mut c)).ok();
            c.design
        };
        assert_eq!(h1[0].design, expected("engagement-maximiser"));
        assert_eq!(h1[1].design, expected("quality-bridging"));
        assert!(h1.iter().all(|c| c.world.outrage_appeal == 2.0));
        assert!(h1.iter().all(|c| c.run.population_size == 300));
        let h7 = &headline_claims()[6].configs_from(&base, &run, 3);
        assert_eq!(h7[1].design.report_trust, 1.0);
        assert_eq!(
            h7[1].design.w_quality,
            expected("quality-bridging").w_quality
        );
    }

    /// The summary measure is the summary's own value, so the claims read
    /// what the calibration report reads.
    #[test]
    fn summary_measure_equals_the_run_summary() {
        let mut config = Config::default();
        config.run.population_size = 200;
        config.run.days = 47;
        config.run.tracked_agents = 0;
        let mut sim = Simulation::new(config).expect("valid");
        let summary = sim.run_to_end();
        for id in [MetricId::HarmIndex, MetricId::EngagementIndex] {
            let m = Measure::Summary(id).of(sim.history());
            assert!((m - summary.get(id)).abs() < 1e-6, "{id:?}");
        }
    }

    /// The wiring of every claim on a tiny run: each check yields a finite
    /// effect and per-seed effects, and a claim's verdict is the conjunction
    /// of its checks. Whether the claims hold is a question for full size
    /// (`incentive robustness`); they are not robust at a test size.
    #[test]
    fn every_claim_evaluates() {
        let run = FactRun {
            seeds: vec![1, 2],
            population: 300,
            days: 40,
        };
        for claim in headline_claims() {
            let mut histories = vec![Vec::new(); claim.variants.len()];
            for &seed in &run.seeds {
                for (v, config) in claim.configs(&run, seed).into_iter().enumerate() {
                    let mut sim = Simulation::new(config).expect("valid");
                    sim.run_to_end();
                    histories[v].push(sim.history().to_vec());
                }
            }
            let details = claim.evaluate_detailed(&histories);
            assert_eq!(details.len(), claim.checks.len());
            for d in &details {
                assert_eq!(d.per_seed.len(), 2, "{}: {}", claim.id, d.result.label);
            }
            let results: Vec<CheckResult> = details.into_iter().map(|d| d.result).collect();
            assert_eq!(Fact::holds(&results), results.iter().all(|r| r.passed));
        }
    }
}
