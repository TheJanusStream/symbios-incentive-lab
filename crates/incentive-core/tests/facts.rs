//! The stylized facts of docs/model.md, section 15, on small paired runs. The
//! command-line runner's `facts` command runs the same checks at full size.

use incentive_core::{FactRun, facts};

/// Departures accumulate slowly, so the harassment fact runs longer than the
/// others to hold with a reliable margin.
fn check(id: &str, days: u32) {
    let fact = facts().iter().find(|f| f.id == id).expect("known fact");
    let run = FactRun {
        seeds: (1..=4).collect(),
        population: 600,
        days,
    };
    let results = fact.run_sequential(&run).expect("valid configurations");
    // These are regression checks of direction at a small size: they judge
    // the effect on the seed means against the band, as before. The runner
    // (`incentive facts`, `incentive report`) gives each check the verdict
    // on the 95% interval of its per-seed effect at full size, which 4 seeds
    // cannot settle. A check the model is known to fail (docs/model.md,
    // section 16) is reported by the runner but not required here.
    for r in results.iter().filter(|r| !r.known_limit) {
        assert!(
            r.passed,
            "{}: {} has effect {} (sd {}) against threshold {}",
            fact.title, r.label, r.effect, r.spread, r.threshold
        );
    }
}

#[test]
fn engagement_ranking_amplifies_divisive_content() {
    check("amplification", 120);
}

#[test]
fn social_reward_teaches_outrage() {
    check("outrage-learning", 120);
}

#[test]
fn chronological_feed_cuts_time_and_barely_moves_polarisation() {
    check("chronological-feed", 120);
}

#[test]
fn forwarding_limits_slow_viral_spread() {
    check("forwarding-limits", 120);
}

#[test]
fn harassment_drives_targets_away() {
    check("harassment-exit", 240);
}
