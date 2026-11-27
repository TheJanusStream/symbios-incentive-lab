//! The headline claims at the defaults on small paired runs. `incentive
//! robustness` judges them at full size and across worlds.

use incentive_core::{Fact, FactRun, headline_claims};

/// Checks a claim on seeds 1 to 4 at `N = 600` for 120 days, on the seed
/// means, as the fact tests do.
fn check(id: &str) {
    let claim = headline_claims()
        .iter()
        .find(|c| c.id == id)
        .expect("known claim");
    let run = FactRun {
        seeds: (1..=4).collect(),
        population: 600,
        days: 120,
    };
    let results = claim.run_sequential(&run).expect("valid configurations");
    for r in &results {
        assert!(
            r.passed,
            "{} {}: {} has effect {} (sd {}) outside {}",
            claim.id,
            claim.title,
            r.label,
            r.effect,
            r.spread,
            r.bound.text()
        );
    }
    assert!(Fact::holds(&results));
}

// At this size every seed's effect of H1 to H6 lies in its band. H7 and H8
// hold on the mean of seeds 1 to 4 but not on every seed (report trust's
// harassment cut: -1.62, -2.83, -0.30, +0.26 per 1,000; the see-less
// penalty's harm rise: +0.085, +0.039, -0.040, +0.063), so they are left
// to the runner at full size, where every seed holds (lab #31).

#[test]
fn calmer_feed_cuts_harm_more_than_engagement() {
    check("H1");
}

#[test]
fn survey_misranks() {
    check("H2");
}

#[test]
fn strict_classifier_cuts_toxicity_and_sanctions_the_innocent() {
    check("H3");
}

#[test]
fn trap_solution_cuts_harm_and_keeps_engagement() {
    check("H4");
}

#[test]
fn notifications_raise_engagement_not_wellbeing() {
    check("H5");
}

#[test]
fn engagement_lead_lasts() {
    check("H6");
}
