//! Each scenario's baseline fails and its recorded known solution passes
//! (docs/model.md, 14.2), on runs of reduced population.

use incentive_core::{Config, Simulation, Summary, scenario};

const SEEDS: [u64; 3] = [1, 2, 3];
const POPULATION: u32 = 600;

fn mean_summary(config: &Config) -> Summary {
    let summaries: Vec<Summary> = SEEDS
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
        .collect();
    Summary::mean(&summaries).expect("at least one seed")
}

fn check(id: &str) {
    let sc = scenario(id).expect("known scenario");
    let baseline = mean_summary(&sc.config());
    let baseline_results = sc.evaluate(&baseline, &baseline);
    assert!(
        baseline_results.iter().any(|r| !r.passed),
        "{id}: the baseline passes"
    );
    let design = sc.known_solution().expect("valid solution");
    let player = mean_summary(&sc.player_config(&design));
    for r in sc.evaluate(&player, &baseline) {
        assert!(
            r.passed,
            "{id}: {} ({} against threshold {})",
            r.description, r.player, r.threshold
        );
    }
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
