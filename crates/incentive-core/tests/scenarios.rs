//! Each scenario's baseline fails, its recorded known solution passes and
//! its recorded naive attempt, one obvious lever, fails (docs/model.md, 14.2
//! and 13.5), on runs of reduced population. Goals are judged on paired
//! per-seed margins with a 95% interval (`Scenario::judge`), so a known
//! solution must pass, and a naive attempt fail, beyond the noise of these
//! seeds. One test per scenario checks all three on the same baseline runs.

use incentive_core::{Config, Simulation, Summary, Verdict, scenario};

/// Six seeds: with three, the 95% interval (Student's t with two degrees of
/// freedom) of the trap's toxicity margin straddled zero although its mean
/// passed by a third of the threshold.
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const POPULATION: u32 = 600;

fn summaries(config: &Config) -> Vec<Summary> {
    SEEDS
        .iter()
        .map(|&seed| {
            let mut config = config.clone();
            config.run.seed = seed;
            config.run.population_size = POPULATION;
            config.run.tracked_agents = 0;
            Simulation::new(config)
                .expect("valid configuration")
                .run_to_end()
        })
        .collect()
}

fn check(id: &str) {
    let sc = scenario(id).expect("known scenario");
    let baseline = summaries(&sc.config());
    assert_ne!(
        sc.judge(&baseline, &baseline).verdict,
        Verdict::Pass,
        "{id}: the baseline passes"
    );
    let design = sc.known_solution().expect("valid solution");
    let player = summaries(&sc.player_config(&design));
    for g in sc.judge(&player, &baseline).goals {
        assert_eq!(
            g.verdict,
            Verdict::Pass,
            "{id}: {} ({} against threshold {}, margin {} with 95% interval {} to {})",
            g.description,
            g.player,
            g.threshold,
            g.margin.mean,
            g.margin.low,
            g.margin.high
        );
    }
    let naive = sc.naive_attempt().expect("valid naive attempt");
    let attempt = sc.judge(&summaries(&sc.player_config(&naive)), &baseline);
    let goals: Vec<String> = attempt
        .goals
        .iter()
        .map(|g| {
            format!(
                "{}: {:?}, margin {} ({} to {})",
                g.description, g.verdict, g.margin.mean, g.margin.low, g.margin.high
            )
        })
        .collect();
    assert_ne!(
        attempt.verdict,
        Verdict::Pass,
        "{id}: the naive attempt ({}) passes: {goals:?}",
        sc.naive_attempt_label()
    );
    assert_eq!(
        attempt.verdict,
        Verdict::Fail,
        "{id}: the naive attempt ({}) does not fail: {goals:?}",
        sc.naive_attempt_label()
    );
}

#[test]
fn the_engagement_trap() {
    check("engagement-trap");
}

#[test]
fn toxicity_spiral() {
    check("toxicity-spiral");
}

#[test]
fn a_living_for_creators() {
    check("creator-living");
}

#[test]
fn misinformation_surge() {
    check("misinformation-surge");
}

#[test]
fn public_square() {
    check("public-square");
}
