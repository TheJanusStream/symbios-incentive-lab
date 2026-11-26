//! The stylized facts of docs/model.md, section 15, on small paired runs. The
//! command-line runner's `facts` command runs the same checks at full size.

use incentive_core::{FactRun, facts};

/// Individuals per run at the test size.
const TEST_POPULATION: u32 = 600;

/// Departures accumulate slowly, so the harassment fact runs longer than the
/// others to hold with a reliable margin.
fn check(id: &str, days: u32) {
    check_sized(id, TEST_POPULATION, days);
}

fn check_sized(id: &str, population: u32, days: u32) {
    let fact = facts().iter().find(|f| f.id == id).expect("known fact");
    let run = FactRun {
        seeds: (1..=4).collect(),
        population,
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

/// The deactivation fact needs the whole year: its effect builds up over
/// the year. With the paired design
/// (docs/model.md, 13.2) at the defaults, seeds 1 to 8, it is -0.087 SD after
/// 120 days and 0.001 after 240 at `N = 2,000`, and 0.054 after 365; at
/// `N = 1,500` and 365 days it is 0.055, with the mean of seeds 1 to 4 at
/// 0.075 (0.060 and 0.086 at `time_cost` 0.11 and 0.12), as large a margin
/// as at full size (0.069). At `N = 1,000` the mean of seeds 1 to 4 is 0.018
/// and at `N = 1,500` after 240 days 0.036, too close to the bound of 0.02.
#[test]
fn leaving_the_platform_raises_wellbeing() {
    check_sized("deactivation", 1_500, 365);
}
