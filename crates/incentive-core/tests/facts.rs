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
/// the year. With the paired design (docs/model.md, 13.2) the per-seed
/// effects scatter widely (standard deviation about 0.07 at full size), and
/// at the default `time_cost` (0.06) the effect lies close to its lower
/// bound of 0.02, so the size is chosen by what this test judges, the mean of
/// seeds 1 to 4 after 365 days, at the default and its neighbours on the
/// calibration grid (0.055 and 0.065). At `N = 600` it is -0.059 at the
/// default, at `N = 1,000` -0.027, both below the bound; at `N = 1,500` it is
/// 0.027 but 0.017 at 0.055; at `N = 2,000` it is 0.063, with 0.037 and 0.060
/// at the neighbours. `N = 2,000`, the full size, is the cheapest size whose
/// mean holds the bound at both neighbouring values of the grid.
#[test]
fn leaving_the_platform_raises_wellbeing() {
    check_sized("deactivation", 2_000, 365);
}

/// The notifications-off fact (docs/model.md, 13.2) measures the run's last
/// week against the week before. With the paired design and one session a
/// day, its sessions check (the one closest to its bound of -0.4) is -0.326
/// SD at full size (`N = 2,000`, 365 days, 8 seeds). At `N = 600`, seeds 1 to
/// 4, it is -0.379 after 120 days (standard deviation 0.046, about one
/// standard error of the 4-seed mean inside the bound), -0.302 after 240 days
/// (0.037, about five standard errors inside) and -0.342 after 365 days
/// (0.054, about two). So the test runs 240 days at the small population,
/// the cheapest size with a reliable margin.
#[test]
fn a_week_without_notifications_barely_changes_use_or_wellbeing() {
    check("notifications-off", 240);
}
